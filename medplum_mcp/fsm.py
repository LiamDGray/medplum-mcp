"""Finite State Machine (FSM) lifecycles and transitions for clinical entities."""

from __future__ import annotations

from enum import Enum
from typing import Any

from medplum_mcp.safety import (
    FORBIDDEN_CLINICAL_STATUSES,
    SafetyInvariantViolation,
    normalize_status,
)


class MedicationRequestLifecycle(str, Enum):
    """Lifecycle states for FHIR MedicationRequest."""

    DRAFT = "draft"
    ACTIVE = "active"
    COMPLETED = "completed"
    CANCELLED = "cancelled"
    ENTERED_IN_ERROR = "entered-in-error"


class AllergyIntoleranceLifecycle(str, Enum):
    """Lifecycle states for FHIR AllergyIntolerance."""

    DRAFT = "draft"
    UNCONFIRMED = "unconfirmed"
    CONFIRMED = "confirmed"
    INACTIVE = "inactive"
    RESOLVED = "resolved"
    REFUTED = "refuted"


class ObservationLifecycle(str, Enum):
    """Lifecycle states for FHIR Observation."""

    REGISTERED = "registered"
    PRELIMINARY = "preliminary"
    FINAL = "final"
    AMENDED = "amended"
    CORRECTED = "corrected"


class DiagnosticReportLifecycle(str, Enum):
    """Lifecycle states for FHIR DiagnosticReport."""

    REGISTERED = "registered"
    PARTIAL = "partial"
    PRELIMINARY = "preliminary"
    FINAL = "final"
    AMENDED = "amended"
    CORRECTED = "corrected"


class ClaimLifecycle(str, Enum):
    """Lifecycle states for FHIR Claim."""

    DRAFT = "draft"
    ACTIVE = "active"
    CANCELLED = "cancelled"
    ENTERED_IN_ERROR = "entered-in-error"


INITIAL_STATES: dict[str, str] = {
    "MedicationRequest": MedicationRequestLifecycle.DRAFT.value,
    "AllergyIntolerance": AllergyIntoleranceLifecycle.DRAFT.value,
    "Observation": ObservationLifecycle.REGISTERED.value,
    "DiagnosticReport": DiagnosticReportLifecycle.REGISTERED.value,
    "Claim": ClaimLifecycle.DRAFT.value,
}

TRANSITION_MAPS: dict[str, dict[str, set[str]]] = {
    "MedicationRequest": {
        "draft": {"draft", "active", "cancelled", "entered-in-error"},
        "active": {"active", "completed", "cancelled", "entered-in-error"},
        "completed": {"completed"},
        "cancelled": {"cancelled"},
        "entered-in-error": {"entered-in-error"},
    },
    "AllergyIntolerance": {
        "draft": {"draft", "unconfirmed", "confirmed"},
        "unconfirmed": {"unconfirmed", "confirmed", "refuted", "inactive", "resolved"},
        "confirmed": {"confirmed", "inactive", "resolved", "refuted"},
        "inactive": {"inactive", "resolved"},
        "resolved": {"resolved"},
        "refuted": {"refuted"},
    },
    "Observation": {
        "registered": {"registered", "preliminary", "final"},
        "preliminary": {"preliminary", "final"},
        "final": {"final", "amended", "corrected"},
        "amended": {"amended", "corrected"},
        "corrected": {"corrected", "amended"},
    },
    "DiagnosticReport": {
        "registered": {"registered", "partial", "preliminary", "final"},
        "partial": {"partial", "preliminary", "final"},
        "preliminary": {"preliminary", "final"},
        "final": {"final", "amended", "corrected"},
        "amended": {"amended", "corrected"},
        "corrected": {"corrected", "amended"},
    },
    "Claim": {
        "draft": {"draft", "active", "cancelled", "entered-in-error"},
        "active": {"active", "cancelled", "entered-in-error"},
        "cancelled": {"cancelled"},
        "entered-in-error": {"entered-in-error"},
    },
}

LIFECYCLE_CLASSES: dict[str, type[Enum]] = {
    "MedicationRequest": MedicationRequestLifecycle,
    "AllergyIntolerance": AllergyIntoleranceLifecycle,
    "Observation": ObservationLifecycle,
    "DiagnosticReport": DiagnosticReportLifecycle,
    "Claim": ClaimLifecycle,
}


def _resolve_entity_type(entity_type: Any) -> str:
    """Resolve entity type name from string or type object."""
    if isinstance(entity_type, type):
        name = entity_type.__name__
        if name.endswith("Lifecycle"):
            name = name[:-9]
        return name
    name_str = str(entity_type)
    if name_str.endswith("Lifecycle"):
        name_str = name_str[:-9]
    for standard_name in INITIAL_STATES:
        if standard_name.lower() == name_str.lower():
            return standard_name
    return name_str


def validate_transition(
    entity_type: str | type,
    current_state: str,
    target_state: str,
    is_mcp: bool = True,
    raise_on_error: bool = False,
) -> bool:
    """Validate whether a state transition is legal according to FSM and safety invariants."""
    name = _resolve_entity_type(entity_type)
    norm_cur = normalize_status(current_state)
    norm_tgt = normalize_status(target_state)

    if is_mcp and norm_tgt in FORBIDDEN_CLINICAL_STATUSES:
        if raise_on_error:
            raise SafetyInvariantViolation(
                f"Target state '{target_state}' is forbidden under MCP mode for {name}."
            )
        return False

    trans_map = TRANSITION_MAPS.get(name, {})
    allowed = trans_map.get(norm_cur, set())
    valid = norm_tgt in allowed

    if not valid and raise_on_error:
        raise ValueError(
            f"Illegal transition from '{current_state}' to '{target_state}' for {name}."
        )

    return valid


def compute_reachable_states(
    entity_type: str | type,
    initial_state: str | None = None,
    is_mcp: bool = True,
) -> set[str]:
    """Compute all reachable states from an initial state via BFS."""
    name = _resolve_entity_type(entity_type)
    if initial_state is None:
        init = INITIAL_STATES.get(name, "draft")
    else:
        init = normalize_status(initial_state)

    visited: set[str] = set()
    queue: list[str] = [init]

    trans_map = TRANSITION_MAPS.get(name, {})

    while queue:
        state = queue.pop(0)
        if state in visited:
            continue
        visited.add(state)

        for next_state in trans_map.get(state, set()):
            if is_mcp and next_state in FORBIDDEN_CLINICAL_STATUSES:
                continue
            if next_state not in visited:
                queue.append(next_state)

    return visited
