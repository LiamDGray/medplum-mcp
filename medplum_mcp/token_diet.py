"""3-Tier HL7 FHIR Context Distillation Engine.

Provides token distillation across three detail tiers:
- compact: Primary IDs, resourceType, codes/labels, and primary value/status (>90%)
- standard: Clinical details, performers, dates, reference ranges (>80%)
- executive: High-level clinical summaries, critical flag indicators (>85%)
"""

from __future__ import annotations

from enum import Enum
from typing import Any


class DetailLevel(str, Enum):
    """Detail tier for FHIR context distillation."""

    COMPACT = "compact"
    STANDARD = "standard"
    EXECUTIVE = "executive"


def parse_detail_level(level: str | DetailLevel = DetailLevel.STANDARD) -> DetailLevel:
    """Parse and normalize detail level string or enum (case-insensitive)."""
    if level is None:
        raise ValueError(
            "Invalid detail level: None. Must be one of: 'compact', 'standard', 'executive'"
        )
    if isinstance(level, DetailLevel):
        return level
    if isinstance(level, str):
        normalized = level.strip().lower()
        try:
            return DetailLevel(normalized)
        except ValueError:
            msg = f"Invalid detail level: '{level}'. Must be: 'compact', 'standard', 'executive'"
            raise ValueError(msg) from None
    msg = f"Invalid detail level: '{level}'. Must be: 'compact', 'standard', 'executive'"
    raise ValueError(msg)


# ---------------------------------------------------------------------------
# Helper Extractors for Verbose HL7 FHIR Structures
# ---------------------------------------------------------------------------


def _extract_name(name_raw: Any) -> str:
    """Extract human-readable name from FHIR HumanName array or string."""
    if isinstance(name_raw, str):
        return name_raw
    if isinstance(name_raw, list) and name_raw:
        first = name_raw[0]
        if isinstance(first, dict):
            if "text" in first and first["text"]:
                return str(first["text"])
            given = first.get("given", [])
            family = first.get("family", "")
            if isinstance(given, list):
                given_str = " ".join(str(g) for g in given)
            else:
                given_str = str(given)
            full = f"{given_str} {family}".strip()
            if full:
                return full
    return "Unknown"


def _extract_identifier(identifiers_raw: Any) -> str:
    """Extract primary identifier / MRN from FHIR Identifier array or string."""
    if isinstance(identifiers_raw, str):
        return identifiers_raw
    if isinstance(identifiers_raw, list) and identifiers_raw:
        # Prefer MRN type if present
        for ident in identifiers_raw:
            if isinstance(ident, dict):
                id_type = ident.get("type", {})
                codings = id_type.get("coding", []) if isinstance(id_type, dict) else []
                for c in codings:
                    if isinstance(c, dict) and c.get("code") in ("MR", "MRN"):
                        val = ident.get("value")
                        if val:
                            return str(val)
        # Fallback to first non-empty value
        for ident in identifiers_raw:
            if isinstance(ident, dict) and ident.get("value"):
                return str(ident["value"])
    return "Unknown"


def _extract_code(code_raw: Any) -> str:
    """Extract display name or primary code from FHIR CodeableConcept/Coding."""
    if isinstance(code_raw, str):
        return code_raw
    if isinstance(code_raw, dict):
        if "text" in code_raw and code_raw["text"]:
            return str(code_raw["text"])
        codings = code_raw.get("coding", [])
        if isinstance(codings, list) and codings:
            first = codings[0]
            if isinstance(first, dict):
                if first.get("display"):
                    return str(first["display"])
                if first.get("code"):
                    return str(first["code"])
    return "Unknown"


def _extract_status(resource: dict[str, Any], status_key: str = "status") -> str:
    """Extract string status code from status field (handling dict or string)."""
    raw = resource.get(status_key)
    if isinstance(raw, str):
        return raw
    if isinstance(raw, dict):
        coding = raw.get("coding", [])
        if coding and isinstance(coding, list) and isinstance(coding[0], dict):
            return str(coding[0].get("code", "unknown"))
        return str(raw.get("text", "unknown"))
    return "unknown"


