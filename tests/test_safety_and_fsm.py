"""Tests for Clinical FSM Lifecycles and Deterministic Safety Gate Invariants."""

import pytest

from medplum_mcp.fsm import (
    AllergyIntoleranceLifecycle,
    ClaimLifecycle,
    DiagnosticReportLifecycle,
    MedicationRequestLifecycle,
    ObservationLifecycle,
    compute_reachable_states,
    validate_transition,
)
from medplum_mcp.safety import (
    FORBIDDEN_CLINICAL_STATUSES,
    SafetyInvariantViolation,
    assert_write_permitted,
    normalize_status,
)


class TestClinicalEntityLifecycles:
    """Test clinical entity lifecycle enumerations and forbidden statuses."""

    def test_medication_request_lifecycle_states(self) -> None:
        assert MedicationRequestLifecycle.DRAFT.value == "draft"
        assert MedicationRequestLifecycle.ACTIVE.value == "active"
        assert MedicationRequestLifecycle.COMPLETED.value == "completed"
        assert MedicationRequestLifecycle.CANCELLED.value == "cancelled"
        assert MedicationRequestLifecycle.ENTERED_IN_ERROR.value == "entered-in-error"

    def test_allergy_intolerance_lifecycle_states(self) -> None:
        assert AllergyIntoleranceLifecycle.DRAFT.value == "draft"
        assert AllergyIntoleranceLifecycle.UNCONFIRMED.value == "unconfirmed"
        assert AllergyIntoleranceLifecycle.CONFIRMED.value == "confirmed"
        assert AllergyIntoleranceLifecycle.INACTIVE.value == "inactive"
        assert AllergyIntoleranceLifecycle.RESOLVED.value == "resolved"
        assert AllergyIntoleranceLifecycle.REFUTED.value == "refuted"

    def test_observation_lifecycle_states(self) -> None:
        assert ObservationLifecycle.REGISTERED.value == "registered"
        assert ObservationLifecycle.PRELIMINARY.value == "preliminary"
        assert ObservationLifecycle.FINAL.value == "final"
        assert ObservationLifecycle.AMENDED.value == "amended"
        assert ObservationLifecycle.CORRECTED.value == "corrected"

    def test_diagnostic_report_lifecycle_states(self) -> None:
        assert DiagnosticReportLifecycle.REGISTERED.value == "registered"
        assert DiagnosticReportLifecycle.PARTIAL.value == "partial"
        assert DiagnosticReportLifecycle.PRELIMINARY.value == "preliminary"
        assert DiagnosticReportLifecycle.FINAL.value == "final"
        assert DiagnosticReportLifecycle.AMENDED.value == "amended"
        assert DiagnosticReportLifecycle.CORRECTED.value == "corrected"

    def test_claim_lifecycle_states(self) -> None:
        assert ClaimLifecycle.DRAFT.value == "draft"
        assert ClaimLifecycle.ACTIVE.value == "active"
        assert ClaimLifecycle.CANCELLED.value == "cancelled"
        assert ClaimLifecycle.ENTERED_IN_ERROR.value == "entered-in-error"

    def test_forbidden_clinical_statuses(self) -> None:
        expected = frozenset(
            {
                "active",
                "completed",
                "final",
                "amended",
                "corrected",
                "resolved",
                "refuted",
                "entered-in-error",
                "cancelled",
            }
        )
        assert FORBIDDEN_CLINICAL_STATUSES == expected
        assert "draft" not in FORBIDDEN_CLINICAL_STATUSES
        assert "registered" not in FORBIDDEN_CLINICAL_STATUSES
        assert "preliminary" not in FORBIDDEN_CLINICAL_STATUSES
        assert "unconfirmed" not in FORBIDDEN_CLINICAL_STATUSES
        assert "partial" not in FORBIDDEN_CLINICAL_STATUSES


