//! In-Memory St. Jude Pediatric Clinical Sandbox with rich synthetic oncology dataset.
//!
//! Provides realistic FHIR R4 clinical data, search and query capabilities,
//! and safety-gated draft mutation methods.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use medplum_mcp_core::safety::{assert_write_permitted, SafetyViolationError};
use serde_json::{json, Value};

static DRAFT_COUNTER: AtomicU64 = AtomicU64::new(1000);

#[derive(Debug, Clone)]
struct SandboxInner {
    patients: Vec<Value>,
    observations: Vec<Value>,
    conditions: Vec<Value>,
    medications: Vec<Value>,
    allergies: Vec<Value>,
    diagnostic_reports: Vec<Value>,
    encounters: Vec<Value>,
    care_plans: Vec<Value>,
}

/// Thread-safe in-memory clinical sandbox populated with synthetic St. Jude pediatric oncology data.
#[derive(Debug, Clone)]
pub struct ClinicalSandbox {
    inner: Arc<RwLock<SandboxInner>>,
}

impl Default for ClinicalSandbox {
    fn default() -> Self {
        Self::new_st_jude()
    }
}

impl ClinicalSandbox {
    /// Create a new sandbox initialized with the pristine St. Jude Research Institute dataset.
    pub fn new_st_jude() -> Self {
        let initial = SandboxInner {
            patients: init_patients(),
            observations: init_observations(),
            conditions: init_conditions(),
            medications: init_medications(),
            allergies: init_allergies(),
            diagnostic_reports: init_diagnostic_reports(),
            encounters: init_encounters(),
            care_plans: init_care_plans(),
        };

        Self {
            inner: Arc::new(RwLock::new(initial)),
        }
    }

    /// Reset the sandbox to its pristine initial synthetic state.
    pub fn reset(&self) {
        if let Ok(mut lock) = self.inner.write() {
            *lock = SandboxInner {
                patients: init_patients(),
                observations: init_observations(),
                conditions: init_conditions(),
                medications: init_medications(),
                allergies: init_allergies(),
                diagnostic_reports: init_diagnostic_reports(),
                encounters: init_encounters(),
                care_plans: init_care_plans(),
            };
        }
    }

    /// Wrap a vector of FHIR resources into a standard searchset Bundle.
    pub fn to_bundle(&self, resources: Vec<Value>) -> Value {
        let count = resources.len();
        let entries: Vec<Value> = resources
            .into_iter()
            .map(|r| json!({ "resource": r }))
            .collect();

        json!({
            "resourceType": "Bundle",
            "type": "searchset",
            "total": count,
            "entry": entries
        })
    }

    // -----------------------------------------------------------------------
    // Patient queries
    // -----------------------------------------------------------------------

    pub fn search_patients(
        &self,
        name: Option<&str>,
        identifier: Option<&str>,
        dob: Option<&str>,
    ) -> Vec<Value> {
        let lock = match self.inner.read() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };

