//! In-situ Borrowed SIMD Distillation Engine
//!
//! Provides ultra-low-latency in-situ JSON parsing and token distillation using SIMD instructions.
//! Eliminates heap allocations and intermediate copies by borrowing string slices directly
//! from the input JSON byte buffer (`&'a str`).

use crate::token_diet::{DetailLevel, TokenDietError};
use simd_json::prelude::*;
use simd_json::BorrowedValue;
use std::borrow::Cow;

/// Borrowed distilled Patient representation pointing into in-situ JSON memory.
#[derive(Debug, Clone, PartialEq)]
pub struct SimdDistilledPatient<'a> {
    pub id: &'a str,
    pub name: Cow<'a, str>,
    pub identifier: Option<&'a str>,
    pub gender: Option<&'a str>,
    pub birth_date: Option<&'a str>,
}

/// Borrowed distilled Observation representation pointing into in-situ JSON memory.
#[derive(Debug, Clone, PartialEq)]
pub struct SimdDistilledObservation<'a> {
    pub id: &'a str,
    pub status: &'a str,
    pub code: &'a str,
    pub code_display: Option<&'a str>,
    pub value_quantity: Option<f64>,
    pub value_string: Option<&'a str>,
    pub unit: Option<&'a str>,
    pub effective_date: Option<&'a str>,
}

/// Borrowed distilled MedicationRequest representation pointing into in-situ JSON memory.
#[derive(Debug, Clone, PartialEq)]
pub struct SimdDistilledMedicationRequest<'a> {
    pub id: &'a str,
    pub status: &'a str,
    pub intent: &'a str,
    pub medication: Cow<'a, str>,
    pub patient_id: Option<&'a str>,
    pub authored_on: Option<&'a str>,
}

/// Generic borrowed distilled resource representation.
#[derive(Debug, Clone, PartialEq)]
pub struct SimdDistilledGeneric<'a> {
    pub resource_type: &'a str,
    pub id: &'a str,
}

