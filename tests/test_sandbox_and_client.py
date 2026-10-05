"""Tests for In-Memory Clinical Sandbox, Medplum Client, and OpenAPI Mock Server."""

from __future__ import annotations

from typing import Any

import pytest
import requests

from medplum_mcp.client import MedplumClient
from medplum_mcp.demo import ClinicalSandbox
from medplum_mcp.mock_server import MedplumMockServer
from medplum_mcp.safety import SafetyInvariantViolation
from medplum_mcp.token_diet import DetailLevel

# ===========================================================================
# 1. In-Memory Clinical Sandbox Tests
# ===========================================================================


class TestClinicalSandboxDataset:
    """Test the rich synthetic clinical dataset based on St. Jude Research Institute."""

    def test_sandbox_patient_population(self) -> None:
        sandbox = ClinicalSandbox()
        patients = sandbox.search_patients()
        assert len(patients) >= 10, f"Expected at least 10 patients, got {len(patients)}"

        # Check required FHIR attributes on all patients
        mrns: set[str] = set()
        for pat in patients:
            assert pat["resourceType"] == "Patient"
            assert "id" in pat and pat["id"].startswith("pat-")
            assert "name" in pat and len(pat["name"]) > 0
            assert "gender" in pat and pat["gender"] in {"male", "female", "other", "unknown"}
            assert "birthDate" in pat
            assert "identifier" in pat
            # Ensure unique MRNs
            for ident in pat.get("identifier", []):
                if ident.get("system") == "http://hospital.smarthealthit.org":
                    mrns.add(ident["value"])
        assert len(mrns) >= 10

    def test_sandbox_clinical_entities_exist_and_reference_patients(self) -> None:
        sandbox = ClinicalSandbox()
        patients = sandbox.search_patients()
        patient_ids = {pat["id"] for pat in patients}

        observations = sandbox.list_observations()
        conditions = sandbox.list_conditions()
        medications = sandbox.list_medications()
        allergies = sandbox.list_allergies()
        reports = sandbox.list_diagnostic_reports()
        encounters = sandbox.list_encounters()
        care_plans = sandbox.list_care_plans()

        assert len(observations) >= 20, "Should have rich lab & vitals dataset"
        assert len(conditions) >= 12, "Should have realistic clinical conditions"
        assert len(medications) >= 12, "Should have oncology/supportive medication requests"
        assert len(allergies) >= 8, "Should have comprehensive allergies"
        assert len(reports) >= 8, "Should have diagnostic reports"
        assert len(encounters) >= 8, "Should have patient encounters"
        assert len(care_plans) >= 6, "Should have treatment care plans"

        # Verify subject references point to existing patients
        for obs in observations:
            ref = obs.get("subject", {}).get("reference", "")
            pat_id = ref.split("/")[-1]
            assert pat_id in patient_ids, f"Observation subject {ref} not in patient IDs"

        for cond in conditions:
            ref = cond.get("subject", {}).get("reference", "")
            pat_id = ref.split("/")[-1]
            assert pat_id in patient_ids, f"Condition subject {ref} not in patient IDs"

    def test_search_patients_by_query_and_name(self) -> None:
        sandbox = ClinicalSandbox()
        # Search by given name
        results = sandbox.search_patients(name="Caleb")
        assert len(results) >= 1
        assert any("Caleb" in str(r.get("name")) for r in results)

        # Search by full-text query
        results_query = sandbox.search_patients(query="Vance")
        assert len(results_query) >= 1
        assert results_query[0]["id"] == "pat-sj-001"

        # Search nonexistent
        results_none = sandbox.search_patients(query="NonexistentPatientName123")
        assert len(results_none) == 0

    def test_get_patient(self) -> None:
        sandbox = ClinicalSandbox()
        pat = sandbox.get_patient("pat-sj-001")
        assert pat is not None
        assert pat["id"] == "pat-sj-001"
        assert sandbox.get_patient("pat-unknown-999") is None

    def test_list_and_get_observations(self) -> None:
        sandbox = ClinicalSandbox()
        pat_obs = sandbox.list_observations(patient_id="pat-sj-001")
        assert len(pat_obs) >= 2
        first_id = pat_obs[0]["id"]
        single_obs = sandbox.get_observation(first_id)
        assert single_obs is not None
        assert single_obs["id"] == first_id

    def test_create_observation_draft_success(self) -> None:
        sandbox = ClinicalSandbox()
        new_obs = {
            "resourceType": "Observation",
            "status": "registered",
            "code": {
                "coding": [
                    {
                        "system": "http://loinc.org",
                        "code": "26499-4",
                        "display": "Absolute Neutrophil Count",
                    }
                ]
            },
            "subject": {"reference": "Patient/pat-sj-001"},
            "valueQuantity": {"value": 1500, "unit": "/uL"},
        }
        created = sandbox.create_observation_draft(new_obs)
        assert created["id"] is not None
        assert created["status"] == "registered"
        assert sandbox.get_observation(created["id"]) is not None

    def test_create_observation_draft_rejects_forbidden_status(self) -> None:
        sandbox = ClinicalSandbox()
        forbidden_obs = {
            "resourceType": "Observation",
            "status": "final",  # Forbidden for MCP writes
            "code": {"text": "WBC"},
            "subject": {"reference": "Patient/pat-sj-001"},
        }
        with pytest.raises(SafetyInvariantViolation):
            sandbox.create_observation_draft(forbidden_obs)

    def test_list_and_get_medications(self) -> None:
        sandbox = ClinicalSandbox()
        pat_meds = sandbox.list_medications(patient_id="pat-sj-001")
        assert len(pat_meds) >= 1
        first_id = pat_meds[0]["id"]
        single_med = sandbox.get_medication(first_id)
        assert single_med is not None
        assert single_med["id"] == first_id

    def test_create_medication_draft_success(self) -> None:
        sandbox = ClinicalSandbox()
        new_med = {
            "resourceType": "MedicationRequest",
            "status": "draft",
            "intent": "order",
            "subject": {"reference": "Patient/pat-sj-001"},
            "medicationCodeableConcept": {
                "coding": [
                    {
                        "system": "http://www.nlm.nih.gov/research/umls/rxnorm",
                        "code": "115113",
                        "display": "Vincristine 1 MG/ML Injectable Solution",
                    }
                ]
            },
        }
        created = sandbox.create_medication_draft(new_med)
        assert created["id"] is not None
        assert created["status"] == "draft"
        assert sandbox.get_medication(created["id"]) is not None

    def test_create_medication_draft_rejects_forbidden_status(self) -> None:
        sandbox = ClinicalSandbox()
        forbidden_med = {
            "resourceType": "MedicationRequest",
            "status": "active",  # Forbidden for MCP writes
            "intent": "order",
            "subject": {"reference": "Patient/pat-sj-001"},
        }
        with pytest.raises(SafetyInvariantViolation):
            sandbox.create_medication_draft(forbidden_med)

    def test_list_and_get_conditions(self) -> None:
        sandbox = ClinicalSandbox()
        conds = sandbox.list_conditions(patient_id="pat-sj-001")
        assert len(conds) >= 1
        first_id = conds[0]["id"]
        assert sandbox.get_condition(first_id) is not None

    def test_list_and_get_allergies(self) -> None:
        sandbox = ClinicalSandbox()
        allergies = sandbox.list_allergies(patient_id="pat-sj-001")
        assert len(allergies) >= 1
        first_id = allergies[0]["id"]
        assert sandbox.get_allergy(first_id) is not None

    def test_list_and_get_diagnostic_reports(self) -> None:
        sandbox = ClinicalSandbox()
        reports = sandbox.list_diagnostic_reports(patient_id="pat-sj-001")
        assert len(reports) >= 1
        first_id = reports[0]["id"]
        assert sandbox.get_diagnostic_report(first_id) is not None

    def test_list_encounters_and_care_plans(self) -> None:
        sandbox = ClinicalSandbox()
        encounters = sandbox.list_encounters(patient_id="pat-sj-001")
        assert len(encounters) >= 1
        care_plans = sandbox.list_care_plans(patient_id="pat-sj-001")
        assert len(care_plans) >= 1

    def test_sandbox_reset(self) -> None:
        sandbox = ClinicalSandbox()
        new_obs = {
            "resourceType": "Observation",
            "status": "registered",
            "code": {"text": "Temporary Lab"},
            "subject": {"reference": "Patient/pat-sj-001"},
        }
        created = sandbox.create_observation_draft(new_obs)
        assert sandbox.get_observation(created["id"]) is not None
        sandbox.reset()
        assert sandbox.get_observation(created["id"]) is None


