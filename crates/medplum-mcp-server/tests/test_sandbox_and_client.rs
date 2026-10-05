//! Tests for In-Memory Clinical Sandbox, Resilient Client, and OpenAPI Mock Server.

use medplum_mcp_core::safety::SafetyViolationError;
use medplum_mcp_core::token_diet::DetailLevel;
use medplum_mcp_server::client::{ClientError, MedplumClient};
use medplum_mcp_server::mock_server::start_mock_server;
use medplum_mcp_server::sandbox::ClinicalSandbox;
use serde_json::json;

#[test]
fn test_sandbox_st_jude_dataset_population() {
    let sandbox = ClinicalSandbox::new_st_jude();
    let patients = sandbox.search_patients(None, None, None);
    assert!(
        patients.len() >= 10,
        "Expected at least 10 patients, got {}",
        patients.len()
    );

    // Verify key pediatric oncology patients exist
    let names: Vec<String> = patients
        .iter()
        .filter_map(|p| {
            p.get("name")
                .and_then(|n| n.as_array())
                .and_then(|arr| arr.first())
                .and_then(|official| official.get("text"))
                .and_then(|t| t.as_str())
                .map(|s| s.to_string())
        })
        .collect();

    assert!(names.iter().any(|n| n.contains("Sarah Chen")));
    assert!(names.iter().any(|n| n.contains("Liam Patel")));
    assert!(names.iter().any(|n| n.contains("Marcus Vance")));
    assert!(names.iter().any(|n| n.contains("Elena Rostova")));
    assert!(names.iter().any(|n| n.contains("Chloe Kim")));
    assert!(names.iter().any(|n| n.contains("David Miller")));
}

#[test]
fn test_sandbox_clinical_resources_and_patient_references() {
    let sandbox = ClinicalSandbox::new_st_jude();
    let patients = sandbox.search_patients(None, None, None);
    let patient_ids: std::collections::HashSet<String> = patients
        .iter()
        .filter_map(|p| {
            p.get("id")
                .and_then(|id| id.as_str())
                .map(|s| s.to_string())
        })
        .collect();

    let observations = sandbox.list_observations(None, None, None);
    let conditions = sandbox.list_conditions(None);
    let medications = sandbox.list_medications(None);
    let allergies = sandbox.list_allergies(None);
    let encounters = sandbox.list_encounters(None);
    let reports = sandbox.list_diagnostic_reports(None, None);
    let care_plans = sandbox.list_care_plans(None);

    assert!(observations.len() >= 10, "Expected rich observations");
    assert!(conditions.len() >= 4, "Expected key conditions");
    assert!(medications.len() >= 4, "Expected key medications");
    assert!(allergies.len() >= 3, "Expected allergies");
    assert!(encounters.len() >= 4, "Expected encounters");
    assert!(reports.len() >= 4, "Expected diagnostic reports");
    assert!(care_plans.len() >= 3, "Expected care plans");

    // Check specific clinical terms
    let obs_str = serde_json::to_string(&observations).unwrap();
    assert!(obs_str.contains("Neutrophil") || obs_str.contains("ANC"));
    assert!(obs_str.contains("Blast") || obs_str.contains("blast"));
    assert!(obs_str.contains("Creatinine") || obs_str.contains("creatinine"));

    let cond_str = serde_json::to_string(&conditions).unwrap();
    assert!(
        cond_str.contains("B-cell") || cond_str.contains("Leukemia") || cond_str.contains("ALL")
    );
    assert!(cond_str.contains("Neutropenia") || cond_str.contains("neutropenia"));
    assert!(cond_str.contains("Asthma") || cond_str.contains("asthma"));
    assert!(cond_str.contains("Diabetes") || cond_str.contains("diabetes"));

    let med_str = serde_json::to_string(&medications).unwrap();
    assert!(med_str.contains("Mercaptopurine") || med_str.contains("mercaptopurine"));
    assert!(med_str.contains("Methotrexate") || med_str.contains("methotrexate"));
    assert!(med_str.contains("Albuterol") || med_str.contains("albuterol"));
    assert!(med_str.contains("Insulin") || med_str.contains("insulin"));

    let allg_str = serde_json::to_string(&allergies).unwrap();
    assert!(allg_str.contains("Penicillin") || allg_str.contains("penicillin"));
    assert!(allg_str.contains("Sulfa") || allg_str.contains("sulfa"));
    assert!(allg_str.contains("Peanut") || allg_str.contains("peanut"));

    // Check subjects point to existing patients
    for obs in &observations {
        if let Some(sub) = obs
            .get("subject")
            .and_then(|s| s.get("reference"))
            .and_then(|r| r.as_str())
        {
            let pat_id = sub.trim_start_matches("Patient/");
            assert!(
                patient_ids.contains(pat_id),
                "Obs subject {} not in patients",
                sub
            );
        }
    }
}

