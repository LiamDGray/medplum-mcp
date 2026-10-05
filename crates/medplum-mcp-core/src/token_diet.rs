//! 3-Tier HL7 FHIR Context Distillation Engine.
//!
//! Provides token distillation across three detail tiers:
//! - compact: Primary IDs, resourceType, codes/labels, and primary value/status (>90%)
//! - standard: Clinical details, performers, dates, reference ranges (>80%)
//! - executive: High-level clinical summaries, critical flag indicators (>85%)

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use thiserror::Error;

/// Detail tier for FHIR context distillation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DetailLevel {
    Compact,
    Standard,
    Executive,
}

impl DetailLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            DetailLevel::Compact => "compact",
            DetailLevel::Standard => "standard",
            DetailLevel::Executive => "executive",
        }
    }
}

impl fmt::Display for DetailLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TokenDietError {
    #[error("Invalid detail level: '{0}'. Must be one of: 'compact', 'standard', 'executive'")]
    InvalidDetailLevel(String),
    #[error("Unsupported resource type for SIMD distillation: '{0}'")]
    UnsupportedResourceType(String),
}

impl FromStr for DetailLevel {
    type Err = TokenDietError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim().to_lowercase();
        match trimmed.as_str() {
            "compact" => Ok(DetailLevel::Compact),
            "standard" => Ok(DetailLevel::Standard),
            "executive" => Ok(DetailLevel::Executive),
            _ => Err(TokenDietError::InvalidDetailLevel(s.to_string())),
        }
    }
}

// ---------------------------------------------------------------------------
// Helper Extractors
// ---------------------------------------------------------------------------

fn extract_name(name_raw: Option<&Value>) -> String {
    let Some(val) = name_raw else {
        return "Unknown".to_string();
    };

    if let Some(s) = val.as_str() {
        return s.to_string();
    }

    if let Some(arr) = val.as_array() {
        if let Some(first) = arr.first() {
            if let Some(obj) = first.as_object() {
                if let Some(text) = obj.get("text").and_then(Value::as_str) {
                    if !text.is_empty() {
                        return text.to_string();
                    }
                }
                let mut given_str = String::new();
                if let Some(given) = obj.get("given") {
                    if let Some(g_arr) = given.as_array() {
                        let parts: Vec<&str> = g_arr.iter().filter_map(Value::as_str).collect();
                        given_str = parts.join(" ");
                    } else if let Some(g_str) = given.as_str() {
                        given_str = g_str.to_string();
                    }
                }
                let family = obj
                    .get("family")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim();
                let full = if given_str.is_empty() {
                    family.to_string()
                } else if family.is_empty() {
                    given_str
                } else {
                    format!("{given_str} {family}")
                };
                if !full.is_empty() {
                    return full;
                }
            }
        }
    }

    "Unknown".to_string()
}