/// Enum containing the distilled resource with zero heap-allocated strings where possible.
#[derive(Debug, Clone, PartialEq)]
pub enum SimdDistilledResource<'a> {
    Patient(SimdDistilledPatient<'a>),
    Observation(SimdDistilledObservation<'a>),
    MedicationRequest(SimdDistilledMedicationRequest<'a>),
    Generic(SimdDistilledGeneric<'a>),
}

impl<'a> SimdDistilledResource<'a> {
    /// Return the FHIR resourceType.
    pub fn resource_type(&self) -> &'a str {
        match self {
            SimdDistilledResource::Patient(_) => "Patient",
            SimdDistilledResource::Observation(_) => "Observation",
            SimdDistilledResource::MedicationRequest(_) => "MedicationRequest",
            SimdDistilledResource::Generic(g) => g.resource_type,
        }
    }

    /// Return the resource ID.
    pub fn resource_id(&self) -> &'a str {
        match self {
            SimdDistilledResource::Patient(p) => p.id,
            SimdDistilledResource::Observation(o) => o.id,
            SimdDistilledResource::MedicationRequest(m) => m.id,
            SimdDistilledResource::Generic(g) => g.id,
        }
    }

    /// Convert the borrowed SIMD representation into a canonical serde_json::Value.
    pub fn to_value(&self) -> serde_json::Value {
        match self {
            SimdDistilledResource::Patient(p) => {
                let mut map = serde_json::Map::new();
                map.insert(
                    "resourceType".to_string(),
                    serde_json::Value::String("Patient".to_string()),
                );
                map.insert(
                    "id".to_string(),
                    serde_json::Value::String(p.id.to_string()),
                );
                map.insert(
                    "name".to_string(),
                    serde_json::Value::String(p.name.to_string()),
                );
                if let Some(gender) = p.gender {
                    map.insert(
                        "gender".to_string(),
                        serde_json::Value::String(gender.to_string()),
                    );
                }
                if let Some(dob) = p.birth_date {
                    map.insert(
                        "birthDate".to_string(),
                        serde_json::Value::String(dob.to_string()),
                    );
                }
                if let Some(mrn) = p.identifier {
                    map.insert(
                        "identifier".to_string(),
                        serde_json::Value::String(mrn.to_string()),
                    );
                }
                serde_json::Value::Object(map)
            }
            SimdDistilledResource::Observation(o) => {
                let mut map = serde_json::Map::new();
                map.insert(
                    "resourceType".to_string(),
                    serde_json::Value::String("Observation".to_string()),
                );
                map.insert(
                    "id".to_string(),
                    serde_json::Value::String(o.id.to_string()),
                );
                map.insert(
                    "status".to_string(),
                    serde_json::Value::String(o.status.to_string()),
                );
                map.insert(
                    "code".to_string(),
                    serde_json::Value::String(o.code.to_string()),
                );
                if let Some(cd) = o.code_display {
                    map.insert(
                        "codeDisplay".to_string(),
                        serde_json::Value::String(cd.to_string()),
                    );
                }
                if let Some(vq) = o.value_quantity {
                    map.insert("value".to_string(), serde_json::json!(vq));
                } else if let Some(vs) = o.value_string {
                    map.insert(
                        "value".to_string(),
                        serde_json::Value::String(vs.to_string()),
                    );
                }
                if let Some(u) = o.unit {
                    map.insert("unit".to_string(), serde_json::Value::String(u.to_string()));
                }
                if let Some(ed) = o.effective_date {
                    map.insert(
                        "effectiveDateTime".to_string(),
                        serde_json::Value::String(ed.to_string()),
                    );
                }
                serde_json::Value::Object(map)
            }
            SimdDistilledResource::MedicationRequest(m) => {
                let mut map = serde_json::Map::new();
                map.insert(
                    "resourceType".to_string(),
                    serde_json::Value::String("MedicationRequest".to_string()),
                );
                map.insert(
                    "id".to_string(),
                    serde_json::Value::String(m.id.to_string()),
                );
                map.insert(
                    "status".to_string(),
                    serde_json::Value::String(m.status.to_string()),
                );
                map.insert(
                    "intent".to_string(),
                    serde_json::Value::String(m.intent.to_string()),
                );
                map.insert(
                    "medication".to_string(),
                    serde_json::Value::String(m.medication.to_string()),
                );
                if let Some(pid) = m.patient_id {
                    map.insert(
                        "patientId".to_string(),
                        serde_json::Value::String(pid.to_string()),
                    );
                }
                if let Some(ao) = m.authored_on {
                    map.insert(
                        "authoredOn".to_string(),
                        serde_json::Value::String(ao.to_string()),
                    );
                }
                serde_json::Value::Object(map)
            }
            SimdDistilledResource::Generic(g) => {
                serde_json::json!({
                    "resourceType": g.resource_type,
                    "id": g.id,
                })
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Zero-Copy Extraction Helpers
// ---------------------------------------------------------------------------

#[inline]
fn get_borrowed_str<'a>(val: &BorrowedValue<'a>) -> Option<&'a str> {
    match val {
        BorrowedValue::String(Cow::Borrowed(s)) => Some(*s),
        _ => None,
    }
}

#[inline]
fn get_field<'b, 'a>(val: &'b BorrowedValue<'a>, key: &str) -> Option<&'b BorrowedValue<'a>> {
    match val {
        BorrowedValue::Object(map) => map.get(key),
        _ => None,
    }
}

#[inline]
fn get_str_field<'a>(val: &BorrowedValue<'a>, key: &str) -> Option<&'a str> {
    get_field(val, key).and_then(get_borrowed_str)
}

#[inline]
fn get_f64_field(val: &BorrowedValue<'_>, key: &str) -> Option<f64> {
    get_field(val, key).and_then(|v| v.as_f64())
}

/// Distill raw FHIR JSON in-situ using SIMD parsing into borrowed representations.
pub fn distill_resource_simd<'a>(
    raw_json: &'a mut [u8],
    _level: DetailLevel,
) -> Result<SimdDistilledResource<'a>, TokenDietError> {
    let borrowed = simd_json::to_borrowed_value(raw_json)
        .map_err(|e| TokenDietError::InvalidDetailLevel(e.to_string()))?;

    let resource_type = get_str_field(&borrowed, "resourceType").unwrap_or("Unknown");
    let id = get_str_field(&borrowed, "id").unwrap_or("");

    match resource_type {
        "Patient" => {
            let name = extract_simd_patient_name(&borrowed);
            let identifier = extract_simd_patient_mrn(&borrowed);
            let gender = get_str_field(&borrowed, "gender");
            let birth_date = get_str_field(&borrowed, "birthDate");

            Ok(SimdDistilledResource::Patient(SimdDistilledPatient {
                id,
                name,
                identifier,
                gender,
                birth_date,
            }))
        }
        "Observation" => {
            let status = get_str_field(&borrowed, "status").unwrap_or("unknown");
            let (code, code_display) = extract_simd_code(&borrowed);
            let (val_num, val_str, unit) = extract_simd_observation_value(&borrowed);
            let effective_date = get_str_field(&borrowed, "effectiveDateTime");

            Ok(SimdDistilledResource::Observation(
                SimdDistilledObservation {
                    id,
                    status,
                    code,
                    code_display,
                    value_quantity: val_num,
                    value_string: val_str,
                    unit,
                    effective_date,
                },
            ))
        }
        "MedicationRequest" => {
            let status = get_str_field(&borrowed, "status").unwrap_or("draft");
            let intent = get_str_field(&borrowed, "intent").unwrap_or("order");
            let medication = extract_simd_medication(&borrowed);
            let patient_id =
                get_field(&borrowed, "subject").and_then(|s| get_str_field(s, "reference"));
            let authored_on = get_str_field(&borrowed, "authoredOn");

            Ok(SimdDistilledResource::MedicationRequest(
                SimdDistilledMedicationRequest {
                    id,
                    status,
                    intent,
                    medication,
                    patient_id,
                    authored_on,
                },
            ))
        }
        _ => Ok(SimdDistilledResource::Generic(SimdDistilledGeneric {
            resource_type,
            id,
        })),
    }
}