#[test]
fn test_sandbox_search_patients_and_get() {
    let sandbox = ClinicalSandbox::new_st_jude();

    // Search by name
    let found_chen = sandbox.search_patients(Some("Sarah"), None, None);
    assert!(!found_chen.is_empty());
    assert_eq!(found_chen[0]["id"], "pat-sj-001");

    // Search by identifier
    let found_mrn = sandbox.search_patients(None, Some("MRN-SJ-100234"), None);
    assert_eq!(found_mrn.len(), 1);
    assert_eq!(found_mrn[0]["id"], "pat-sj-001");

    // Search by dob
    let found_dob = sandbox.search_patients(None, None, Some("2018-05-14"));
    assert!(!found_dob.is_empty());

    // Get single patient
    let pat = sandbox.get_patient("pat-sj-001");
    assert!(pat.is_some());
    assert_eq!(pat.unwrap()["id"], "pat-sj-001");

    let not_found = sandbox.get_patient("pat-nonexistent");
    assert!(not_found.is_none());
}

#[test]
fn test_sandbox_create_drafts_enforces_status() {
    let sandbox = ClinicalSandbox::new_st_jude();

    // Draft observation creation with status="preliminary"
    let obs = sandbox
        .create_observation_draft(
            "pat-sj-001",
            "26499-4",
            1200.0,
            "/uL",
            "Absolute Neutrophil Count",
        )
        .expect("draft observation creation should succeed");

    assert_eq!(obs["resourceType"], "Observation");
    assert_eq!(obs["status"], "preliminary");
    assert_eq!(obs["subject"]["reference"], "Patient/pat-sj-001");
    assert!(obs["id"].as_str().unwrap().starts_with("obs-"));

    // Draft medication creation with status="draft"
    let med = sandbox
        .create_medication_draft(
            "pat-sj-001",
            "6851",
            "50 mg/m2",
            "Take 1 tablet orally daily at bedtime",
            "Mercaptopurine",
        )
        .expect("draft medication creation should succeed");

    assert_eq!(med["resourceType"], "MedicationRequest");
    assert_eq!(med["status"], "draft");
    assert_eq!(med["intent"], "order");
    assert_eq!(med["subject"]["reference"], "Patient/pat-sj-001");
    assert!(med["id"].as_str().unwrap().starts_with("med-"));
}

#[tokio::test]
async fn test_medplum_client_demo_mode_token_distillation() {
    let sandbox = ClinicalSandbox::new_st_jude();
    let client = MedplumClient::new_demo(sandbox);

    // Raw/Full patient size vs Compact distilled size
    let full_patient = client
        .get_patient("pat-sj-001", None)
        .await
        .expect("get_patient succeeded")
        .expect("patient found");

    let compact_patient = client
        .get_patient("pat-sj-001", Some(DetailLevel::Compact))
        .await
        .expect("get_patient succeeded")
        .expect("patient found");

    let raw_len = serde_json::to_string(&full_patient).unwrap().len();
    let compact_len = serde_json::to_string(&compact_patient).unwrap().len();

    let reduction = (raw_len - compact_len) as f64 / raw_len as f64;
    assert!(
        reduction >= 0.70,
        "Expected significant reduction, got {:.2}% (raw: {}, compact: {})",
        reduction * 100.0,
        raw_len,
        compact_len
    );
    assert_eq!(compact_patient["resourceType"], "Patient");
    assert_eq!(compact_patient["id"], "pat-sj-001");
}