def _extract_telecom(telecom_raw: Any) -> list[dict[str, str]]:
    """Extract minimal contact info from FHIR ContactPoint array."""
    if not isinstance(telecom_raw, list):
        return []
    results: list[dict[str, str]] = []
    for item in telecom_raw:
        if isinstance(item, dict) and item.get("value"):
            entry: dict[str, str] = {
                "system": str(item.get("system", "phone")),
                "value": str(item["value"]),
            }
            if item.get("use"):
                entry["use"] = str(item["use"])
            results.append(entry)
    return results


def _extract_reference_range(ref_ranges: Any) -> str | None:
    """Distill reference range list to concise string."""
    if not isinstance(ref_ranges, list) or not ref_ranges:
        return None
    first = ref_ranges[0]
    if isinstance(first, dict):
        if "text" in first and first["text"]:
            return str(first["text"])
        low = first.get("low", {}).get("value")
        high = first.get("high", {}).get("value")
        unit = first.get("high", {}).get("unit") or first.get("low", {}).get("unit", "")
        if low is not None and high is not None:
            return f"{low} - {high} {unit}".strip()
        if low is not None:
            return f">= {low} {unit}".strip()
        if high is not None:
            return f"<= {high} {unit}".strip()
    return None


def _extract_interpretation(interp_raw: Any) -> str | None:
    """Distill interpretation code/display."""
    if isinstance(interp_raw, str):
        return interp_raw
    if isinstance(interp_raw, list) and interp_raw:
        return _extract_code(interp_raw[0])
    if isinstance(interp_raw, dict):
        return _extract_code(interp_raw)
    return None


def _extract_performer(performer_raw: Any) -> str | None:
    """Extract concise performer reference or display."""
    if isinstance(performer_raw, str):
        return performer_raw
    if isinstance(performer_raw, list) and performer_raw:
        first = performer_raw[0]
        if isinstance(first, dict):
            return str(first.get("display") or first.get("reference") or "Unknown")
    if isinstance(performer_raw, dict):
        return str(performer_raw.get("display") or performer_raw.get("reference") or "Unknown")
    return None


# ---------------------------------------------------------------------------
# Individual Resource Distillers
# ---------------------------------------------------------------------------


def _distill_patient(resource: dict[str, Any], level: DetailLevel) -> dict[str, Any]:
    res_id = resource.get("id", "")
    name = _extract_name(resource.get("name"))
    mrn = _extract_identifier(resource.get("identifier"))
    gender = resource.get("gender", "unknown")
    birth_date = resource.get("birthDate", "unknown")
    active = resource.get("active", True)

    if level == DetailLevel.COMPACT:
        return {
            "resourceType": "Patient",
            "id": res_id,
            "name": name,
            "identifier": mrn,
            "gender": gender,
            "birthDate": birth_date,
        }
    elif level == DetailLevel.STANDARD:
        telecom = _extract_telecom(resource.get("telecom"))
        return {
            "resourceType": "Patient",
            "id": res_id,
            "name": name,
            "identifier": mrn,
            "gender": gender,
            "birthDate": birth_date,
            "active": active,
            "telecom": telecom,
        }
    else:  # EXECUTIVE
        return {
            "resourceType": "Patient",
            "id": res_id,
            "name": name,
            "identifier": mrn,
            "gender": gender,
            "birthDate": birth_date,
            "active": active,
        }