class TestSafetyGate:
    """Test assert_write_permitted safety gate and normalization."""

    def test_write_rejected_when_allow_writes_false(self) -> None:
        payload = {"resourceType": "MedicationRequest", "status": "draft"}
        with pytest.raises(PermissionError, match="disabled"):
            assert_write_permitted("MedicationRequest", payload, allow_writes=False)

    def test_write_permitted_when_allow_writes_true_and_draft(self) -> None:
        payload = {"resourceType": "MedicationRequest", "status": "draft"}
        result = assert_write_permitted("MedicationRequest", payload, allow_writes=True)
        assert result is True

    @pytest.mark.parametrize(
        "forbidden_status",
        [
            "active",
            "completed",
            "final",
            "amended",
            "corrected",
            "resolved",
            "refuted",
            "entered-in-error",
            "cancelled",
        ],
    )
    def test_write_rejected_on_forbidden_status(self, forbidden_status: str) -> None:
        payload = {"resourceType": "MedicationRequest", "status": forbidden_status}
        with pytest.raises(SafetyInvariantViolation):
            assert_write_permitted("MedicationRequest", payload, allow_writes=True)

    def test_write_rejected_on_nested_forbidden_status(self) -> None:
        # Nested in dictionary
        payload_nested_dict = {
            "resourceType": "Bundle",
            "entry": [{"resource": {"resourceType": "Observation", "status": "final"}}],
        }
        with pytest.raises(SafetyInvariantViolation):
            assert_write_permitted("Bundle", payload_nested_dict, allow_writes=True)

        # Nested in CodeableConcept coding
        payload_codeable = {
            "resourceType": "AllergyIntolerance",
            "clinicalStatus": {
                "coding": [
                    {
                        "system": (
                            "http://terminology.hl7.org/CodeSystem/allergyintolerance-clinical"
                        ),
                        "code": "resolved",
                    }
                ]
            },
        }
        with pytest.raises(SafetyInvariantViolation):
            assert_write_permitted("AllergyIntolerance", payload_codeable, allow_writes=True)

    def test_unicode_nfkc_normalization_fullwidth(self) -> None:
        # Full-width 'ａctive' -> U+FF41, U+FF43, U+FF54, U+FF49, U+FF56, U+FF45
        fullwidth_active = "\uff41\uff43\uff54\uff49\uff56\uff45"
        payload = {"resourceType": "MedicationRequest", "status": fullwidth_active}
        with pytest.raises(SafetyInvariantViolation):
            assert_write_permitted("MedicationRequest", payload, allow_writes=True)

    def test_unicode_cyrillic_homoglyph_immunity(self) -> None:
        # Cyrillic 'а' (U+0430) + latin 'ctive'
        cyrillic_active = "\u0430ctive"
        assert normalize_status(cyrillic_active) == "active"
        payload = {"resourceType": "MedicationRequest", "status": cyrillic_active}
        with pytest.raises(SafetyInvariantViolation):
            assert_write_permitted("MedicationRequest", payload, allow_writes=True)

        # Cyrillic 'fіnаl' (Cyrillic і U+0456, Cyrillic а U+0430)
        cyrillic_final = "f\u0456n\u0430l"
        assert normalize_status(cyrillic_final) == "final"
        payload_obs = {"resourceType": "Observation", "status": cyrillic_final}
        with pytest.raises(SafetyInvariantViolation):
            assert_write_permitted("Observation", payload_obs, allow_writes=True)


class TestFSMTransitionsAndReachability:
    """Test FSM state transitions and reachability computations."""

    def test_validate_transition_mcp_mode(self) -> None:
        # In MCP mode, transition to draft is allowed
        assert validate_transition("MedicationRequest", "draft", "draft", is_mcp=True) is True
        # In MCP mode, transition to active is blocked
        assert validate_transition("MedicationRequest", "draft", "active", is_mcp=True) is False

    def test_validate_transition_unconstrained_mode(self) -> None:
        # In vendor unconstrained mode, draft -> active is valid
        assert validate_transition("MedicationRequest", "draft", "active", is_mcp=False) is True
        # active -> completed is valid
        assert validate_transition("MedicationRequest", "active", "completed", is_mcp=False) is True
        # completed -> draft is invalid (terminal)
        assert validate_transition("MedicationRequest", "completed", "draft", is_mcp=False) is False

    def test_compute_reachable_states_mcp_mode(self) -> None:
        reachable = compute_reachable_states(
            "MedicationRequest", initial_state="draft", is_mcp=True
        )
        assert reachable == {"draft"}
        for forbidden in FORBIDDEN_CLINICAL_STATUSES:
            assert forbidden not in reachable

    def test_compute_reachable_states_unconstrained_mode(self) -> None:
        reachable = compute_reachable_states(
            "MedicationRequest", initial_state="draft", is_mcp=False
        )
        assert "active" in reachable
        assert "completed" in reachable
        assert "cancelled" in reachable
        assert "entered-in-error" in reachable


class TestDeterministicFSMReachability:
    """Test deterministic FSM reachability and Zero Unauthorized Commitment Invariants."""

    @pytest.mark.parametrize(
        "entity_type",
        [
            "MedicationRequest",
            "AllergyIntolerance",
            "Observation",
            "DiagnosticReport",
            "Claim",
        ],
    )
    def test_clinical_reachability_zero_forbidden_in_mcp_mode(self, entity_type: str) -> None:
        reachable = compute_reachable_states(entity_type, is_mcp=True)
        leak = reachable & FORBIDDEN_CLINICAL_STATUSES
        assert leak == set(), (
            f"Zero Unauthorized Commitment Invariant violated for {entity_type}: "
            f"reachable forbidden states {leak}"
        )

    def test_unconstrained_system_finds_valid_path_to_terminal_states(self) -> None:
        # Non-vacuity test: when is_mcp=False, reachability finds forbidden terminal states
        reachable = compute_reachable_states("MedicationRequest", is_mcp=False)
        terminal_states = reachable & FORBIDDEN_CLINICAL_STATUSES
        assert len(terminal_states) > 0, "Unconstrained EHR must permit terminal states"
        assert "completed" in terminal_states or "active" in terminal_states
