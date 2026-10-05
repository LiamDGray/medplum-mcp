"""Standalone HTTP FHIR R4 Mock Server with In-Memory Clinical Sandbox.

Implements standard FHIR R4 REST endpoints, OAuth2 token issuance, rate limit headers,
and safety invariants with zero external heavy dependencies.
"""

from __future__ import annotations

import json
import logging
import threading
import time
import urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any

from medplum_mcp.demo import ClinicalSandbox
from medplum_mcp.safety import SafetyInvariantViolation

logger = logging.getLogger(__name__)


class _MedplumMockRequestHandler(BaseHTTPRequestHandler):
    """HTTP Request Handler implementing FHIR R4 endpoints for MedplumMockServer."""

    server: MedplumMockServer  # type annotation for HTTPServer instance

    def log_message(self, format: str, *args: Any) -> None:
        """Suppress default stderr logging during automated testing."""
        logger.debug(format, *args)

    def _send_json(
        self,
        status_code: int,
        payload: dict[str, Any],
        extra_headers: dict[str, str] | None = None,
    ) -> None:
        """Serialize and send a JSON response with FHIR and rate limiting headers."""
        data = json.dumps(payload, indent=2).encode("utf-8")
        self.send_response(status_code)
        self.send_header("Content-Type", "application/fhir+json; charset=utf-8")
        self.send_header("Content-Length", str(len(data)))
        self.send_header("X-Rate-Limit-Limit", "100")
        self.send_header("X-Rate-Limit-Remaining", "99")
        if extra_headers:
            for key, val in extra_headers.items():
                self.send_header(key, val)
        self.end_headers()
        self.wfile.write(data)

    def _send_outcome(self, status_code: int, code: str, message: str) -> None:
        """Send a standard FHIR OperationOutcome."""
        outcome = {
            "resourceType": "OperationOutcome",
            "issue": [
                {
                    "severity": "error",
                    "code": code,
                    "diagnostics": message,
                }
            ],
        }
        self._send_json(status_code, outcome)

    def _parse_body(self) -> dict[str, Any]:
        """Read and parse JSON body from request."""
        content_len_hdr = self.headers.get("Content-Length")
        if not content_len_hdr:
            return {}
        try:
            length = int(content_len_hdr)
            raw = self.rfile.read(length)
            return json.loads(raw.decode("utf-8")) if raw else {}
        except Exception as ex:
            logger.warning("Error parsing JSON body: %s", ex)
            return {}

    # -----------------------------------------------------------------------
    # HTTP GET Handler
    # -----------------------------------------------------------------------

    def do_GET(self) -> None:  # noqa: N802
        parsed_url = urllib.parse.urlparse(self.path)
        path = parsed_url.path.strip("/")
        query_params = urllib.parse.parse_qs(parsed_url.query)

        # Normalize query params from list to single strings
        params = {k: v[0] for k, v in query_params.items() if v}

        parts = path.split("/")
        # Expecting path pattern: fhir/R4/<ResourceType> or fhir/R4/<ResourceType>/<id>
        if len(parts) >= 3 and parts[0] == "fhir" and parts[1] == "R4":
            resource_type = parts[2]
            resource_id = parts[3] if len(parts) >= 4 else None

            sandbox = self.server.sandbox

            # Single resource fetch
            if resource_id is not None:
                fetchers = {
                    "Patient": sandbox.get_patient,
                    "Observation": sandbox.get_observation,
                    "Condition": sandbox.get_condition,
                    "MedicationRequest": sandbox.get_medication,
                    "AllergyIntolerance": sandbox.get_allergy,
                    "DiagnosticReport": sandbox.get_diagnostic_report,
                    "Encounter": sandbox.get_encounter,
                    "CarePlan": sandbox.get_care_plan,
                }
                fetcher = fetchers.get(resource_type)
                if not fetcher:
                    self._send_outcome(
                        404, "not-found", f"Resource type '{resource_type}' not supported"
                    )
                    return
                res = fetcher(resource_id)
                if res is None:
                    self._send_outcome(404, "not-found", f"{resource_type}/{resource_id} not found")
                    return
                self._send_json(200, res)
                return

            # Collection search / query
            count_val = (
                int(params["_count"]) if "_count" in params and params["_count"].isdigit() else None
            )
            patient_ref = params.get("patient") or params.get("subject")

            if resource_type == "Patient":
                items = sandbox.search_patients(
                    name=params.get("name"),
                    identifier=params.get("identifier"),
                    query=params.get("query"),
                    gender=params.get("gender"),
                    count=count_val,
                )
            elif resource_type == "Observation":
                items = sandbox.list_observations(
                    patient_id=patient_ref,
                    category=params.get("category"),
                    code=params.get("code"),
                    status=params.get("status"),
                    count=count_val,
                )
            elif resource_type == "Condition":
                items = sandbox.list_conditions(
                    patient_id=patient_ref,
                    count=count_val,
                )
            elif resource_type == "MedicationRequest":
                items = sandbox.list_medications(
                    patient_id=patient_ref,
                    status=params.get("status"),
                    count=count_val,
                )
            elif resource_type == "AllergyIntolerance":
                items = sandbox.list_allergies(
                    patient_id=patient_ref,
                    count=count_val,
                )
            elif resource_type == "DiagnosticReport":
                items = sandbox.list_diagnostic_reports(
                    patient_id=patient_ref,
                    code=params.get("code"),
                    count=count_val,
                )
            elif resource_type == "Encounter":
                items = sandbox.list_encounters(
                    patient_id=patient_ref,
                    status=params.get("status"),
                    count=count_val,
                )
            elif resource_type == "CarePlan":
                items = sandbox.list_care_plans(
                    patient_id=patient_ref,
                    status=params.get("status"),
                    count=count_val,
                )
            else:
                items = []

            bundle = sandbox.to_bundle(items)
            self._send_json(200, bundle)
            return

        self._send_outcome(404, "not-found", f"Endpoint not found: {self.path}")

    # -----------------------------------------------------------------------
    # HTTP POST Handler
    # -----------------------------------------------------------------------

    def do_POST(self) -> None:  # noqa: N802
        parsed_url = urllib.parse.urlparse(self.path)
        path = parsed_url.path.strip("/")

        # OAuth2 token issuance endpoint
        if path == "oauth2/token":
            token_response = {
                "access_token": "mock-medplum-token-st-jude-secure",
                "token_type": "Bearer",
                "expires_in": 3600,
                "scope": "user/*.*",
            }
            self._send_json(200, token_response)
            return

        parts = path.split("/")
        if len(parts) >= 3 and parts[0] == "fhir" and parts[1] == "R4":
            resource_type = parts[2]
            payload = self._parse_body()
            sandbox = self.server.sandbox

            try:
                if resource_type == "Observation":
                    created = sandbox.create_observation_draft(payload)
                    extra = {"Location": f"/fhir/R4/Observation/{created['id']}"}
                    self._send_json(201, created, extra_headers=extra)
                    return
                elif resource_type == "MedicationRequest":
                    created = sandbox.create_medication_draft(payload)
                    extra = {"Location": f"/fhir/R4/MedicationRequest/{created['id']}"}
                    self._send_json(201, created, extra_headers=extra)
                    return
                else:
                    self._send_outcome(
                        400,
                        "not-supported",
                        f"POST to {resource_type} not supported in mock server",
                    )
                    return
            except SafetyInvariantViolation as err:
                self._send_outcome(422, "invariant-violation", str(err))
                return
            except Exception as err:
                self._send_outcome(400, "invalid-request", str(err))
                return

        self._send_outcome(404, "not-found", f"Endpoint not found: {self.path}")