def _distill_observation(resource: dict[str, Any], level: DetailLevel) -> dict[str, Any]:
    res_id = resource.get("id", "")
    code = _extract_code(resource.get("code"))
    status = _extract_status(resource)

    # Value & Unit extraction
    val: Any = None
    unit: str | None = None
    if "valueQuantity" in resource and isinstance(resource["valueQuantity"], dict):
        val = resource["valueQuantity"].get("value")
        unit = resource["valueQuantity"].get("unit")
    elif "valueString" in resource:
        val = resource["valueString"]
    elif "valueCodeableConcept" in resource:
        val = _extract_code(resource["valueCodeableConcept"])

    interp = _extract_interpretation(resource.get("interpretation"))
    is_critical = False
    if interp:
        interp_lower = interp.lower()
        if any(
            flag in interp_lower
            for flag in ["high", "critical", "low", "abnormal", "panic", "alert", "h"]
        ):
            is_critical = True

    if level == DetailLevel.COMPACT:
        out: dict[str, Any] = {
            "resourceType": "Observation",
            "id": res_id,
            "code": code,
            "value": val,
            "status": status,
        }
        if unit:
            out["unit"] = unit
        return out
    elif level == DetailLevel.STANDARD:
        ref_range = _extract_reference_range(resource.get("referenceRange"))
        performer = _extract_performer(resource.get("performer"))
        out = {
            "resourceType": "Observation",
            "id": res_id,
            "code": code,
            "value": val,
            "status": status,
            "effectiveDateTime": resource.get("effectiveDateTime"),
        }
        if unit:
            out["unit"] = unit
        if ref_range:
            out["referenceRange"] = ref_range
        if interp:
            out["interpretation"] = interp
        if performer:
            out["performer"] = performer
        return out
    else:  # EXECUTIVE
        out = {
            "resourceType": "Observation",
            "id": res_id,
            "code": code,
            "value": f"{val} {unit}".strip() if unit and val is not None else val,
            "status": status,
        }
        if interp:
            out["interpretation"] = interp
        if is_critical:
            out["is_critical"] = True
        return out


def _distill_condition(resource: dict[str, Any], level: DetailLevel) -> dict[str, Any]:
    res_id = resource.get("id", "")
    code = _extract_code(resource.get("code"))
    clinical_status = _extract_status(resource, "clinicalStatus")
    verification_status = _extract_status(resource, "verificationStatus")

    if level == DetailLevel.COMPACT:
        return {
            "resourceType": "Condition",
            "id": res_id,
            "code": code,
            "clinicalStatus": clinical_status,
        }
    elif level == DetailLevel.STANDARD:
        out: dict[str, Any] = {
            "resourceType": "Condition",
            "id": res_id,
            "code": code,
            "clinicalStatus": clinical_status,
            "verificationStatus": verification_status,
        }
        if resource.get("onsetDateTime"):
            out["onsetDateTime"] = resource["onsetDateTime"]
        if resource.get("category"):
            cats = resource["category"]
            if isinstance(cats, list) and cats:
                out["category"] = _extract_code(cats[0])
        return out
    else:  # EXECUTIVE
        return {
            "resourceType": "Condition",
            "id": res_id,
            "code": code,
            "clinicalStatus": clinical_status,
            "verificationStatus": verification_status,
            "is_active": clinical_status == "active",
        }


def _distill_medication_request(resource: dict[str, Any], level: DetailLevel) -> dict[str, Any]:
    res_id = resource.get("id", "")
    medication = _extract_code(
        resource.get("medicationCodeableConcept") or resource.get("medicationReference")
    )
    status = _extract_status(resource)
    intent = resource.get("intent", "order")

    if level == DetailLevel.COMPACT:
        return {
            "resourceType": "MedicationRequest",
            "id": res_id,
            "medication": medication,
            "status": status,
        }
    elif level == DetailLevel.STANDARD:
        dosage_text = ""
        dosage_instr = resource.get("dosageInstruction")
        if isinstance(dosage_instr, list) and dosage_instr:
            first_dose = dosage_instr[0]
            if isinstance(first_dose, dict):
                dosage_text = first_dose.get("text", "")
                if not dosage_text and "timing" in first_dose:
                    dosage_text = "Standard clinical dosage"

        out: dict[str, Any] = {
            "resourceType": "MedicationRequest",
            "id": res_id,
            "medication": medication,
            "status": status,
            "intent": intent,
            "dosageInstruction": dosage_text,
        }
        if resource.get("authoredOn"):
            out["authoredOn"] = resource["authoredOn"]
        return out
    else:  # EXECUTIVE
        return {
            "resourceType": "MedicationRequest",
            "id": res_id,
            "medication": medication,
            "status": status,
            "intent": intent,
        }