# ===========================================================================
# 2. Resilient Client in Demo Mode Tests
# ===========================================================================


class TestMedplumClientDemoMode:
    """Test MedplumClient operating in zero-latency demo mode with token distillation."""

    def test_demo_client_queries_patients_with_distillation(self) -> None:
        client = MedplumClient(demo_mode=True)
        # Compact tier
        bundle_compact = client.search_patients(detail_level=DetailLevel.COMPACT)
        assert bundle_compact["resourceType"] == "Bundle"
        assert bundle_compact["total"] >= 10
        first_entry = bundle_compact["entry"][0]["resource"]
        assert "name" in first_entry
        assert "meta" not in first_entry
        assert "telecom" not in first_entry

        # Standard tier
        bundle_std = client.search_patients(detail_level="standard")
        assert bundle_std["resourceType"] == "Bundle"
        first_std = bundle_std["entry"][0]["resource"]
        assert "gender" in first_std

        # Executive tier
        bundle_exec = client.search_patients(detail_level="executive")
        assert bundle_exec["resourceType"] == "Bundle"

    def test_demo_client_get_patient(self) -> None:
        client = MedplumClient(demo_mode=True)
        patient = client.get_patient("pat-sj-001", detail_level="compact")
        assert patient is not None
        assert patient["id"] == "pat-sj-001"
        assert "Caleb" in patient["name"]

    def test_demo_client_queries_clinical_resources(self) -> None:
        client = MedplumClient(demo_mode=True)
        obs_bundle = client.list_observations(
            patient_id="pat-sj-001", detail_level=DetailLevel.COMPACT
        )
        assert obs_bundle["resourceType"] == "Bundle"
        assert len(obs_bundle["entry"]) >= 2

        med_bundle = client.list_medications(
            patient_id="pat-sj-001", detail_level=DetailLevel.COMPACT
        )
        assert med_bundle["resourceType"] == "Bundle"

        cond_bundle = client.list_conditions(
            patient_id="pat-sj-001", detail_level=DetailLevel.COMPACT
        )
        assert cond_bundle["resourceType"] == "Bundle"

        all_bundle = client.list_allergies(
            patient_id="pat-sj-001", detail_level=DetailLevel.COMPACT
        )
        assert all_bundle["resourceType"] == "Bundle"

        rep_bundle = client.list_diagnostic_reports(
            patient_id="pat-sj-001", detail_level=DetailLevel.COMPACT
        )
        assert rep_bundle["resourceType"] == "Bundle"

    def test_demo_client_creates_draft_with_safety_gates(self) -> None:
        client = MedplumClient(demo_mode=True)

        # Write rejected when allow_writes=False
        draft_obs = {
            "resourceType": "Observation",
            "status": "registered",
            "code": {"text": "Platelets"},
            "subject": {"reference": "Patient/pat-sj-001"},
        }
        with pytest.raises(PermissionError):
            client.create_observation(draft_obs, allow_writes=False)

        # Write rejected when status is forbidden even with allow_writes=True
        forbidden_obs = {
            "resourceType": "Observation",
            "status": "final",
            "code": {"text": "Platelets"},
            "subject": {"reference": "Patient/pat-sj-001"},
        }
        with pytest.raises(SafetyInvariantViolation):
            client.create_observation(forbidden_obs, allow_writes=True)

        # Write permitted when allow_writes=True and status is draft/registered
        created = client.create_observation(draft_obs, allow_writes=True)
        assert created["id"] is not None
        assert created["status"] == "registered"

    def test_demo_client_creates_medication_draft_safety(self) -> None:
        client = MedplumClient(demo_mode=True)
        draft_med = {
            "resourceType": "MedicationRequest",
            "status": "draft",
            "intent": "order",
            "subject": {"reference": "Patient/pat-sj-001"},
            "medicationCodeableConcept": {"text": "Methotrexate"},
        }
        with pytest.raises(PermissionError):
            client.create_medication_request(draft_med, allow_writes=False)

        forbidden_med = {
            "resourceType": "MedicationRequest",
            "status": "active",
            "intent": "order",
            "subject": {"reference": "Patient/pat-sj-001"},
        }
        with pytest.raises(SafetyInvariantViolation):
            client.create_medication_request(forbidden_med, allow_writes=True)

        created = client.create_medication_request(draft_med, allow_writes=True)
        assert created["id"] is not None
        assert created["status"] == "draft"