fn extract_identifier(ident_raw: Option<&Value>) -> String {
    let Some(val) = ident_raw else {
        return "Unknown".to_string();
    };

    if let Some(s) = val.as_str() {
        return s.to_string();
    }

    if let Some(arr) = val.as_array() {
        // Prefer MR or MRN code
        for item in arr {
            if let Some(obj) = item.as_object() {
                if let Some(type_obj) = obj.get("type").and_then(Value::as_object) {
                    if let Some(codings) = type_obj.get("coding").and_then(Value::as_array) {
                        for c in codings {
                            if let Some(code) = c.get("code").and_then(Value::as_str) {
                                if code == "MR" || code == "MRN" {
                                    if let Some(v) = obj.get("value").and_then(Value::as_str) {
                                        return v.to_string();
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        // Fallback to first non-empty value
        for item in arr {
            if let Some(v) = item.get("value").and_then(Value::as_str) {
                if !v.is_empty() {
                    return v.to_string();
                }
            }
        }
    }

    "Unknown".to_string()
}

fn extract_code(code_raw: Option<&Value>) -> String {
    let Some(val) = code_raw else {
        return "Unknown".to_string();
    };

    if let Some(s) = val.as_str() {
        return s.to_string();
    }

    if let Some(obj) = val.as_object() {
        if let Some(text) = obj.get("text").and_then(Value::as_str) {
            if !text.is_empty() {
                return text.to_string();
            }
        }
        if let Some(codings) = obj.get("coding").and_then(Value::as_array) {
            if let Some(first) = codings.first() {
                if let Some(disp) = first.get("display").and_then(Value::as_str) {
                    if !disp.is_empty() {
                        return disp.to_string();
                    }
                }
                if let Some(c) = first.get("code").and_then(Value::as_str) {
                    if !c.is_empty() {
                        return c.to_string();
                    }
                }
            }
        }
    }

    "Unknown".to_string()
}

fn extract_status(resource: &Map<String, Value>, key: &str) -> String {
    let Some(raw) = resource.get(key) else {
        return "unknown".to_string();
    };

    if let Some(s) = raw.as_str() {
        return s.to_string();
    }

    if let Some(obj) = raw.as_object() {
        if let Some(codings) = obj.get("coding").and_then(Value::as_array) {
            if let Some(first) = codings.first() {
                if let Some(code) = first.get("code").and_then(Value::as_str) {
                    return code.to_string();
                }
            }
        }
        if let Some(text) = obj.get("text").and_then(Value::as_str) {
            return text.to_string();
        }
    }

    "unknown".to_string()
}

fn extract_telecom(telecom_raw: Option<&Value>) -> Vec<Value> {
    let Some(arr) = telecom_raw.and_then(Value::as_array) else {
        return Vec::new();
    };

    let mut results = Vec::new();
    for item in arr {
        if let Some(obj) = item.as_object() {
            if let Some(val) = obj.get("value").and_then(Value::as_str) {
                let system = obj.get("system").and_then(Value::as_str).unwrap_or("phone");
                let mut entry = serde_json::Map::new();
                entry.insert("system".to_string(), json!(system));
                entry.insert("value".to_string(), json!(val));
                if let Some(use_val) = obj.get("use").and_then(Value::as_str) {
                    entry.insert("use".to_string(), json!(use_val));
                }
                results.push(Value::Object(entry));
            }
        }
    }
    results
}

fn extract_reference_range(ref_ranges: Option<&Value>) -> Option<String> {
    let arr = ref_ranges.and_then(Value::as_array)?;
    let first = arr.first()?.as_object()?;

    if let Some(text) = first.get("text").and_then(Value::as_str) {
        if !text.is_empty() {
            return Some(text.to_string());
        }
    }

    let low = first
        .get("low")
        .and_then(Value::as_object)
        .and_then(|o| o.get("value"))
        .and_then(|v| {
            v.as_f64()
                .map(|n| n.to_string())
                .or_else(|| v.as_i64().map(|n| n.to_string()))
        });

    let high = first
        .get("high")
        .and_then(Value::as_object)
        .and_then(|o| o.get("value"))
        .and_then(|v| {
            v.as_f64()
                .map(|n| n.to_string())
                .or_else(|| v.as_i64().map(|n| n.to_string()))
        });

    let unit = first
        .get("high")
        .and_then(Value::as_object)
        .and_then(|o| o.get("unit"))
        .and_then(Value::as_str)
        .or_else(|| {
            first
                .get("low")
                .and_then(Value::as_object)
                .and_then(|o| o.get("unit"))
                .and_then(Value::as_str)
        })
        .unwrap_or("");

    match (low, high) {
        (Some(l), Some(h)) => {
            let u = if unit.is_empty() { "" } else { " " };
            Some(format!("{l} - {h}{u}{unit}").trim().to_string())
        }
        (Some(l), None) => {
            let u = if unit.is_empty() { "" } else { " " };
            Some(format!(">= {l}{u}{unit}").trim().to_string())
        }
        (None, Some(h)) => {
            let u = if unit.is_empty() { "" } else { " " };
            Some(format!("<= {h}{u}{unit}").trim().to_string())
        }
        (None, None) => None,
    }
}

fn extract_interpretation(interp_raw: Option<&Value>) -> Option<String> {
    let val = interp_raw?;
    if let Some(s) = val.as_str() {
        return Some(s.to_string());
    }
    if let Some(arr) = val.as_array() {
        if let Some(first) = arr.first() {
            let code = extract_code(Some(first));
            if code != "Unknown" {
                return Some(code);
            }
        }
    }
    if val.is_object() {
        let code = extract_code(Some(val));
        if code != "Unknown" {
            return Some(code);
        }
    }
    None
}

fn extract_performer(perf_raw: Option<&Value>) -> Option<String> {
    let val = perf_raw?;
    if let Some(s) = val.as_str() {
        return Some(s.to_string());
    }
    if let Some(arr) = val.as_array() {
        if let Some(first) = arr.first() {
            if let Some(obj) = first.as_object() {
                if let Some(disp) = obj.get("display").and_then(Value::as_str) {
                    return Some(disp.to_string());
                }
                if let Some(refr) = obj.get("reference").and_then(Value::as_str) {
                    return Some(refr.to_string());
                }
            }
        }
    }
    if let Some(obj) = val.as_object() {
        if let Some(disp) = obj.get("display").and_then(Value::as_str) {
            return Some(disp.to_string());
        }
        if let Some(refr) = obj.get("reference").and_then(Value::as_str) {
            return Some(refr.to_string());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Resource Distillers
// ---------------------------------------------------------------------------

fn distill_patient(res: &Map<String, Value>, level: DetailLevel) -> Value {
    let id = res.get("id").and_then(Value::as_str).unwrap_or("");
    let name = extract_name(res.get("name"));
    let mrn = extract_identifier(res.get("identifier"));
    let gender = res
        .get("gender")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let birth_date = res
        .get("birthDate")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let active = res.get("active").and_then(Value::as_bool).unwrap_or(true);

    match level {
        DetailLevel::Compact => json!({
            "resourceType": "Patient",
            "id": id,
            "name": name,
            "identifier": mrn,
            "gender": gender,
            "birthDate": birth_date,
        }),
        DetailLevel::Standard => {
            let telecom = extract_telecom(res.get("telecom"));
            json!({
                "resourceType": "Patient",
                "id": id,
                "name": name,
                "identifier": mrn,
                "gender": gender,
                "birthDate": birth_date,
                "active": active,
                "telecom": telecom,
            })
        }
        DetailLevel::Executive => json!({
            "resourceType": "Patient",
            "id": id,
            "name": name,
            "identifier": mrn,
            "gender": gender,
            "birthDate": birth_date,
            "active": active,
        }),
    }
}

fn distill_observation(res: &Map<String, Value>, level: DetailLevel) -> Value {
    let id = res.get("id").and_then(Value::as_str).unwrap_or("");
    let code = extract_code(res.get("code"));
    let status = extract_status(res, "status");

    let mut val: Value = Value::Null;
    let mut unit: Option<String> = None;

    if let Some(vq) = res.get("valueQuantity").and_then(Value::as_object) {
        if let Some(v) = vq.get("value") {
            val = v.clone();
        }
        if let Some(u) = vq.get("unit").and_then(Value::as_str) {
            unit = Some(u.to_string());
        }
    } else if let Some(vs) = res.get("valueString") {
        val = vs.clone();
    } else if let Some(vc) = res.get("valueCodeableConcept") {
        val = Value::String(extract_code(Some(vc)));
    }

    let interp = extract_interpretation(res.get("interpretation"));
    let mut is_critical = false;
    if let Some(ref interp_str) = interp {
        let lower = interp_str.to_lowercase();
        let flags = ["high", "critical", "low", "abnormal", "panic", "alert", "h"];
        if flags.iter().any(|&f| lower.contains(f)) {
            is_critical = true;
        }
    }

    match level {
        DetailLevel::Compact => {
            let mut out = json!({
                "resourceType": "Observation",
                "id": id,
                "code": code,
                "value": val,
                "status": status,
            });
            if let Some(u) = unit {
                out["unit"] = json!(u);
            }
            out
        }
        DetailLevel::Standard => {
            let ref_range = extract_reference_range(res.get("referenceRange"));
            let performer = extract_performer(res.get("performer"));
            let mut out = json!({
                "resourceType": "Observation",
                "id": id,
                "code": code,
                "value": val,
                "status": status,
                "effectiveDateTime": res.get("effectiveDateTime").cloned().unwrap_or(Value::Null),
            });
            if let Some(u) = unit {
                out["unit"] = json!(u);
            }
            if let Some(rr) = ref_range {
                out["referenceRange"] = json!(rr);
            }
            if let Some(ip) = interp {
                out["interpretation"] = json!(ip);
            }
            if let Some(p) = performer {
                out["performer"] = json!(p);
            }
            out
        }
        DetailLevel::Executive => {
            let formatted_val = if let Some(ref u) = unit {
                if !val.is_null() {
                    let v_str = val
                        .as_f64()
                        .map(|n| n.to_string())
                        .or_else(|| val.as_i64().map(|n| n.to_string()))
                        .unwrap_or_else(|| val.to_string());
                    json!(format!("{v_str} {u}").trim())
                } else {
                    val
                }
            } else {
                val
            };
            let mut out = json!({
                "resourceType": "Observation",
                "id": id,
                "code": code,
                "value": formatted_val,
                "status": status,
            });
            if let Some(ip) = interp {
                out["interpretation"] = json!(ip);
            }
            if is_critical {
                out["is_critical"] = json!(true);
            }
            out
        }
    }
}

fn distill_condition(res: &Map<String, Value>, level: DetailLevel) -> Value {
    let id = res.get("id").and_then(Value::as_str).unwrap_or("");
    let code = extract_code(res.get("code"));
    let clinical_status = extract_status(res, "clinicalStatus");
    let verification_status = extract_status(res, "verificationStatus");

    match level {
        DetailLevel::Compact => json!({
            "resourceType": "Condition",
            "id": id,
            "code": code,
            "clinicalStatus": clinical_status,
        }),
        DetailLevel::Standard => {
            let mut out = json!({
                "resourceType": "Condition",
                "id": id,
                "code": code,
                "clinicalStatus": clinical_status,
                "verificationStatus": verification_status,
            });
            if let Some(onset) = res.get("onsetDateTime") {
                out["onsetDateTime"] = onset.clone();
            }
            if let Some(cats) = res.get("category").and_then(Value::as_array) {
                if let Some(first) = cats.first() {
                    out["category"] = json!(extract_code(Some(first)));
                }
            }
            out
        }
        DetailLevel::Executive => json!({
            "resourceType": "Condition",
            "id": id,
            "code": code,
            "clinicalStatus": clinical_status,
            "verificationStatus": verification_status,
            "is_active": clinical_status == "active",
        }),
    }
}

fn distill_medication_request(res: &Map<String, Value>, level: DetailLevel) -> Value {
    let id = res.get("id").and_then(Value::as_str).unwrap_or("");
    let med_val = res
        .get("medicationCodeableConcept")
        .or_else(|| res.get("medicationReference"));
    let medication = extract_code(med_val);
    let status = extract_status(res, "status");
    let intent = res.get("intent").and_then(Value::as_str).unwrap_or("order");

    match level {
        DetailLevel::Compact => json!({
            "resourceType": "MedicationRequest",
            "id": id,
            "medication": medication,
            "status": status,
        }),
        DetailLevel::Standard => {
            let mut dosage_text = String::new();
            if let Some(dosage_instr) = res.get("dosageInstruction").and_then(Value::as_array) {
                if let Some(first_dose) = dosage_instr.first().and_then(Value::as_object) {
                    if let Some(text) = first_dose.get("text").and_then(Value::as_str) {
                        dosage_text = text.to_string();
                    } else if first_dose.contains_key("timing") {
                        dosage_text = "Standard clinical dosage".to_string();
                    }
                }
            }

            let mut out = json!({
                "resourceType": "MedicationRequest",
                "id": id,
                "medication": medication,
                "status": status,
                "intent": intent,
                "dosageInstruction": dosage_text,
            });
            if let Some(auth_on) = res.get("authoredOn") {
                out["authoredOn"] = auth_on.clone();
            }
            out
        }
        DetailLevel::Executive => json!({
            "resourceType": "MedicationRequest",
            "id": id,
            "medication": medication,
            "status": status,
            "intent": intent,
        }),
    }
}

fn distill_allergy_intolerance(res: &Map<String, Value>, level: DetailLevel) -> Value {
    let id = res.get("id").and_then(Value::as_str).unwrap_or("");
    let code = extract_code(res.get("code"));
    let clinical_status = extract_status(res, "clinicalStatus");
    let verification_status = extract_status(res, "verificationStatus");
    let criticality = res
        .get("criticality")
        .and_then(Value::as_str)
        .unwrap_or("low");
    let is_critical = criticality == "high" || criticality == "CRIT-H";

    match level {
        DetailLevel::Compact => json!({
            "resourceType": "AllergyIntolerance",
            "id": id,
            "code": code,
            "clinicalStatus": clinical_status,
            "criticality": criticality,
        }),
        DetailLevel::Standard => {
            let mut reactions = Vec::new();
            if let Some(rxns) = res.get("reaction").and_then(Value::as_array) {
                for rxn in rxns {
                    if let Some(r_obj) = rxn.as_object() {
                        let manifestations = r_obj.get("manifestation").and_then(Value::as_array);
                        let manifest_str = if let Some(m_arr) = manifestations {
                            extract_code(m_arr.first())
                        } else {
                            "Unknown".to_string()
                        };
                        let severity = r_obj
                            .get("severity")
                            .and_then(Value::as_str)
                            .unwrap_or("moderate");
                        reactions.push(json!({
                            "manifestation": manifest_str,
                            "severity": severity,
                        }));
                    }
                }
            }

            let mut out = json!({
                "resourceType": "AllergyIntolerance",
                "id": id,
                "code": code,
                "clinicalStatus": clinical_status,
                "verificationStatus": verification_status,
                "criticality": criticality,
                "reaction": reactions,
            });
            if let Some(onset) = res.get("onsetDateTime") {
                out["onset"] = onset.clone();
            }
            out
        }
        DetailLevel::Executive => json!({
            "resourceType": "AllergyIntolerance",
            "id": id,
            "code": code,
            "criticality": criticality,
            "is_critical": is_critical,
        }),
    }
}

fn distill_diagnostic_report(res: &Map<String, Value>, level: DetailLevel) -> Value {
    let id = res.get("id").and_then(Value::as_str).unwrap_or("");
    let code = extract_code(res.get("code"));
    let status = extract_status(res, "status");
    let conclusion = res.get("conclusion").and_then(Value::as_str).unwrap_or("");

    let mut category = "Laboratory".to_string();
    if let Some(cats) = res.get("category").and_then(Value::as_array) {
        if let Some(first) = cats.first() {
            let c = extract_code(Some(first));
            if c != "Unknown" {
                category = c;
            }
        }
    }

    match level {
        DetailLevel::Compact => json!({
            "resourceType": "DiagnosticReport",
            "id": id,
            "code": code,
            "status": status,
        }),
        DetailLevel::Standard => {
            let performer = extract_performer(res.get("performer"));
            let mut out = json!({
                "resourceType": "DiagnosticReport",
                "id": id,
                "code": code,
                "status": status,
                "category": category,
                "conclusion": conclusion,
                "effectiveDateTime": res.get("effectiveDateTime").cloned().unwrap_or(Value::Null),
            });
            if let Some(p) = performer {
                out["performer"] = json!(p);
            }
            out
        }
        DetailLevel::Executive => json!({
            "resourceType": "DiagnosticReport",
            "id": id,
            "code": code,
            "status": status,
            "category": category,
            "conclusion": conclusion,
        }),
    }
}

fn distill_encounter(res: &Map<String, Value>, level: DetailLevel) -> Value {
    let id = res.get("id").and_then(Value::as_str).unwrap_or("");
    let status = extract_status(res, "status");

    let mut enc_class = "AMB".to_string();
    if let Some(raw_class) = res.get("class") {
        if let Some(c_obj) = raw_class.as_object() {
            if let Some(c) = c_obj.get("code").and_then(Value::as_str) {
                enc_class = c.to_string();
            }
        } else if let Some(s) = raw_class.as_str() {
            enc_class = s.to_string();
        }
    }

    let mut enc_type = "Encounter".to_string();
    if let Some(types) = res.get("type").and_then(Value::as_array) {
        if let Some(first) = types.first() {
            let t = extract_code(Some(first));
            if t != "Unknown" {
                enc_type = t;
            }
        }
    }

    let mut reason = String::new();
    if let Some(reasons) = res.get("reasonCode").and_then(Value::as_array) {
        if let Some(first) = reasons.first() {
            let r = extract_code(Some(first));
            if r != "Unknown" {
                reason = r;
            }
        }
    }

    match level {
        DetailLevel::Compact => json!({
            "resourceType": "Encounter",
            "id": id,
            "status": status,
            "class": enc_class,
            "type": enc_type,
        }),
        DetailLevel::Standard => json!({
            "resourceType": "Encounter",
            "id": id,
            "status": status,
            "class": enc_class,
            "type": enc_type,
            "period": res.get("period").cloned().unwrap_or_else(|| json!({})),
            "reasonCode": reason,
        }),
        DetailLevel::Executive => json!({
            "resourceType": "Encounter",
            "id": id,
            "status": status,
            "class": enc_class,
            "type": enc_type,
            "reason": reason,
        }),
    }
}

fn distill_care_plan(res: &Map<String, Value>, level: DetailLevel) -> Value {
    let id = res.get("id").and_then(Value::as_str).unwrap_or("");
    let title = res
        .get("title")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .unwrap_or_else(|| extract_code(res.get("category")));
    let title = if title == "Unknown" || title.is_empty() {
        "Care Plan".to_string()
    } else {
        title
    };

    let status = extract_status(res, "status");
    let intent = res.get("intent").and_then(Value::as_str).unwrap_or("plan");

    let mut activities_list = Vec::new();
    if let Some(raw_acts) = res.get("activity").and_then(Value::as_array) {
        for act in raw_acts {
            if let Some(detail) = act.get("detail").and_then(Value::as_object) {
                let desc = detail
                    .get("description")
                    .and_then(Value::as_str)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| extract_code(detail.get("code")));
                if !desc.is_empty() && desc != "Unknown" {
                    activities_list.push(desc);
                }
            }
        }
    }

    match level {
        DetailLevel::Compact => json!({
            "resourceType": "CarePlan",
            "id": id,
            "title": title,
            "status": status,
            "intent": intent,
        }),
        DetailLevel::Standard => json!({
            "resourceType": "CarePlan",
            "id": id,
            "title": title,
            "status": status,
            "intent": intent,
            "period": res.get("period").cloned().unwrap_or_else(|| json!({})),
            "activities": activities_list,
        }),
        DetailLevel::Executive => json!({
            "resourceType": "CarePlan",
            "id": id,
            "title": title,
            "status": status,
            "intent": intent,
            "activity_count": activities_list.len(),
        }),
    }
}

fn distill_generic(res: &Map<String, Value>, _level: DetailLevel) -> Value {
    let res_type = res
        .get("resourceType")
        .and_then(Value::as_str)
        .unwrap_or("Resource");
    let id = res.get("id").and_then(Value::as_str).unwrap_or("");
    let mut out = json!({
        "resourceType": res_type,
        "id": id,
    });
    if res.contains_key("status") {
        out["status"] = json!(extract_status(res, "status"));
    }
    if res.contains_key("code") {
        out["code"] = json!(extract_code(res.get("code")));
    }
    out
}

fn distill_bundle(bundle_map: &Map<String, Value>, level: DetailLevel) -> Value {
    let entries = bundle_map.get("entry").and_then(Value::as_array);
    let mut distilled_entries = Vec::new();

    if let Some(arr) = entries {
        for entry in arr {
            if let Some(entry_obj) = entry.as_object() {
                if let Some(resource_val) = entry_obj.get("resource") {
                    let mut dist_item = json!({
                        "resource": distill_resource(resource_val, level),
                    });
                    if level != DetailLevel::Compact {
                        if let Some(full_url) = entry_obj.get("fullUrl") {
                            dist_item["fullUrl"] = full_url.clone();
                        }
                    }
                    distilled_entries.push(dist_item);
                } else {
                    distilled_entries.push(json!({
                        "resource": distill_resource(entry, level),
                    }));
                }
            }
        }
    }

    let default_total = distilled_entries.len() as i64;
    let total_count = bundle_map
        .get("total")
        .and_then(Value::as_i64)
        .unwrap_or(default_total);
    let bundle_type = bundle_map
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("searchset");

    let mut out = json!({
        "resourceType": "Bundle",
        "type": bundle_type,
        "total": total_count,
        "entry": distilled_entries,
    });
    if let Some(id) = bundle_map.get("id") {
        out["id"] = id.clone();
    }
    out
}

/// Distill a FHIR R4 resource or Bundle according to the specified detail level.
pub fn distill_resource(resource: &Value, level: DetailLevel) -> Value {
    let Some(obj) = resource.as_object() else {
        return resource.clone();
    };

    let res_type = obj
        .get("resourceType")
        .and_then(Value::as_str)
        .unwrap_or("");
    if res_type == "Bundle" {
        return distill_bundle(obj, level);
    }

    match res_type {
        "Patient" => distill_patient(obj, level),
        "Observation" => distill_observation(obj, level),
        "Condition" => distill_condition(obj, level),
        "MedicationRequest" => distill_medication_request(obj, level),
        "AllergyIntolerance" => distill_allergy_intolerance(obj, level),
        "DiagnosticReport" => distill_diagnostic_report(obj, level),
        "Encounter" => distill_encounter(obj, level),
        "CarePlan" => distill_care_plan(obj, level),
        _ => distill_generic(obj, level),
    }
}

/// Distill a raw JSON byte slice using in-situ SIMD acceleration where supported,
/// falling back seamlessly to standard serde JSON parsing.
pub fn distill_raw_slice(
    raw_bytes: &mut [u8],
    level: DetailLevel,
) -> Result<Value, TokenDietError> {
    match crate::simd_diet::distill_resource_simd(raw_bytes, level) {
        Ok(simd_distilled) => Ok(simd_distilled.to_value()),
        Err(_) => {
            let val: Value = serde_json::from_slice(raw_bytes)
                .map_err(|e| TokenDietError::InvalidDetailLevel(e.to_string()))?;
            Ok(distill_resource(&val, level))
        }
    }
}