def _distill_allergy_intolerance(resource: dict[str, Any], level: DetailLevel) -> dict[str, Any]:
    res_id = resource.get("id", "")
    code = _extract_code(resource.get("code"))
    clinical_status = _extract_status(resource, "clinicalStatus")
    verification_status = _extract_status(resource, "verificationStatus")
    criticality = resource.get("criticality", "low")

    is_critical = criticality in ("high", "CRIT-H")

    if level == DetailLevel.COMPACT:
        return {
            "resourceType": "AllergyIntolerance",
            "id": res_id,
            "code": code,
            "clinicalStatus": clinical_status,
            "criticality": criticality,
        }
    elif level == DetailLevel.STANDARD:
        reactions: list[dict[str, str]] = []
        raw_rxns = resource.get("reaction")
        if isinstance(raw_rxns, list):
            for rxn in raw_rxns:
                if isinstance(rxn, dict):
                    manifestations = rxn.get("manifestation", [])
                    manifest_str = (
                        _extract_code(manifestations[0])
                        if isinstance(manifestations, list) and manifestations
                        else "Unknown"
                    )
                    reactions.append(
                        {
                            "manifestation": manifest_str,
                            "severity": str(rxn.get("severity", "moderate")),
                        }
                    )
        out: dict[str, Any] = {
            "resourceType": "AllergyIntolerance",
            "id": res_id,
            "code": code,
            "clinicalStatus": clinical_status,
            "verificationStatus": verification_status,
            "criticality": criticality,
            "reaction": reactions,
        }
        if resource.get("onsetDateTime"):
            out["onset"] = resource["onsetDateTime"]
        return out
    else:  # EXECUTIVE
        return {
            "resourceType": "AllergyIntolerance",
            "id": res_id,
            "code": code,
            "criticality": criticality,
            "is_critical": is_critical,
        }


def _distill_diagnostic_report(resource: dict[str, Any], level: DetailLevel) -> dict[str, Any]:
    res_id = resource.get("id", "")
    code = _extract_code(resource.get("code"))
    status = _extract_status(resource)
    conclusion = resource.get("conclusion", "")

    category = "Laboratory"
    cats = resource.get("category")
    if isinstance(cats, list) and cats:
        category = _extract_code(cats[0])

    if level == DetailLevel.COMPACT:
        return {
            "resourceType": "DiagnosticReport",
            "id": res_id,
            "code": code,
            "status": status,
        }
    elif level == DetailLevel.STANDARD:
        performer = _extract_performer(resource.get("performer"))
        out: dict[str, Any] = {
            "resourceType": "DiagnosticReport",
            "id": res_id,
            "code": code,
            "status": status,
            "category": category,
            "conclusion": conclusion,
            "effectiveDateTime": resource.get("effectiveDateTime"),
        }
        if performer:
            out["performer"] = performer
        return out
    else:  # EXECUTIVE
        return {
            "resourceType": "DiagnosticReport",
            "id": res_id,
            "code": code,
            "status": status,
            "category": category,
            "conclusion": conclusion,
        }


def _distill_encounter(resource: dict[str, Any], level: DetailLevel) -> dict[str, Any]:
    res_id = resource.get("id", "")
    status = _extract_status(resource)

    enc_class = "AMB"
    raw_class = resource.get("class")
    if isinstance(raw_class, dict):
        enc_class = str(raw_class.get("code", "AMB"))
    elif isinstance(raw_class, str):
        enc_class = raw_class

    enc_type = "Encounter"
    raw_type = resource.get("type")
    if isinstance(raw_type, list) and raw_type:
        enc_type = _extract_code(raw_type[0])

    reason = ""
    reasons = resource.get("reasonCode")
    if isinstance(reasons, list) and reasons:
        reason = _extract_code(reasons[0])

    if level == DetailLevel.COMPACT:
        return {
            "resourceType": "Encounter",
            "id": res_id,
            "status": status,
            "class": enc_class,
            "type": enc_type,
        }
    elif level == DetailLevel.STANDARD:
        out: dict[str, Any] = {
            "resourceType": "Encounter",
            "id": res_id,
            "status": status,
            "class": enc_class,
            "type": enc_type,
            "period": resource.get("period", {}),
            "reasonCode": reason,
        }
        return out
    else:  # EXECUTIVE
        return {
            "resourceType": "Encounter",
            "id": res_id,
            "status": status,
            "class": enc_class,
            "type": enc_type,
            "reason": reason,
        }