# ===========================================================================
# 3. OpenAPI Mock Server & Live Client Tests
# ===========================================================================


@pytest.fixture(scope="module")
def mock_server() -> Any:
    server = MedplumMockServer(port=0)
    server.start()
    yield server
    server.stop()


class TestMockServerAndLiveClient:
    """Test standard FHIR R4 HTTP endpoints on in-process MedplumMockServer."""

    def test_mock_server_rate_limit_headers_and_oauth(self, mock_server: MedplumMockServer) -> None:
        base_url = mock_server.base_url
        resp = requests.post(f"{base_url}/oauth2/token", json={"grant_type": "client_credentials"})
        assert resp.status_code == 200
        assert resp.headers.get("X-Rate-Limit-Limit") == "100"
        assert resp.headers.get("X-Rate-Limit-Remaining") == "99"
        data = resp.json()
        assert "access_token" in data
        assert data["token_type"] == "Bearer"

    def test_live_client_search_patients(self, mock_server: MedplumMockServer) -> None:
        client = MedplumClient(
            base_url=mock_server.base_url,
            access_token="test-bearer-token",
            demo_mode=False,
        )
        bundle = client.search_patients(name="Caleb", detail_level="compact")
        assert bundle["resourceType"] == "Bundle"
        assert bundle["total"] >= 1
        entry = bundle["entry"][0]["resource"]
        assert "Caleb" in entry["name"]

    def test_live_client_get_patient_and_not_found(self, mock_server: MedplumMockServer) -> None:
        client = MedplumClient(
            base_url=mock_server.base_url,
            access_token="test-bearer-token",
            demo_mode=False,
        )
        pat = client.get_patient("pat-sj-001")
        assert pat is not None
        assert pat["id"] == "pat-sj-001"

        pat_missing = client.get_patient("pat-missing-999")
        assert pat_missing is None

    def test_live_client_list_observations_and_medications(
        self, mock_server: MedplumMockServer
    ) -> None:
        client = MedplumClient(
            base_url=mock_server.base_url,
            access_token="test-bearer-token",
            demo_mode=False,
        )
        obs_bundle = client.list_observations(patient_id="pat-sj-001")
        assert obs_bundle["resourceType"] == "Bundle"
        assert len(obs_bundle["entry"]) >= 2

        med_bundle = client.list_medications(patient_id="pat-sj-001")
        assert med_bundle["resourceType"] == "Bundle"
        assert len(med_bundle["entry"]) >= 1

    def test_live_client_create_observation_draft(self, mock_server: MedplumMockServer) -> None:
        client = MedplumClient(
            base_url=mock_server.base_url,
            access_token="test-bearer-token",
            demo_mode=False,
        )
        draft_obs = {
            "resourceType": "Observation",
            "status": "registered",
            "code": {"text": "Minimal Residual Disease (MRD)"},
            "subject": {"reference": "Patient/pat-sj-001"},
            "valueString": "Negative (<0.01%)",
        }

        # Client boundary rejects before network call
        with pytest.raises(PermissionError):
            client.create_observation(draft_obs, allow_writes=False)

        # Client boundary rejects forbidden status
        forbidden_obs = dict(draft_obs)
        forbidden_obs["status"] = "final"
        with pytest.raises(SafetyInvariantViolation):
            client.create_observation(forbidden_obs, allow_writes=True)

        # Successful creation over HTTP
        created = client.create_observation(draft_obs, allow_writes=True)
        assert created["id"] is not None
        assert created["status"] == "registered"

    def test_mock_server_direct_http_rejects_forbidden_status(
        self, mock_server: MedplumMockServer
    ) -> None:
        base_url = mock_server.base_url
        forbidden_obs = {
            "resourceType": "Observation",
            "status": "final",
            "code": {"text": "Illegal Observation"},
            "subject": {"reference": "Patient/pat-sj-001"},
        }
        resp = requests.post(f"{base_url}/fhir/R4/Observation", json=forbidden_obs)
        assert resp.status_code in {400, 422}
        outcome = resp.json()
        assert outcome["resourceType"] == "OperationOutcome"

    def test_mock_server_context_manager(self) -> None:
        with MedplumMockServer(port=0) as temp_server:
            resp = requests.get(f"{temp_server.base_url}/fhir/R4/Patient/pat-sj-001")
            assert resp.status_code == 200

    def test_client_with_secret_string_token(self, mock_server: MedplumMockServer) -> None:
        from medplum_mcp.vault import SecretString

        client = MedplumClient(
            base_url=mock_server.base_url,
            access_token=SecretString("mcp-secure-vault-token"),
            demo_mode=False,
        )
        pat = client.get_patient("pat-sj-001")
        assert pat is not None

    def test_client_retry_on_503(self) -> None:
        from unittest.mock import MagicMock

        client = MedplumClient(
            base_url="http://mock.example.com",
            demo_mode=False,
            max_retries=2,
            backoff_factor=0.01,
        )
        mock_response_503 = MagicMock()
        mock_response_503.status_code = 503
        mock_response_503.headers = {}

        mock_response_200 = MagicMock()
        mock_response_200.status_code = 200
        mock_response_200.headers = {}
        mock_response_200.json.return_value = {"resourceType": "Patient", "id": "pat-1"}

        client._session.request = MagicMock(side_effect=[mock_response_503, mock_response_200])  # type: ignore[method-assign]
        res = client.get_patient("pat-1")
        assert res is not None
        assert res["id"] == "pat-1"
        assert client._session.request.call_count == 2

    def test_live_client_authenticate_oauth2_and_pkce(self, mock_server: MedplumMockServer) -> None:
        # 1. Client credentials flow
        client = MedplumClient.authenticate_oauth2(
            base_url=mock_server.base_url,
            client_id="test-client-id",
            client_secret="test-client-secret",
        )
        assert client._get_token_value() == "mock-medplum-token-st-jude-secure"
        assert client.base_url == mock_server.base_url

        # Check authenticated query works
        bundle = client.search_patients()
        assert bundle["resourceType"] == "Bundle"
        assert bundle["total"] >= 10

        # 2. PKCE code exchange flow
        pkce_client = MedplumClient.exchange_code_pkce(
            base_url=mock_server.base_url,
            code="test-code-123",
            code_verifier="test-verifier-456",
            redirect_uri="http://localhost/cb",
        )
        assert pkce_client._get_token_value() == "mock-medplum-token-st-jude-secure"
