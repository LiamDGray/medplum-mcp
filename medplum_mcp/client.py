"""Resilient Medplum FHIR Client with In-Memory Sandbox and Context Distillation.

Supports zero-latency demo mode, robust HTTP retries on 429/503, and transparent
3-tier FHIR token distillation.
"""

from __future__ import annotations

import logging
import time
from typing import Any

import requests

from medplum_mcp.demo import ClinicalSandbox
from medplum_mcp.safety import assert_write_permitted
from medplum_mcp.token_diet import DetailLevel, distill_fhir_resource
from medplum_mcp.vault import SecretString

logger = logging.getLogger(__name__)


class MedplumClient:
    """High-assurance client for Medplum FHIR REST API with sandbox and safety gates.

    In demo mode, calls ClinicalSandbox in-process without network overhead.
    In live mode, executes resilient HTTP requests with exponential backoff on 429/503.
    Enforces Zero Unauthorized Commitment invariants before any mutation.
    """

    def __init__(
        self,
        base_url: str = "https://api.medplum.com",
        access_token: str | SecretString | None = None,
        demo_mode: bool = False,
        timeout: float = 30.0,
        max_retries: int = 3,
        backoff_factor: float = 0.5,
        default_detail_level: str | DetailLevel | None = DetailLevel.STANDARD,
        sandbox: ClinicalSandbox | None = None,
    ) -> None:
        self.base_url = base_url.rstrip("/")
        self._access_token = access_token
        self.demo_mode = demo_mode
        self.timeout = timeout
        self.max_retries = max_retries
        self.backoff_factor = backoff_factor
        self.default_detail_level = default_detail_level

        self._sandbox: ClinicalSandbox | None = None
        if demo_mode:
            self._sandbox = sandbox if sandbox is not None else ClinicalSandbox()

        self._session = requests.Session()

    @property
    def sandbox(self) -> ClinicalSandbox:
        """Access the underlying in-memory sandbox (only in demo mode)."""
        if self._sandbox is None:
            raise RuntimeError("Sandbox is only available when demo_mode=True")
        return self._sandbox

    def _get_token_value(self) -> str | None:
        """Unwrap access token string safely."""
        if self._access_token is None:
            return None
        if isinstance(self._access_token, SecretString):
            return self._access_token.get_secret_value()
        return str(self._access_token)

    def _apply_distillation(
        self, payload: dict[str, Any], detail_level: str | DetailLevel | None
    ) -> dict[str, Any]:
        """Distill payload if detail_level is specified."""
        target_level = detail_level if detail_level is not None else self.default_detail_level
        if target_level is None:
            return payload
        return distill_fhir_resource(payload, detail_level=target_level)

    # -----------------------------------------------------------------------
    # Resilient HTTP Execution (Live Mode)
    # -----------------------------------------------------------------------

    def _http_request(
        self,
        method: str,
        path: str,
        params: dict[str, Any] | None = None,
        json_data: dict[str, Any] | None = None,
    ) -> requests.Response:
        """Execute HTTP request with exponential backoff on 429/503."""
        url = f"{self.base_url}/{path.lstrip('/')}"
        headers: dict[str, str] = {
            "Accept": "application/fhir+json, application/json",
            "Content-Type": "application/fhir+json; charset=utf-8",
        }
        token = self._get_token_value()
        if token:
            headers["Authorization"] = f"Bearer {token}"

        last_resp: requests.Response | None = None
        for attempt in range(self.max_retries + 1):
            try:
                resp = self._session.request(
                    method=method,
                    url=url,
                    params=params,
                    json=json_data,
                    headers=headers,
                    timeout=self.timeout,
                )
                last_resp = resp

                if resp.status_code in {429, 503} and attempt < self.max_retries:
                    retry_after_hdr = resp.headers.get("Retry-After")
                    if retry_after_hdr and retry_after_hdr.isdigit():
                        sleep_time = float(retry_after_hdr)
                    else:
                        sleep_time = self.backoff_factor * (2**attempt)
                    logger.warning(
                        "Received HTTP %d from %s; backing off for %.2fs (attempt %d/%d)",
                        resp.status_code,
                        url,
                        sleep_time,
                        attempt + 1,
                        self.max_retries,
                    )
                    time.sleep(sleep_time)
                    continue

                return resp

            except (requests.ConnectionError, requests.Timeout) as ex:
                if attempt < self.max_retries:
                    sleep_time = self.backoff_factor * (2**attempt)
                    logger.warning(
                        "Network error %s; retrying in %.2fs (attempt %d/%d)",
                        ex,
                        sleep_time,
                        attempt + 1,
                        self.max_retries,
                    )
                    time.sleep(sleep_time)
                    continue
                raise

        if last_resp is not None:
            return last_resp
        raise RuntimeError("HTTP request execution failed without response")

    # -----------------------------------------------------------------------
    # Generic FHIR Operations
    # -----------------------------------------------------------------------

    def get_resource(
        self,
        resource_type: str,
        resource_id: str,
        detail_level: str | DetailLevel | None = DetailLevel.STANDARD,
    ) -> dict[str, Any] | None:
        """Fetch a single resource by type and ID."""
        clean_id = resource_id.replace(f"{resource_type}/", "").strip()

        if self.demo_mode and self._sandbox is not None:
            fetchers = {
                "Patient": self._sandbox.get_patient,
                "Observation": self._sandbox.get_observation,
                "Condition": self._sandbox.get_condition,
                "MedicationRequest": self._sandbox.get_medication,
                "AllergyIntolerance": self._sandbox.get_allergy,
                "DiagnosticReport": self._sandbox.get_diagnostic_report,
                "Encounter": self._sandbox.get_encounter,
                "CarePlan": self._sandbox.get_care_plan,
            }
            fetcher = fetchers.get(resource_type)
            if not fetcher:
                return None
            res = fetcher(clean_id)
            if res is None:
                return None
            return self._apply_distillation(res, detail_level)

        # Live Mode
        resp = self._http_request("GET", f"fhir/R4/{resource_type}/{clean_id}")
        if resp.status_code == 404:
            return None
        resp.raise_for_status()
        data: dict[str, Any] = resp.json()
        return self._apply_distillation(data, detail_level)

    def search_resources(
        self,
        resource_type: str,
        params: dict[str, Any] | None = None,
        detail_level: str | DetailLevel | None = DetailLevel.STANDARD,
    ) -> dict[str, Any]:
        """Search FHIR resources returning a distilled Bundle."""
        if self.demo_mode and self._sandbox is not None:
            filter_params = params or {}
            if resource_type == "Patient":
                items = self._sandbox.search_patients(
                    name=filter_params.get("name"),
                    identifier=filter_params.get("identifier"),
                    query=filter_params.get("query"),
                    count=filter_params.get("_count"),
                )
            elif resource_type == "Observation":
                pat_id = filter_params.get("patient") or filter_params.get("subject")
                items = self._sandbox.list_observations(
                    patient_id=pat_id,
                    category=filter_params.get("category"),
                    code=filter_params.get("code"),
                    count=filter_params.get("_count"),
                )
            elif resource_type == "Condition":
                pat_id = filter_params.get("patient") or filter_params.get("subject")
                items = self._sandbox.list_conditions(
                    patient_id=pat_id,
                    count=filter_params.get("_count"),
                )
            elif resource_type == "MedicationRequest":
                pat_id = filter_params.get("patient") or filter_params.get("subject")
                items = self._sandbox.list_medications(
                    patient_id=pat_id,
                    status=filter_params.get("status"),
                    count=filter_params.get("_count"),
                )
            elif resource_type == "AllergyIntolerance":
                pat_id = filter_params.get("patient") or filter_params.get("subject")
                items = self._sandbox.list_allergies(
                    patient_id=pat_id,
                    count=filter_params.get("_count"),
                )
            elif resource_type == "DiagnosticReport":
                pat_id = filter_params.get("patient") or filter_params.get("subject")
                items = self._sandbox.list_diagnostic_reports(
                    patient_id=pat_id,
                    code=filter_params.get("code"),
                    count=filter_params.get("_count"),
                )
            elif resource_type == "Encounter":
                pat_id = filter_params.get("patient") or filter_params.get("subject")
                items = self._sandbox.list_encounters(
                    patient_id=pat_id,
                    status=filter_params.get("status"),
                    count=filter_params.get("_count"),
                )
            elif resource_type == "CarePlan":
                pat_id = filter_params.get("patient") or filter_params.get("subject")
                items = self._sandbox.list_care_plans(
                    patient_id=pat_id,
                    status=filter_params.get("status"),
                    count=filter_params.get("_count"),
                )
            else:
                items = []

            bundle = self._sandbox.to_bundle(items)
            return self._apply_distillation(bundle, detail_level)

        # Live Mode
        resp = self._http_request("GET", f"fhir/R4/{resource_type}", params=params)
        resp.raise_for_status()
        bundle_data: dict[str, Any] = resp.json()
        return self._apply_distillation(bundle_data, detail_level)

    def create_resource(
        self,
        resource_type: str,
        payload: dict[str, Any],
        allow_writes: bool = False,
        detail_level: str | DetailLevel | None = DetailLevel.STANDARD,
    ) -> dict[str, Any]:
        """Create a resource draft, asserting safety invariants at client boundary."""
        # Non-bypassable safety verification at client boundary
        assert_write_permitted(resource_type, payload, allow_writes=allow_writes)

        if self.demo_mode and self._sandbox is not None:
            if resource_type == "Observation":
                created = self._sandbox.create_observation_draft(payload)
            elif resource_type == "MedicationRequest":
                created = self._sandbox.create_medication_draft(payload)
            else:
                raise NotImplementedError(
                    f"Draft creation not supported in sandbox for {resource_type}"
                )
            return self._apply_distillation(created, detail_level)

        # Live Mode
        resp = self._http_request("POST", f"fhir/R4/{resource_type}", json_data=payload)
        resp.raise_for_status()
        created_data: dict[str, Any] = resp.json()
        return self._apply_distillation(created_data, detail_level)

    # -----------------------------------------------------------------------
    # Typed Convenience Methods
    # -----------------------------------------------------------------------

    def search_patients(
        self,
        name: str | None = None,
        identifier: str | None = None,
        query: str | None = None,
        count: int | None = None,
        detail_level: str | DetailLevel | None = DetailLevel.STANDARD,
    ) -> dict[str, Any]:
        """Search patients by name, identifier, or query."""
        params: dict[str, Any] = {}
        if name:
            params["name"] = name
        if identifier:
            params["identifier"] = identifier
        if query:
            params["query"] = query
        if count:
            params["_count"] = count
        return self.search_resources("Patient", params=params, detail_level=detail_level)

    def get_patient(
        self,
        patient_id: str,
        detail_level: str | DetailLevel | None = DetailLevel.STANDARD,
    ) -> dict[str, Any] | None:
        """Fetch a single Patient by ID."""
        return self.get_resource("Patient", patient_id, detail_level=detail_level)

    def list_observations(
        self,
        patient_id: str | None = None,
        category: str | None = None,
        code: str | None = None,
        count: int | None = None,
        detail_level: str | DetailLevel | None = DetailLevel.STANDARD,
    ) -> dict[str, Any]:
        """List observations filtered by patient, category, or code."""
        params: dict[str, Any] = {}
        if patient_id:
            params["patient"] = patient_id
        if category:
            params["category"] = category
        if code:
            params["code"] = code
        if count:
            params["_count"] = count
        return self.search_resources("Observation", params=params, detail_level=detail_level)

    def get_observation(
        self,
        observation_id: str,
        detail_level: str | DetailLevel | None = DetailLevel.STANDARD,
    ) -> dict[str, Any] | None:
        """Fetch a single Observation by ID."""
        return self.get_resource("Observation", observation_id, detail_level=detail_level)

    def create_observation(
        self,
        resource: dict[str, Any],
        allow_writes: bool = False,
        detail_level: str | DetailLevel | None = DetailLevel.STANDARD,
    ) -> dict[str, Any]:
        """Create a draft Observation, enforcing write permission and draft invariants."""
        return self.create_resource(
            "Observation", resource, allow_writes=allow_writes, detail_level=detail_level
        )

    def create_observation_draft(
        self,
        resource: dict[str, Any],
        allow_writes: bool = False,
        detail_level: str | DetailLevel | None = DetailLevel.STANDARD,
    ) -> dict[str, Any]:
        """Alias for create_observation."""
        return self.create_observation(
            resource, allow_writes=allow_writes, detail_level=detail_level
        )

    def list_conditions(
        self,
        patient_id: str | None = None,
        count: int | None = None,
        detail_level: str | DetailLevel | None = DetailLevel.STANDARD,
    ) -> dict[str, Any]:
        """List conditions filtered by patient."""
        params: dict[str, Any] = {}
        if patient_id:
            params["patient"] = patient_id
        if count:
            params["_count"] = count
        return self.search_resources("Condition", params=params, detail_level=detail_level)

    def get_condition(
        self,
        condition_id: str,
        detail_level: str | DetailLevel | None = DetailLevel.STANDARD,
    ) -> dict[str, Any] | None:
        """Fetch a single Condition by ID."""
        return self.get_resource("Condition", condition_id, detail_level=detail_level)

    def list_medications(
        self,
        patient_id: str | None = None,
        status: str | None = None,
        count: int | None = None,
        detail_level: str | DetailLevel | None = DetailLevel.STANDARD,
    ) -> dict[str, Any]:
        """List MedicationRequests filtered by patient and status."""
        params: dict[str, Any] = {}
        if patient_id:
            params["patient"] = patient_id
        if status:
            params["status"] = status
        if count:
            params["_count"] = count
        return self.search_resources("MedicationRequest", params=params, detail_level=detail_level)

    def get_medication(
        self,
        medication_id: str,
        detail_level: str | DetailLevel | None = DetailLevel.STANDARD,
    ) -> dict[str, Any] | None:
        """Fetch a single MedicationRequest by ID."""
        return self.get_resource("MedicationRequest", medication_id, detail_level=detail_level)

    def create_medication_request(
        self,
        resource: dict[str, Any],
        allow_writes: bool = False,
        detail_level: str | DetailLevel | None = DetailLevel.STANDARD,
    ) -> dict[str, Any]:
        """Create a draft MedicationRequest, enforcing write permission and draft invariants."""
        return self.create_resource(
            "MedicationRequest", resource, allow_writes=allow_writes, detail_level=detail_level
        )

    def create_medication_draft(
        self,
        resource: dict[str, Any],
        allow_writes: bool = False,
        detail_level: str | DetailLevel | None = DetailLevel.STANDARD,
    ) -> dict[str, Any]:
        """Alias for create_medication_request."""
        return self.create_medication_request(
            resource, allow_writes=allow_writes, detail_level=detail_level
        )

    def list_allergies(
        self,
        patient_id: str | None = None,
        count: int | None = None,
        detail_level: str | DetailLevel | None = DetailLevel.STANDARD,
    ) -> dict[str, Any]:
        """List AllergyIntolerances filtered by patient."""
        params: dict[str, Any] = {}
        if patient_id:
            params["patient"] = patient_id
        if count:
            params["_count"] = count
        return self.search_resources("AllergyIntolerance", params=params, detail_level=detail_level)

    def get_allergy(
        self,
        allergy_id: str,
        detail_level: str | DetailLevel | None = DetailLevel.STANDARD,
    ) -> dict[str, Any] | None:
        """Fetch a single AllergyIntolerance by ID."""
        return self.get_resource("AllergyIntolerance", allergy_id, detail_level=detail_level)

    def list_diagnostic_reports(
        self,
        patient_id: str | None = None,
        code: str | None = None,
        count: int | None = None,
        detail_level: str | DetailLevel | None = DetailLevel.STANDARD,
    ) -> dict[str, Any]:
        """List DiagnosticReports filtered by patient or code."""
        params: dict[str, Any] = {}
        if patient_id:
            params["patient"] = patient_id
        if code:
            params["code"] = code
        if count:
            params["_count"] = count
        return self.search_resources("DiagnosticReport", params=params, detail_level=detail_level)

    def get_diagnostic_report(
        self,
        report_id: str,
        detail_level: str | DetailLevel | None = DetailLevel.STANDARD,
    ) -> dict[str, Any] | None:
        """Fetch a single DiagnosticReport by ID."""
        return self.get_resource("DiagnosticReport", report_id, detail_level=detail_level)

    def list_encounters(
        self,
        patient_id: str | None = None,
        status: str | None = None,
        count: int | None = None,
        detail_level: str | DetailLevel | None = DetailLevel.STANDARD,
    ) -> dict[str, Any]:
        """List Encounters filtered by patient and status."""
        params: dict[str, Any] = {}
        if patient_id:
            params["patient"] = patient_id
        if status:
            params["status"] = status
        if count:
            params["_count"] = count
        return self.search_resources("Encounter", params=params, detail_level=detail_level)

    def get_encounter(
        self,
        encounter_id: str,
        detail_level: str | DetailLevel | None = DetailLevel.STANDARD,
    ) -> dict[str, Any] | None:
        """Fetch a single Encounter by ID."""
        return self.get_resource("Encounter", encounter_id, detail_level=detail_level)

    def list_care_plans(
        self,
        patient_id: str | None = None,
        status: str | None = None,
        count: int | None = None,
        detail_level: str | DetailLevel | None = DetailLevel.STANDARD,
    ) -> dict[str, Any]:
        """List CarePlans filtered by patient and status."""
        params: dict[str, Any] = {}
        if patient_id:
            params["patient"] = patient_id
        if status:
            params["status"] = status
        if count:
            params["_count"] = count
        return self.search_resources("CarePlan", params=params, detail_level=detail_level)

    def get_care_plan(
        self,
        care_plan_id: str,
        detail_level: str | DetailLevel | None = DetailLevel.STANDARD,
    ) -> dict[str, Any] | None:
        """Fetch a single CarePlan by ID."""
        return self.get_resource("CarePlan", care_plan_id, detail_level=detail_level)