fn extract_simd_patient_name<'a>(v: &BorrowedValue<'a>) -> Cow<'a, str> {
    if let Some(name_val) = get_field(v, "name") {
        if let Some(s) = get_borrowed_str(name_val) {
            return Cow::Borrowed(s);
        }
        if let BorrowedValue::Array(arr) = name_val {
            if let Some(first) = arr.first() {
                if let Some(text) = get_str_field(first, "text") {
                    if !text.is_empty() {
                        return Cow::Borrowed(text);
                    }
                }
                let family = get_str_field(first, "family").unwrap_or("");
                let mut given_joined = String::new();
                if let Some(BorrowedValue::Array(givens)) = get_field(first, "given") {
                    for (i, g) in givens.iter().filter_map(get_borrowed_str).enumerate() {
                        if i > 0 {
                            given_joined.push(' ');
                        }
                        given_joined.push_str(g);
                    }
                }
                if !family.is_empty() && !given_joined.is_empty() {
                    return Cow::Owned(format!("{family}, {given_joined}"));
                } else if !family.is_empty() {
                    return Cow::Borrowed(family);
                } else if !given_joined.is_empty() {
                    return Cow::Owned(given_joined);
                }
            }
        }
    }
    Cow::Borrowed("Unknown")
}

fn extract_simd_patient_mrn<'a>(v: &BorrowedValue<'a>) -> Option<&'a str> {
    if let Some(BorrowedValue::Array(arr)) = get_field(v, "identifier") {
        for item in arr.iter() {
            if let Some(type_obj) = get_field(item, "type") {
                if let Some(BorrowedValue::Array(codings)) = get_field(type_obj, "coding") {
                    for c in codings.iter() {
                        if let Some(code) = get_str_field(c, "code") {
                            if code == "MR" || code == "MRN" {
                                if let Some(val) = get_str_field(item, "value") {
                                    return Some(val);
                                }
                            }
                        }
                    }
                }
            }
        }
        for item in arr.iter() {
            if let Some(val) = get_str_field(item, "value") {
                if !val.is_empty() {
                    return Some(val);
                }
            }
        }
    }
    None
}

fn extract_simd_code<'a>(v: &BorrowedValue<'a>) -> (&'a str, Option<&'a str>) {
    if let Some(code_val) = get_field(v, "code") {
        if let Some(BorrowedValue::Array(codings)) = get_field(code_val, "coding") {
            if let Some(first) = codings.first() {
                let code = get_str_field(first, "code").unwrap_or("Unknown");
                let display = get_str_field(first, "display");
                return (code, display);
            }
        }
        if let Some(text) = get_str_field(code_val, "text") {
            return (text, None);
        }
    }
    ("Unknown", None)
}

fn extract_simd_observation_value<'a>(
    v: &BorrowedValue<'a>,
) -> (Option<f64>, Option<&'a str>, Option<&'a str>) {
    if let Some(vq) = get_field(v, "valueQuantity") {
        let val_num = get_f64_field(vq, "value");
        let unit = get_str_field(vq, "unit");
        return (val_num, None, unit);
    }
    if let Some(vs) = get_str_field(v, "valueString") {
        return (None, Some(vs), None);
    }
    (None, None, None)
}

fn extract_simd_medication<'a>(v: &BorrowedValue<'a>) -> Cow<'a, str> {
    if let Some(concept) = get_field(v, "medicationCodeableConcept") {
        if let Some(text) = get_str_field(concept, "text") {
            return Cow::Borrowed(text);
        }
        if let Some(BorrowedValue::Array(codings)) = get_field(concept, "coding") {
            if let Some(first) = codings.first() {
                if let Some(disp) = get_str_field(first, "display") {
                    return Cow::Borrowed(disp);
                }
            }
        }
    }
    if let Some(med_ref) = get_field(v, "medicationReference") {
        if let Some(disp) = get_str_field(med_ref, "display") {
            return Cow::Borrowed(disp);
        }
    }
    Cow::Borrowed("Unknown Medication")
}