class MedplumMockServer(ThreadingHTTPServer):
    """In-process HTTP FHIR mock server backed by ClinicalSandbox."""

    def __init__(
        self,
        host: str = "127.0.0.1",
        port: int = 0,
        sandbox: ClinicalSandbox | None = None,
    ) -> None:
        self.sandbox = sandbox if sandbox is not None else ClinicalSandbox()
        self._thread: threading.Thread | None = None
        super().__init__((host, port), _MedplumMockRequestHandler)

    @property
    def host(self) -> str:
        return str(self.server_address[0])

    @property
    def port(self) -> int:
        return int(self.server_address[1])

    @property
    def base_url(self) -> str:
        return f"http://{self.host}:{self.port}"

    def start(self) -> None:
        """Start the mock server in a daemon thread."""
        self._thread = threading.Thread(target=self.serve_forever, daemon=True)
        self._thread.start()
        time.sleep(0.05)

    def stop(self) -> None:
        """Shutdown the mock server and close socket."""
        self.shutdown()
        self.server_close()
        if self._thread and self._thread.is_alive():
            self._thread.join(timeout=2.0)

    def __enter__(self) -> MedplumMockServer:
        self.start()
        return self

    def __exit__(self, exc_type: Any, exc_val: Any, exc_tb: Any) -> None:
        self.stop()