        lock.patients
            .iter()
            .filter(|pat| {
                if let Some(target_name) = name {
                    let target_lower = target_name.to_lowercase();
                    let mut matched = false;
                    if let Some(names) = pat.get("name").and_then(Value::as_array) {
                        for n in names {
                            if let Some(text) = n.get("text").and_then(Value::as_str) {
                                if text.to_lowercase().contains(&target_lower) {
                                    matched = true;
                                    break;
                                }
                            }
                            if let Some(family) = n.get("family").and_then(Value::as_str) {
                                if family.to_lowercase().contains(&target_lower) {
                                    matched = true;
                                    break;
                                }
                            }
                            if let Some(givens) = n.get("given").and_then(Value::as_array) {
                                for g in givens {
                                    if let Some(g_str) = g.as_str() {
                                        if g_str.to_lowercase().contains(&target_lower) {
                                            matched = true;
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if !matched {
                        return false;
                    }
                }

                if let Some(target_id) = identifier {
                    let mut matched = false;
                    if let Some(idents) = pat.get("identifier").and_then(Value::as_array) {
                        for ident in idents {
                            if let Some(v) = ident.get("value").and_then(Value::as_str) {
                                if v.eq_ignore_ascii_case(target_id) {
                                    matched = true;
                                    break;
                                }
                            }
                        }
                    }
                    if !matched {
                        return false;
                    }
                }

                if let Some(target_dob) = dob {
                    if let Some(pat_dob) = pat.get("birthDate").and_then(Value::as_str) {
                        if !pat_dob.starts_with(target_dob) {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }

                true
            })
            .cloned()
            .collect()
    }

    pub fn get_patient(&self, id: &str) -> Option<Value> {
        let clean_id = id.trim_start_matches("Patient/").trim();
        let lock = match self.inner.read() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };

        lock.patients
            .iter()
            .find(|p| p.get("id").and_then(Value::as_str) == Some(clean_id))
            .cloned()
    }

    // -----------------------------------------------------------------------
    // Observation methods
    // -----------------------------------------------------------------------

    pub fn list_observations(
        &self,
        patient_id: Option<&str>,
        category: Option<&str>,
        code: Option<&str>,
    ) -> Vec<Value> {
        let clean_pat_id = patient_id.map(|p| p.trim_start_matches("Patient/").trim());
        let lock = match self.inner.read() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };

        lock.observations
            .iter()
            .filter(|obs| {
                if let Some(target_pat) = clean_pat_id {
                    let sub = obs
                        .get("subject")
                        .and_then(|s| s.get("reference"))
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    if sub.trim_start_matches("Patient/").trim() != target_pat {
                        return false;
                    }
                }

                if let Some(target_cat) = category {
                    let cat_match = obs
                        .get("category")
                        .and_then(Value::as_array)
                        .map(|arr| {
                            arr.iter().any(|c| {
                                c.to_string()
                                    .to_lowercase()
                                    .contains(&target_cat.to_lowercase())
                            })
                        })
                        .unwrap_or(false);
                    if !cat_match {
                        return false;
                    }
                }

                if let Some(target_code) = code {
                    let code_str = obs.get("code").map(|c| c.to_string()).unwrap_or_default();
                    if !code_str
                        .to_lowercase()
                        .contains(&target_code.to_lowercase())
                    {
                        return false;
                    }
                }

                true
            })
            .cloned()
            .collect()
    }

    pub fn get_observation(&self, id: &str) -> Option<Value> {
        let clean_id = id.trim_start_matches("Observation/").trim();
        let lock = match self.inner.read() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };

        lock.observations
            .iter()
            .find(|o| o.get("id").and_then(Value::as_str) == Some(clean_id))
            .cloned()
    }

    pub fn create_observation_draft(
        &self,
        patient_id: &str,
        code: &str,
        value: f64,
        unit: &str,
        display: &str,
    ) -> Result<Value, SafetyViolationError> {
        let clean_pat = patient_id.trim_start_matches("Patient/").trim();
        let draft_id = format!("obs-{}", DRAFT_COUNTER.fetch_add(1, Ordering::SeqCst));

        let resource = json!({
            "resourceType": "Observation",
            "id": draft_id,
            "status": "preliminary",
            "code": {
                "coding": [{
                    "system": "http://loinc.org",
                    "code": code,
                    "display": display
                }],
                "text": display
            },
            "subject": {
                "reference": format!("Patient/{}", clean_pat)
            },
            "valueQuantity": {
                "value": value,
                "unit": unit
            }
        });

        self.create_observation_resource(resource)
    }

    pub fn create_observation_resource(
        &self,
        mut resource: Value,
    ) -> Result<Value, SafetyViolationError> {
        assert_write_permitted("Observation", &resource, true)?;

        let obj =
            resource
                .as_object_mut()
                .ok_or_else(|| SafetyViolationError::PermissionDenied {
                    resource_type: "Observation".to_string(),
                })?;

        obj.insert(
            "resourceType".to_string(),
            Value::String("Observation".to_string()),
        );

        if !obj.contains_key("id")
            || obj
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .is_empty()
        {
            let id = format!("obs-{}", DRAFT_COUNTER.fetch_add(1, Ordering::SeqCst));
            obj.insert("id".to_string(), Value::String(id));
        }

        if !obj.contains_key("status") {
            obj.insert(
                "status".to_string(),
                Value::String("preliminary".to_string()),
            );
        }

        let saved = Value::Object(obj.clone());
        let mut lock = match self.inner.write() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        lock.observations.push(saved.clone());
        Ok(saved)
    }

    // -----------------------------------------------------------------------
    // Condition methods
    // -----------------------------------------------------------------------

    pub fn list_conditions(&self, patient_id: Option<&str>) -> Vec<Value> {
        let clean_pat_id = patient_id.map(|p| p.trim_start_matches("Patient/").trim());
        let lock = match self.inner.read() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };

        lock.conditions
            .iter()
            .filter(|cond| {
                if let Some(target_pat) = clean_pat_id {
                    let sub = cond
                        .get("subject")
                        .and_then(|s| s.get("reference"))
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    if sub.trim_start_matches("Patient/").trim() != target_pat {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect()
    }

    pub fn get_condition(&self, id: &str) -> Option<Value> {
        let clean_id = id.trim_start_matches("Condition/").trim();
        let lock = match self.inner.read() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };

        lock.conditions
            .iter()
            .find(|c| c.get("id").and_then(Value::as_str) == Some(clean_id))
            .cloned()
    }

    // -----------------------------------------------------------------------
    // MedicationRequest methods
    // -----------------------------------------------------------------------

    pub fn list_medications(&self, patient_id: Option<&str>) -> Vec<Value> {
        let clean_pat_id = patient_id.map(|p| p.trim_start_matches("Patient/").trim());
        let lock = match self.inner.read() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };

        lock.medications
            .iter()
            .filter(|med| {
                if let Some(target_pat) = clean_pat_id {
                    let sub = med
                        .get("subject")
                        .and_then(|s| s.get("reference"))
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    if sub.trim_start_matches("Patient/").trim() != target_pat {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect()
    }

    pub fn get_medication(&self, id: &str) -> Option<Value> {
        let clean_id = id.trim_start_matches("MedicationRequest/").trim();
        let lock = match self.inner.read() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };

        lock.medications
            .iter()
            .find(|m| m.get("id").and_then(Value::as_str) == Some(clean_id))
            .cloned()
    }

    pub fn create_medication_draft(
        &self,
        patient_id: &str,
        med_code: &str,
        dosage: &str,
        instructions: &str,
        display: &str,
    ) -> Result<Value, SafetyViolationError> {
        let clean_pat = patient_id.trim_start_matches("Patient/").trim();
        let draft_id = format!("med-{}", DRAFT_COUNTER.fetch_add(1, Ordering::SeqCst));

        let resource = json!({
            "resourceType": "MedicationRequest",
            "id": draft_id,
            "status": "draft",
            "intent": "order",
            "subject": {
                "reference": format!("Patient/{}", clean_pat)
            },
            "medicationCodeableConcept": {
                "coding": [{
                    "system": "http://www.nlm.nih.gov/research/umls/rxnorm",
                    "code": med_code,
                    "display": display
                }],
                "text": display
            },
            "dosageInstruction": [{
                "text": format!("{}: {}", dosage, instructions)
            }]
        });

        self.create_medication_resource(resource)
    }

    pub fn create_medication_resource(
        &self,
        mut resource: Value,
    ) -> Result<Value, SafetyViolationError> {
        assert_write_permitted("MedicationRequest", &resource, true)?;

        let obj =
            resource
                .as_object_mut()
                .ok_or_else(|| SafetyViolationError::PermissionDenied {
                    resource_type: "MedicationRequest".to_string(),
                })?;

        obj.insert(
            "resourceType".to_string(),
            Value::String("MedicationRequest".to_string()),
        );

        if !obj.contains_key("id")
            || obj
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .is_empty()
        {
            let id = format!("med-{}", DRAFT_COUNTER.fetch_add(1, Ordering::SeqCst));
            obj.insert("id".to_string(), Value::String(id));
        }

        if !obj.contains_key("status") {
            obj.insert("status".to_string(), Value::String("draft".to_string()));
        }
        if !obj.contains_key("intent") {
            obj.insert("intent".to_string(), Value::String("order".to_string()));
        }

        let saved = Value::Object(obj.clone());
        let mut lock = match self.inner.write() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        lock.medications.push(saved.clone());
        Ok(saved)
    }

    // -----------------------------------------------------------------------
    // AllergyIntolerance methods
    // -----------------------------------------------------------------------

    pub fn list_allergies(&self, patient_id: Option<&str>) -> Vec<Value> {
        let clean_pat_id = patient_id.map(|p| p.trim_start_matches("Patient/").trim());
        let lock = match self.inner.read() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };

        lock.allergies
            .iter()
            .filter(|allg| {
                if let Some(target_pat) = clean_pat_id {
                    let sub = allg
                        .get("patient")
                        .and_then(|s| s.get("reference"))
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    if sub.trim_start_matches("Patient/").trim() != target_pat {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect()
    }

    pub fn get_allergy(&self, id: &str) -> Option<Value> {
        let clean_id = id.trim_start_matches("AllergyIntolerance/").trim();
        let lock = match self.inner.read() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };

        lock.allergies
            .iter()
            .find(|a| a.get("id").and_then(Value::as_str) == Some(clean_id))
            .cloned()
    }

    // -----------------------------------------------------------------------
    // DiagnosticReport methods
    // -----------------------------------------------------------------------

    pub fn list_diagnostic_reports(
        &self,
        patient_id: Option<&str>,
        code: Option<&str>,
    ) -> Vec<Value> {
        let clean_pat_id = patient_id.map(|p| p.trim_start_matches("Patient/").trim());
        let lock = match self.inner.read() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };

        lock.diagnostic_reports
            .iter()
            .filter(|rep| {
                if let Some(target_pat) = clean_pat_id {
                    let sub = rep
                        .get("subject")
                        .and_then(|s| s.get("reference"))
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    if sub.trim_start_matches("Patient/").trim() != target_pat {
                        return false;
                    }
                }
                if let Some(target_code) = code {
                    let c_str = rep.get("code").map(|c| c.to_string()).unwrap_or_default();
                    if !c_str.to_lowercase().contains(&target_code.to_lowercase()) {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect()
    }

    pub fn get_diagnostic_report(&self, id: &str) -> Option<Value> {
        let clean_id = id.trim_start_matches("DiagnosticReport/").trim();
        let lock = match self.inner.read() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };

        lock.diagnostic_reports
            .iter()
            .find(|r| r.get("id").and_then(Value::as_str) == Some(clean_id))
            .cloned()
    }

    // -----------------------------------------------------------------------
    // Encounter methods
    // -----------------------------------------------------------------------

    pub fn list_encounters(&self, patient_id: Option<&str>) -> Vec<Value> {
        let clean_pat_id = patient_id.map(|p| p.trim_start_matches("Patient/").trim());
        let lock = match self.inner.read() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };

        lock.encounters
            .iter()
            .filter(|enc| {
                if let Some(target_pat) = clean_pat_id {
                    let sub = enc
                        .get("subject")
                        .and_then(|s| s.get("reference"))
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    if sub.trim_start_matches("Patient/").trim() != target_pat {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect()
    }

    pub fn get_encounter(&self, id: &str) -> Option<Value> {
        let clean_id = id.trim_start_matches("Encounter/").trim();
        let lock = match self.inner.read() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };

        lock.encounters
            .iter()
            .find(|e| e.get("id").and_then(Value::as_str) == Some(clean_id))
            .cloned()
    }

    // -----------------------------------------------------------------------
    // CarePlan methods
    // -----------------------------------------------------------------------

    pub fn list_care_plans(&self, patient_id: Option<&str>) -> Vec<Value> {
        let clean_pat_id = patient_id.map(|p| p.trim_start_matches("Patient/").trim());
        let lock = match self.inner.read() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };

        lock.care_plans
            .iter()
            .filter(|cp| {
                if let Some(target_pat) = clean_pat_id {
                    let sub = cp
                        .get("subject")
                        .and_then(|s| s.get("reference"))
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    if sub.trim_start_matches("Patient/").trim() != target_pat {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect()
    }

    pub fn get_care_plan(&self, id: &str) -> Option<Value> {
        let clean_id = id.trim_start_matches("CarePlan/").trim();
        let lock = match self.inner.read() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };

        lock.care_plans
            .iter()
            .find(|c| c.get("id").and_then(Value::as_str) == Some(clean_id))
            .cloned()
    }
}

// ===========================================================================
// Initial St. Jude Synthetic Dataset
// ===========================================================================

fn init_patients() -> Vec<Value> {
    vec![
        json!({
            "resourceType": "Patient",
            "id": "pat-sj-001",
            "identifier": [
                {"system": "http://hospital.smarthealthit.org", "value": "MRN-SJ-100234", "type": {"text": "MRN"}},
                {"system": "http://hl7.org/fhir/sid/us-ssn", "value": "900-11-2341"}
            ],
            "active": true,
            "name": [{
                "use": "official",
                "family": "Chen",
                "given": ["Sarah"],
                "text": "Sarah Chen"
            }],
            "telecom": [
                {"system": "phone", "value": "555-019-2831", "use": "home"},
                {"system": "email", "value": "family.chen@example.org"}
            ],
            "gender": "female",
            "birthDate": "2018-05-14",
            "address": [{
                "use": "home",
                "line": ["262 Danny Thomas Place", "Suite 4B"],
                "city": "Memphis",
                "state": "TN",
                "postalCode": "38105",
                "country": "US"
            }]
        }),
        json!({
            "resourceType": "Patient",
            "id": "pat-sj-002",
            "identifier": [
                {"system": "http://hospital.smarthealthit.org", "value": "MRN-SJ-100235", "type": {"text": "MRN"}}
            ],
            "active": true,
            "name": [{
                "use": "official",
                "family": "Patel",
                "given": ["Liam"],
                "text": "Liam Patel"
            }],
            "gender": "male",
            "birthDate": "2017-09-22",
            "address": [{
                "line": ["450 Hospital Blvd"],
                "city": "Memphis",
                "state": "TN",
                "postalCode": "38105"
            }]
        }),
        json!({
            "resourceType": "Patient",
            "id": "pat-sj-003",
            "identifier": [
                {"system": "http://hospital.smarthealthit.org", "value": "MRN-SJ-100236", "type": {"text": "MRN"}}
            ],
            "active": true,
            "name": [{
                "use": "official",
                "family": "Vance",
                "given": ["Marcus"],
                "text": "Marcus Vance"
            }],
            "gender": "male",
            "birthDate": "2019-04-12",
            "address": [{
                "line": ["100 St. Jude Terrace"],
                "city": "Memphis",
                "state": "TN",
                "postalCode": "38105"
            }]
        }),
        json!({
            "resourceType": "Patient",
            "id": "pat-sj-004",
            "identifier": [
                {"system": "http://hospital.smarthealthit.org", "value": "MRN-SJ-100237", "type": {"text": "MRN"}}
            ],
            "active": true,
            "name": [{
                "use": "official",
                "family": "Rostova",
                "given": ["Elena"],
                "text": "Elena Rostova"
            }],
            "gender": "female",
            "birthDate": "2016-11-03",
            "address": [{
                "line": ["332 Medical Center Way"],
                "city": "Memphis",
                "state": "TN",
                "postalCode": "38105"
            }]
        }),
        json!({
            "resourceType": "Patient",
            "id": "pat-sj-005",
            "identifier": [
                {"system": "http://hospital.smarthealthit.org", "value": "MRN-SJ-100238", "type": {"text": "MRN"}}
            ],
            "active": true,
            "name": [{
                "use": "official",
                "family": "Kim",
                "given": ["Chloe"],
                "text": "Chloe Kim"
            }],
            "gender": "female",
            "birthDate": "2020-02-18",
            "address": [{
                "line": ["88 Pediatric Care Lane"],
                "city": "Memphis",
                "state": "TN",
                "postalCode": "38105"
            }]
        }),
        json!({
            "resourceType": "Patient",
            "id": "pat-sj-006",
            "identifier": [
                {"system": "http://hospital.smarthealthit.org", "value": "MRN-SJ-100239", "type": {"text": "MRN"}}
            ],
            "active": true,
            "name": [{
                "use": "official",
                "family": "Miller",
                "given": ["David"],
                "text": "David Miller"
            }],
            "gender": "male",
            "birthDate": "2015-07-30",
            "address": [{
                "line": ["12 Ridgeview Dr"],
                "city": "Germantown",
                "state": "TN",
                "postalCode": "38138"
            }]
        }),
        json!({
            "resourceType": "Patient",
            "id": "pat-sj-007",
            "identifier": [
                {"system": "http://hospital.smarthealthit.org", "value": "MRN-SJ-100240", "type": {"text": "MRN"}}
            ],
            "active": true,
            "name": [{
                "use": "official",
                "family": "Al-Mansoor",
                "given": ["Aisha"],
                "text": "Aisha Al-Mansoor"
            }],
            "gender": "female",
            "birthDate": "2018-12-05",
            "address": [{
                "line": ["501 Poplar Ave"],
                "city": "Memphis",
                "state": "TN",
                "postalCode": "38103"
            }]
        }),
        json!({
            "resourceType": "Patient",
            "id": "pat-sj-008",
            "identifier": [
                {"system": "http://hospital.smarthealthit.org", "value": "MRN-SJ-100241", "type": {"text": "MRN"}}
            ],
            "active": true,
            "name": [{
                "use": "official",
                "family": "Washington",
                "given": ["Noah"],
                "text": "Noah Washington"
            }],
            "gender": "male",
            "birthDate": "2017-03-19",
            "address": [{
                "line": ["720 Union Ave"],
                "city": "Memphis",
                "state": "TN",
                "postalCode": "38103"
            }]
        }),
        json!({
            "resourceType": "Patient",
            "id": "pat-sj-009",
            "identifier": [
                {"system": "http://hospital.smarthealthit.org", "value": "MRN-SJ-100242", "type": {"text": "MRN"}}
            ],
            "active": true,
            "name": [{
                "use": "official",
                "family": "Martinez",
                "given": ["Sophia"],
                "text": "Sophia Martinez"
            }],
            "gender": "female",
            "birthDate": "2019-08-11",
            "address": [{
                "line": ["310 Cooper St"],
                "city": "Memphis",
                "state": "TN",
                "postalCode": "38104"
            }]
        }),
        json!({
            "resourceType": "Patient",
            "id": "pat-sj-010",
            "identifier": [
                {"system": "http://hospital.smarthealthit.org", "value": "MRN-SJ-100243", "type": {"text": "MRN"}}
            ],
            "active": true,
            "name": [{
                "use": "official",
                "family": "Dubois",
                "given": ["Lucas"],
                "text": "Lucas Dubois"
            }],
            "gender": "male",
            "birthDate": "2016-01-25",
            "address": [{
                "line": ["15 Madison Ave"],
                "city": "Memphis",
                "state": "TN",
                "postalCode": "38103"
            }]
        }),
        json!({
            "resourceType": "Patient",
            "id": "pat-sj-011",
            "identifier": [
                {"system": "http://hospital.smarthealthit.org", "value": "MRN-SJ-100244", "type": {"text": "MRN"}}
            ],
            "active": true,
            "name": [{
                "use": "official",
                "family": "Lin",
                "given": ["Maya"],
                "text": "Maya Lin"
            }],
            "gender": "female",
            "birthDate": "2021-06-14",
            "address": [{
                "line": ["90 McLean Blvd"],
                "city": "Memphis",
                "state": "TN",
                "postalCode": "38104"
            }]
        }),
    ]
}

fn init_observations() -> Vec<Value> {
    vec![
        // Sarah Chen (pat-sj-001) Labs & Vitals
        json!({
            "resourceType": "Observation",
            "id": "obs-sj-001",
            "status": "final",
            "category": [{
                "coding": [{
                    "system": "http://terminology.hl7.org/CodeSystem/observation-category",
                    "code": "laboratory"
                }]
            }],
            "code": {
                "coding": [{
                    "system": "http://loinc.org",
                    "code": "26499-4",
                    "display": "Absolute Neutrophil Count"
                }],
                "text": "Absolute Neutrophil Count (ANC)"
            },
            "subject": {"reference": "Patient/pat-sj-001"},
            "effectiveDateTime": "2026-09-28T09:30:00Z",
            "valueQuantity": {"value": 1350.0, "unit": "/uL", "system": "http://unitsofmeasure.org", "code": "/uL"},
            "referenceRange": [{"low": {"value": 1500.0, "unit": "/uL"}, "high": {"value": 8000.0, "unit": "/uL"}}]
        }),
        json!({
            "resourceType": "Observation",
            "id": "obs-sj-002",
            "status": "final",
            "category": [{
                "coding": [{
                    "system": "http://terminology.hl7.org/CodeSystem/observation-category",
                    "code": "laboratory"
                }]
            }],
            "code": {
                "coding": [{
                    "system": "http://loinc.org",
                    "code": "77147-7",
                    "display": "Blasts/100 leukocytes in Bone marrow by Manual count"
                }],
                "text": "Bone Marrow Blast Percentage"
            },
            "subject": {"reference": "Patient/pat-sj-001"},
            "effectiveDateTime": "2026-09-28T10:00:00Z",
            "valueQuantity": {"value": 0.02, "unit": "%", "system": "http://unitsofmeasure.org", "code": "%"},
            "interpretation": [{
                "coding": [{"system": "http://terminology.hl7.org/CodeSystem/v3-ObservationInterpretation", "code": "N", "display": "Normal"}]
            }]
        }),
        json!({
            "resourceType": "Observation",
            "id": "obs-sj-003",
            "status": "final",
            "category": [{
                "coding": [{
                    "system": "http://terminology.hl7.org/CodeSystem/observation-category",
                    "code": "laboratory"
                }]
            }],
            "code": {
                "coding": [{
                    "system": "http://loinc.org",
                    "code": "2160-0",
                    "display": "Creatinine [Mass/volume] in Serum or Plasma"
                }],
                "text": "Serum Creatinine"
            },
            "subject": {"reference": "Patient/pat-sj-001"},
            "effectiveDateTime": "2026-09-28T09:30:00Z",
            "valueQuantity": {"value": 0.42, "unit": "mg/dL", "system": "http://unitsofmeasure.org", "code": "mg/dL"}
        }),
        json!({
            "resourceType": "Observation",
            "id": "obs-sj-004",
            "status": "final",
            "category": [{
                "coding": [{
                    "system": "http://terminology.hl7.org/CodeSystem/observation-category",
                    "code": "vital-signs"
                }]
            }],
            "code": {
                "coding": [{
                    "system": "http://loinc.org",
                    "code": "8867-4",
                    "display": "Heart rate"
                }],
                "text": "Heart Rate"
            },
            "subject": {"reference": "Patient/pat-sj-001"},
            "effectiveDateTime": "2026-09-28T09:15:00Z",
            "valueQuantity": {"value": 98.0, "unit": "beats/min"}
        }),
        // Liam Patel (pat-sj-002) Vitals & Peak Flow
        json!({
            "resourceType": "Observation",
            "id": "obs-sj-005",
            "status": "final",
            "category": [{"coding": [{"system": "http://terminology.hl7.org/CodeSystem/observation-category", "code": "vital-signs"}]}],
            "code": {
                "coding": [{"system": "http://loinc.org", "code": "9279-1", "display": "Respiratory rate"}],
                "text": "Respiratory Rate"
            },
            "subject": {"reference": "Patient/pat-sj-002"},
            "effectiveDateTime": "2026-09-25T14:00:00Z",
            "valueQuantity": {"value": 22.0, "unit": "/min"}
        }),
        // Marcus Vance (pat-sj-003) ANC & Blast count
        json!({
            "resourceType": "Observation",
            "id": "obs-sj-006",
            "status": "final",
            "category": [{"coding": [{"system": "http://terminology.hl7.org/CodeSystem/observation-category", "code": "laboratory"}]}],
            "code": {
                "coding": [{"system": "http://loinc.org", "code": "26499-4", "display": "Absolute Neutrophil Count"}],
                "text": "Absolute Neutrophil Count"
            },
            "subject": {"reference": "Patient/pat-sj-003"},
            "effectiveDateTime": "2026-09-27T08:00:00Z",
            "valueQuantity": {"value": 920.0, "unit": "/uL"}
        }),
        json!({
            "resourceType": "Observation",
            "id": "obs-sj-007",
            "status": "final",
            "category": [{"coding": [{"system": "http://terminology.hl7.org/CodeSystem/observation-category", "code": "laboratory"}]}],
            "code": {
                "coding": [{"system": "http://loinc.org", "code": "77147-7", "display": "Blasts/100 leukocytes in Bone marrow"}],
                "text": "Bone Marrow Blast Count"
            },
            "subject": {"reference": "Patient/pat-sj-003"},
            "effectiveDateTime": "2026-09-27T08:30:00Z",
            "valueQuantity": {"value": 0.01, "unit": "%"}
        }),
        // Elena Rostova (pat-sj-004) Blood Glucose & Creatinine
        json!({
            "resourceType": "Observation",
            "id": "obs-sj-008",
            "status": "final",
            "category": [{"coding": [{"system": "http://terminology.hl7.org/CodeSystem/observation-category", "code": "laboratory"}]}],
            "code": {
                "coding": [{"system": "http://loinc.org", "code": "2339-0", "display": "Glucose [Mass/volume] in Blood"}],
                "text": "Blood Glucose"
            },
            "subject": {"reference": "Patient/pat-sj-004"},
            "effectiveDateTime": "2026-09-29T07:15:00Z",
            "valueQuantity": {"value": 118.0, "unit": "mg/dL"}
        }),
        json!({
            "resourceType": "Observation",
            "id": "obs-sj-009",
            "status": "final",
            "category": [{"coding": [{"system": "http://terminology.hl7.org/CodeSystem/observation-category", "code": "laboratory"}]}],
            "code": {
                "coding": [{"system": "http://loinc.org", "code": "2160-0", "display": "Creatinine in Serum"}],
                "text": "Serum Creatinine"
            },
            "subject": {"reference": "Patient/pat-sj-004"},
            "effectiveDateTime": "2026-09-29T07:15:00Z",
            "valueQuantity": {"value": 0.55, "unit": "mg/dL"}
        }),
        // Chloe Kim (pat-sj-005) Vitals
        json!({
            "resourceType": "Observation",
            "id": "obs-sj-010",
            "status": "final",
            "category": [{"coding": [{"system": "http://terminology.hl7.org/CodeSystem/observation-category", "code": "vital-signs"}]}],
            "code": {
                "coding": [{"system": "http://loinc.org", "code": "8310-5", "display": "Body temperature"}],
                "text": "Body Temperature"
            },
            "subject": {"reference": "Patient/pat-sj-005"},
            "effectiveDateTime": "2026-09-28T11:00:00Z",
            "valueQuantity": {"value": 37.1, "unit": "Cel"}
        }),
        // David Miller (pat-sj-006) Labs
        json!({
            "resourceType": "Observation",
            "id": "obs-sj-011",
            "status": "final",
            "category": [{"coding": [{"system": "http://terminology.hl7.org/CodeSystem/observation-category", "code": "laboratory"}]}],
            "code": {
                "coding": [{"system": "http://loinc.org", "code": "26499-4", "display": "Absolute Neutrophil Count"}],
                "text": "Absolute Neutrophil Count"
            },
            "subject": {"reference": "Patient/pat-sj-006"},
            "effectiveDateTime": "2026-09-26T10:00:00Z",
            "valueQuantity": {"value": 2400.0, "unit": "/uL"}
        }),
    ]
}

fn init_conditions() -> Vec<Value> {
    vec![
        json!({
            "resourceType": "Condition",
            "id": "cond-sj-001",
            "clinicalStatus": {"coding": [{"system": "http://terminology.hl7.org/CodeSystem/condition-clinical", "code": "active"}]},
            "verificationStatus": {"coding": [{"system": "http://terminology.hl7.org/CodeSystem/condition-ver-status", "code": "confirmed"}]},
            "category": [{"coding": [{"system": "http://terminology.hl7.org/CodeSystem/condition-category", "code": "encounter-diagnosis"}]}],
            "code": {
                "coding": [{
                    "system": "http://snomed.info/sct",
                    "code": "91857003",
                    "display": "B-cell acute lymphoblastic leukemia"
                }],
                "text": "Precursor B-cell Acute Lymphoblastic Leukemia (ALL)"
            },
            "subject": {"reference": "Patient/pat-sj-001"},
            "onsetDateTime": "2024-02-10"
        }),
        json!({
            "resourceType": "Condition",
            "id": "cond-sj-002",
            "clinicalStatus": {"coding": [{"system": "http://terminology.hl7.org/CodeSystem/condition-clinical", "code": "active"}]},
            "code": {
                "coding": [{
                    "system": "http://snomed.info/sct",
                    "code": "302215000",
                    "display": "Neutropenia"
                }],
                "text": "Chemotherapy-induced Neutropenia"
            },
            "subject": {"reference": "Patient/pat-sj-001"},
            "onsetDateTime": "2026-09-20"
        }),
        json!({
            "resourceType": "Condition",
            "id": "cond-sj-003",
            "clinicalStatus": {"coding": [{"system": "http://terminology.hl7.org/CodeSystem/condition-clinical", "code": "active"}]},
            "code": {
                "coding": [{
                    "system": "http://snomed.info/sct",
                    "code": "195967001",
                    "display": "Asthma"
                }],
                "text": "Pediatric Extrinsic Asthma"
            },
            "subject": {"reference": "Patient/pat-sj-002"},
            "onsetDateTime": "2021-06-15"
        }),
        json!({
            "resourceType": "Condition",
            "id": "cond-sj-004",
            "clinicalStatus": {"coding": [{"system": "http://terminology.hl7.org/CodeSystem/condition-clinical", "code": "active"}]},
            "code": {
                "coding": [{
                    "system": "http://snomed.info/sct",
                    "code": "46635009",
                    "display": "Type 1 diabetes mellitus"
                }],
                "text": "Type 1 Diabetes Mellitus"
            },
            "subject": {"reference": "Patient/pat-sj-004"},
            "onsetDateTime": "2022-09-01"
        }),
        json!({
            "resourceType": "Condition",
            "id": "cond-sj-005",
            "clinicalStatus": {"coding": [{"system": "http://terminology.hl7.org/CodeSystem/condition-clinical", "code": "active"}]},
            "code": {
                "coding": [{
                    "system": "http://snomed.info/sct",
                    "code": "91857003",
                    "display": "B-cell acute lymphoblastic leukemia"
                }],
                "text": "B-cell ALL"
            },
            "subject": {"reference": "Patient/pat-sj-003"}
        }),
    ]
}

fn init_medications() -> Vec<Value> {
    vec![
        json!({
            "resourceType": "MedicationRequest",
            "id": "med-sj-001",
            "status": "active",
            "intent": "order",
            "medicationCodeableConcept": {
                "coding": [{
                    "system": "http://www.nlm.nih.gov/research/umls/rxnorm",
                    "code": "6851",
                    "display": "Mercaptopurine 50 MG Oral Tablet"
                }],
                "text": "Mercaptopurine 50mg"
            },
            "subject": {"reference": "Patient/pat-sj-001"},
            "dosageInstruction": [{
                "text": "50 mg/m2 orally once daily at bedtime"
            }]
        }),
        json!({
            "resourceType": "MedicationRequest",
            "id": "med-sj-002",
            "status": "active",
            "intent": "order",
            "medicationCodeableConcept": {
                "coding": [{
                    "system": "http://www.nlm.nih.gov/research/umls/rxnorm",
                    "code": "6854",
                    "display": "Methotrexate 2.5 MG Oral Tablet"
                }],
                "text": "Methotrexate 20 mg/m2"
            },
            "subject": {"reference": "Patient/pat-sj-001"},
            "dosageInstruction": [{
                "text": "20 mg/m2 orally once weekly on Mondays"
            }]
        }),
        json!({
            "resourceType": "MedicationRequest",
            "id": "med-sj-003",
            "status": "active",
            "intent": "order",
            "medicationCodeableConcept": {
                "coding": [{
                    "system": "http://www.nlm.nih.gov/research/umls/rxnorm",
                    "code": "435",
                    "display": "Albuterol 90 MCG Inhaler"
                }],
                "text": "Albuterol Inhaler"
            },
            "subject": {"reference": "Patient/pat-sj-002"},
            "dosageInstruction": [{
                "text": "1-2 puffs inhaled every 4-6 hours as needed for wheezing"
            }]
        }),
        json!({
            "resourceType": "MedicationRequest",
            "id": "med-sj-004",
            "status": "active",
            "intent": "order",
            "medicationCodeableConcept": {
                "coding": [{
                    "system": "http://www.nlm.nih.gov/research/umls/rxnorm",
                    "code": "285018",
                    "display": "Insulin Glargine 100 UNT/ML Injectable Solution"
                }],
                "text": "Insulin Glargine"
            },
            "subject": {"reference": "Patient/pat-sj-004"},
            "dosageInstruction": [{
                "text": "14 units subcutaneously once daily at 21:00"
            }]
        }),
        json!({
            "resourceType": "MedicationRequest",
            "id": "med-sj-005",
            "status": "active",
            "intent": "order",
            "medicationCodeableConcept": {
                "coding": [{
                    "system": "http://www.nlm.nih.gov/research/umls/rxnorm",
                    "code": "6851",
                    "display": "Mercaptopurine"
                }],
                "text": "Mercaptopurine"
            },
            "subject": {"reference": "Patient/pat-sj-003"}
        }),
    ]
}

fn init_allergies() -> Vec<Value> {
    vec![
        json!({
            "resourceType": "AllergyIntolerance",
            "id": "allg-sj-001",
            "clinicalStatus": {"coding": [{"system": "http://terminology.hl7.org/CodeSystem/allergyintolerance-clinical", "code": "active"}]},
            "verificationStatus": {"coding": [{"system": "http://terminology.hl7.org/CodeSystem/allergyintolerance-verification", "code": "confirmed"}]},
            "type": "allergy",
            "category": ["medication"],
            "criticality": "high",
            "code": {
                "coding": [{
                    "system": "http://www.nlm.nih.gov/research/umls/rxnorm",
                    "code": "7980",
                    "display": "Penicillin"
                }],
                "text": "Penicillin (anaphylaxis)"
            },
            "patient": {"reference": "Patient/pat-sj-001"}
        }),
        json!({
            "resourceType": "AllergyIntolerance",
            "id": "allg-sj-002",
            "clinicalStatus": {"coding": [{"system": "http://terminology.hl7.org/CodeSystem/allergyintolerance-clinical", "code": "active"}]},
            "type": "allergy",
            "category": ["medication"],
            "criticality": "high",
            "code": {
                "coding": [{
                    "system": "http://www.nlm.nih.gov/research/umls/rxnorm",
                    "code": "10183",
                    "display": "Sulfamethoxazole"
                }],
                "text": "Sulfa Drugs"
            },
            "patient": {"reference": "Patient/pat-sj-002"}
        }),
        json!({
            "resourceType": "AllergyIntolerance",
            "id": "allg-sj-003",
            "clinicalStatus": {"coding": [{"system": "http://terminology.hl7.org/CodeSystem/allergyintolerance-clinical", "code": "active"}]},
            "type": "allergy",
            "category": ["food"],
            "criticality": "high",
            "code": {
                "coding": [{
                    "system": "http://snomed.info/sct",
                    "code": "91935009",
                    "display": "Allergy to peanut"
                }],
                "text": "Peanut Allergy"
            },
            "patient": {"reference": "Patient/pat-sj-004"}
        }),
    ]
}

fn init_diagnostic_reports() -> Vec<Value> {
    vec![
        json!({
            "resourceType": "DiagnosticReport",
            "id": "rep-sj-001",
            "status": "final",
            "category": [{"coding": [{"system": "http://terminology.hl7.org/CodeSystem/v2-0074", "code": "LAB"}]}],
            "code": {
                "coding": [{
                    "system": "http://loinc.org",
                    "code": "58410-2",
                    "display": "Complete blood count with automated differential"
                }],
                "text": "Complete Blood Count with Diff"
            },
            "subject": {"reference": "Patient/pat-sj-001"},
            "result": [
                {"reference": "Observation/obs-sj-001"},
                {"reference": "Observation/obs-sj-003"}
            ]
        }),
        json!({
            "resourceType": "DiagnosticReport",
            "id": "rep-sj-002",
            "status": "final",
            "code": {
                "coding": [{"system": "http://loinc.org", "code": "77147-7", "display": "Bone marrow biopsy report"}],
                "text": "Bone Marrow Aspirate & Biopsy Evaluation"
            },
            "subject": {"reference": "Patient/pat-sj-001"},
            "result": [
                {"reference": "Observation/obs-sj-002"}
            ]
        }),
        json!({
            "resourceType": "DiagnosticReport",
            "id": "rep-sj-003",
            "status": "final",
            "code": {"text": "Pulmonary Function Test Report"},
            "subject": {"reference": "Patient/pat-sj-002"}
        }),
        json!({
            "resourceType": "DiagnosticReport",
            "id": "rep-sj-004",
            "status": "final",
            "code": {"text": "Hemoglobin A1c Glycated Hemoglobin Report"},
            "subject": {"reference": "Patient/pat-sj-004"}
        }),
    ]
}

fn init_encounters() -> Vec<Value> {
    vec![
        json!({
            "resourceType": "Encounter",
            "id": "enc-sj-001",
            "status": "finished",
            "class": {
                "system": "http://terminology.hl7.org/CodeSystem/v3-ActCode",
                "code": "IMP",
                "display": "inpatient encounter"
            },
            "subject": {"reference": "Patient/pat-sj-001"},
            "period": {
                "start": "2026-09-27T08:00:00Z",
                "end": "2026-09-28T18:00:00Z"
            }
        }),
        json!({
            "resourceType": "Encounter",
            "id": "enc-sj-002",
            "status": "finished",
            "class": {
                "system": "http://terminology.hl7.org/CodeSystem/v3-ActCode",
                "code": "AMB",
                "display": "ambulatory"
            },
            "subject": {"reference": "Patient/pat-sj-002"}
        }),
        json!({
            "resourceType": "Encounter",
            "id": "enc-sj-003",
            "status": "finished",
            "class": {
                "system": "http://terminology.hl7.org/CodeSystem/v3-ActCode",
                "code": "AMB",
                "display": "ambulatory"
            },
            "subject": {"reference": "Patient/pat-sj-003"}
        }),
        json!({
            "resourceType": "Encounter",
            "id": "enc-sj-004",
            "status": "finished",
            "class": {
                "system": "http://terminology.hl7.org/CodeSystem/v3-ActCode",
                "code": "AMB",
                "display": "ambulatory"
            },
            "subject": {"reference": "Patient/pat-sj-004"}
        }),
    ]
}

fn init_care_plans() -> Vec<Value> {
    vec![
        json!({
            "resourceType": "CarePlan",
            "id": "cp-sj-001",
            "status": "active",
            "intent": "plan",
            "title": "St. Jude Total Therapy Study XVII for Acute Lymphoblastic Leukemia",
            "subject": {"reference": "Patient/pat-sj-001"},
            "period": {"start": "2024-02-15"}
        }),
        json!({
            "resourceType": "CarePlan",
            "id": "cp-sj-002",
            "status": "active",
            "intent": "plan",
            "title": "Pediatric Asthma Action Plan",
            "subject": {"reference": "Patient/pat-sj-002"}
        }),
        json!({
            "resourceType": "CarePlan",
            "id": "cp-sj-003",
            "status": "active",
            "intent": "plan",
            "title": "Type 1 Diabetes Intensive Basal-Bolus Regimen",
            "subject": {"reference": "Patient/pat-sj-004"}
        }),
    ]
}