#[tokio::test]
async fn test_medplum_client_safety_gates() {
    let sandbox = ClinicalSandbox::new_st_jude();
    let client_read_only = MedplumClient::new_demo(sandbox.clone()).with_allow_writes(false);

    // 1. Blocked writes when allow_writes=false
    let blocked_err = client_read_only
        .create_medication_draft("pat-sj-001", "6851", "50 mg/m2", "daily", "Mercaptopurine")
        .await;

    assert!(matches!(blocked_err, Err(ClientError::WritesBlocked)));

    // 2. Blocked forbidden status even when allow_writes=true
    let client_write_enabled = MedplumClient::new_demo(sandbox).with_allow_writes(true);

    let forbidden_med = json!({
        "resourceType": "MedicationRequest",
        "status": "active", // Forbidden committed state!
        "intent": "order",
        "subject": {
            "reference": "Patient/pat-sj-001"
        }
    });

    let forbidden_err = client_write_enabled
        .create_medication_request(forbidden_med)
        .await;
    assert!(matches!(
        forbidden_err,
        Err(ClientError::SafetyViolation(
            SafetyViolationError::ForbiddenTerminalStatus { .. }
        ))
    ));
}

#[tokio::test]
async fn test_mock_server_full_fhir_r4_endpoints() {
    let sandbox = ClinicalSandbox::new_st_jude();
    let server_handle = start_mock_server(0, sandbox)
        .await
        .expect("mock server started");
    let base_url = server_handle.base_url();
    let http = reqwest::Client::new();

    // 1. POST /oauth2/token
    let token_resp = http
        .post(format!("{}/oauth2/token", base_url))
        .json(&json!({"grant_type": "client_credentials"}))
        .send()
        .await
        .expect("token endpoint response");

    assert_eq!(token_resp.status(), reqwest::StatusCode::OK);
    let token_body: serde_json::Value = token_resp.json().await.unwrap();
    assert_eq!(token_body["access_token"], "mock-token");
    assert_eq!(token_body["token_type"], "Bearer");
    assert_eq!(token_body["expires_in"], 7200);

    // 2. GET /fhir/R4/Patient with rate limit header
    let pat_resp = http
        .get(format!("{}/fhir/R4/Patient", base_url))
        .send()
        .await
        .expect("patient search response");

    assert_eq!(pat_resp.status(), reqwest::StatusCode::OK);
    assert_eq!(pat_resp.headers().get("X-Rate-Limit-Limit").unwrap(), "100");
    let pat_bundle: serde_json::Value = pat_resp.json().await.unwrap();
    assert_eq!(pat_bundle["resourceType"], "Bundle");
    let total = pat_bundle["total"].as_u64().unwrap();
    assert!(total >= 10);
    assert!(!pat_bundle["entry"].as_array().unwrap().is_empty());

    // 3. GET /fhir/R4/Patient/{id}
    let single_pat_resp = http
        .get(format!("{}/fhir/R4/Patient/pat-sj-001", base_url))
        .send()
        .await
        .expect("single patient response");
    assert_eq!(single_pat_resp.status(), reqwest::StatusCode::OK);
    let single_pat: serde_json::Value = single_pat_resp.json().await.unwrap();
    assert_eq!(single_pat["id"], "pat-sj-001");

    let missing_pat_resp = http
        .get(format!("{}/fhir/R4/Patient/nonexistent-999", base_url))
        .send()
        .await
        .expect("missing patient response");
    assert_eq!(missing_pat_resp.status(), reqwest::StatusCode::NOT_FOUND);

    // 4. GET /fhir/R4/Observation?patient={id}
    let obs_resp = http
        .get(format!(
            "{}/fhir/R4/Observation?patient=pat-sj-001",
            base_url
        ))
        .send()
        .await
        .expect("obs response");
    assert_eq!(obs_resp.status(), reqwest::StatusCode::OK);
    let obs_bundle: serde_json::Value = obs_resp.json().await.unwrap();
    assert_eq!(obs_bundle["resourceType"], "Bundle");
    assert!(!obs_bundle["entry"].as_array().unwrap().is_empty());

    // 5. POST /fhir/R4/Observation -> 201 Created
    let new_draft_obs = json!({
        "resourceType": "Observation",
        "status": "preliminary",
        "code": {
            "coding": [{
                "system": "http://loinc.org",
                "code": "26499-4",
                "display": "Absolute Neutrophil Count"
            }]
        },
        "subject": {
            "reference": "Patient/pat-sj-001"
        },
        "valueQuantity": {
            "value": 1450.0,
            "unit": "/uL"
        }
    });

    let create_obs_resp = http
        .post(format!("{}/fhir/R4/Observation", base_url))
        .json(&new_draft_obs)
        .send()
        .await
        .expect("create obs response");

    assert_eq!(create_obs_resp.status(), reqwest::StatusCode::CREATED);
    let created_obs: serde_json::Value = create_obs_resp.json().await.unwrap();
    assert_eq!(created_obs["resourceType"], "Observation");
    assert_eq!(created_obs["status"], "preliminary");

    // 6. GET /fhir/R4/MedicationRequest
    let med_resp = http
        .get(format!(
            "{}/fhir/R4/MedicationRequest?patient=pat-sj-001",
            base_url
        ))
        .send()
        .await
        .expect("med response");
    assert_eq!(med_resp.status(), reqwest::StatusCode::OK);
    let med_bundle: serde_json::Value = med_resp.json().await.unwrap();
    assert_eq!(med_bundle["resourceType"], "Bundle");

    // 7. POST /fhir/R4/MedicationRequest -> 201 Created
    let new_draft_med = json!({
        "resourceType": "MedicationRequest",
        "status": "draft",
        "intent": "order",
        "subject": {
            "reference": "Patient/pat-sj-001"
        },
        "medicationCodeableConcept": {
            "text": "Methotrexate 20mg/m2"
        }
    });

    let create_med_resp = http
        .post(format!("{}/fhir/R4/MedicationRequest", base_url))
        .json(&new_draft_med)
        .send()
        .await
        .expect("create med response");

    assert_eq!(create_med_resp.status(), reqwest::StatusCode::CREATED);
    let created_med: serde_json::Value = create_med_resp.json().await.unwrap();
    assert_eq!(created_med["resourceType"], "MedicationRequest");
    assert_eq!(created_med["status"], "draft");

    // POST with forbidden status -> rejected
    let forbidden_med = json!({
        "resourceType": "MedicationRequest",
        "status": "active", // Forbidden!
        "intent": "order",
        "subject": {
            "reference": "Patient/pat-sj-001"
        }
    });
    let forbidden_resp = http
        .post(format!("{}/fhir/R4/MedicationRequest", base_url))
        .json(&forbidden_med)
        .send()
        .await
        .expect("forbidden med response");
    assert_eq!(
        forbidden_resp.status(),
        reqwest::StatusCode::UNPROCESSABLE_ENTITY
    );

    // 8. GET /fhir/R4/Condition
    let cond_resp = http
        .get(format!("{}/fhir/R4/Condition", base_url))
        .send()
        .await
        .expect("condition response");
    assert_eq!(cond_resp.status(), reqwest::StatusCode::OK);

    // 9. GET /fhir/R4/AllergyIntolerance
    let allg_resp = http
        .get(format!("{}/fhir/R4/AllergyIntolerance", base_url))
        .send()
        .await
        .expect("allergy response");
    assert_eq!(allg_resp.status(), reqwest::StatusCode::OK);

    // 10. GET /fhir/R4/DiagnosticReport
    let rep_resp = http
        .get(format!("{}/fhir/R4/DiagnosticReport", base_url))
        .send()
        .await
        .expect("report response");
    assert_eq!(rep_resp.status(), reqwest::StatusCode::OK);

    // 11. GET /fhir/R4/Encounter
    let enc_resp = http
        .get(format!("{}/fhir/R4/Encounter", base_url))
        .send()
        .await
        .expect("encounter response");
    assert_eq!(enc_resp.status(), reqwest::StatusCode::OK);

    // 12. GET /fhir/R4/CarePlan
    let cp_resp = http
        .get(format!("{}/fhir/R4/CarePlan", base_url))
        .send()
        .await
        .expect("care plan response");
    assert_eq!(cp_resp.status(), reqwest::StatusCode::OK);
}