def _distill_care_plan(resource: dict[str, Any], level: DetailLevel) -> dict[str, Any]:
    res_id = resource.get("id", "")
    title = resource.get("title", _extract_code(resource.get("category", "Care Plan")))
    status = _extract_status(resource)
    intent = resource.get("intent", "plan")

    activities_list: list[str] = []
    raw_activities = resource.get("activity")
    if isinstance(raw_activities, list):
        for act in raw_activities:
            if isinstance(act, dict):
                detail = act.get("detail", {})
                if isinstance(detail, dict):
                    desc = detail.get("description") or _extract_code(detail.get("code"))
                    if desc and desc != "Unknown":
                        activities_list.append(desc)

    if level == DetailLevel.COMPACT:
        return {
            "resourceType": "CarePlan",
            "id": res_id,
            "title": title,
            "status": status,
            "intent": intent,
        }
    elif level == DetailLevel.STANDARD:
        return {
            "resourceType": "CarePlan",
            "id": res_id,
            "title": title,
            "status": status,
            "intent": intent,
            "period": resource.get("period", {}),
            "activities": activities_list,
        }
    else:  # EXECUTIVE
        return {
            "resourceType": "CarePlan",
            "id": res_id,
            "title": title,
            "status": status,
            "intent": intent,
            "activity_count": len(activities_list),
        }


def _distill_generic(resource: dict[str, Any], level: DetailLevel) -> dict[str, Any]:
    """Distill unknown FHIR resource type to a stripped base representation."""
    res_type = resource.get("resourceType", "Resource")
    res_id = resource.get("id", "")
    out: dict[str, Any] = {"resourceType": res_type, "id": res_id}
    if "status" in resource:
        out["status"] = _extract_status(resource)
    if "code" in resource:
        out["code"] = _extract_code(resource["code"])
    return out


# ---------------------------------------------------------------------------
# Public API
# ---------------------------------------------------------------------------


def distill_fhir_resource(
    resource: dict[str, Any], detail_level: str | DetailLevel = DetailLevel.STANDARD
) -> dict[str, Any]:
    """Distill a FHIR R4 resource or Bundle according to the specified detail level."""
    level = parse_detail_level(detail_level)
    res_type = resource.get("resourceType")

    if res_type == "Bundle":
        return distill_fhir_bundle(resource, level)

    distillers = {
        "Patient": _distill_patient,
        "Observation": _distill_observation,
        "Condition": _distill_condition,
        "MedicationRequest": _distill_medication_request,
        "AllergyIntolerance": _distill_allergy_intolerance,
        "DiagnosticReport": _distill_diagnostic_report,
        "Encounter": _distill_encounter,
        "CarePlan": _distill_care_plan,
    }

    distiller = distillers.get(str(res_type), _distill_generic)
    return distiller(resource, level)


def distill_fhir_bundle(
    bundle: dict[str, Any], detail_level: str | DetailLevel = DetailLevel.STANDARD
) -> dict[str, Any]:
    """Distill a FHIR Bundle and all contained resources, preserving total count."""
    level = parse_detail_level(detail_level)
    entries = bundle.get("entry", [])
    distilled_entries: list[dict[str, Any]] = []

    for entry in entries:
        if isinstance(entry, dict):
            if "resource" in entry and isinstance(entry["resource"], dict):
                dist_item: dict[str, Any] = {
                    "resource": distill_fhir_resource(entry["resource"], level),
                }
                if level != DetailLevel.COMPACT and "fullUrl" in entry:
                    dist_item["fullUrl"] = entry["fullUrl"]
                distilled_entries.append(dist_item)
            else:
                distilled_entries.append({"resource": distill_fhir_resource(entry, level)})

    total_count = bundle.get("total", len(distilled_entries))
    out: dict[str, Any] = {
        "resourceType": "Bundle",
        "type": bundle.get("type", "searchset"),
        "total": total_count,
        "entry": distilled_entries,
    }
    if "id" in bundle:
        out["id"] = bundle["id"]
    return out
