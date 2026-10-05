//! Exhaustive Integration and Unit Tests for all 16 Medplum Clinical Tools.
//!
//! Validates parameter permutations, ID prefixes, detail level distillation (Compact,
//! Standard, Executive), error handling, permission checks, and audit logging.

use std::sync::{Arc, Mutex};

use medplum_mcp_core::audit::{ActionStatus, AuditLogManager};
use medplum_mcp_server::client::MedplumClient;
use medplum_mcp_server::mcp::McpServer;
use serde_json::{json, Value};

/// Helper to invoke a tool via JSON-RPC 2.0 and return `(is_error, parsed_content_or_error)`.
async fn call_tool(server: &McpServer, name: &str, args: Value) -> (bool, Value) {
    let req = json!({
        "jsonrpc": "2.0",
        "id": 100,
        "method": "tools/call",
        "params": {
            "name": name,
            "arguments": args
        }
    });

    let resp_str = server
        .handle_jsonrpc_message(&req.to_string())
        .await
        .expect("response");
    let resp: Value = serde_json::from_str(&resp_str).expect("valid json response");

    if let Some(err) = resp.get("error") {
        return (true, err.clone());
    }

    let result = &resp["result"];
    let is_error = result["isError"].as_bool().unwrap_or(false);
    let text = result["content"][0]["text"].as_str().unwrap_or("");
    let parsed: Value =
        serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_string()));
    (is_error, parsed)
}

fn create_test_server(allow_writes: bool) -> McpServer {
    let client = MedplumClient::new_demo_default().with_allow_writes(allow_writes);
    McpServer::new(client, None, allow_writes)
}

fn create_audited_server(
    allow_writes: bool,
) -> (McpServer, Arc<Mutex<AuditLogManager>>, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let audit_file = tmp.path().join("audit.jsonl");
    let mgr = Arc::new(Mutex::new(
        AuditLogManager::new(&audit_file, b"test-secret-key-32-bytes-long!").unwrap(),
    ));
    let client = MedplumClient::new_demo_default().with_allow_writes(allow_writes);
    let server = McpServer::new(client, Some(mgr.clone()), allow_writes);
    (server, mgr, tmp)
}

// ===========================================================================
// 1. medplum_search_patients
// ===========================================================================

#[tokio::test]
async fn test_tool_search_patients_by_name() {
    let server = create_test_server(false);
    let (err, res) = call_tool(&server, "medplum_search_patients", json!({"name": "Lin"})).await;
    assert!(!err);
    assert_eq!(res["resourceType"], "Bundle");
    let entries = res["entry"].as_array().expect("bundle entries");
    assert!(!entries.is_empty());
    let pat = &entries[0]["resource"];
    assert_eq!(pat["resourceType"], "Patient");
}

#[tokio::test]
async fn test_tool_search_patients_by_identifier() {
    let server = create_test_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_search_patients",
        json!({"identifier": "MRN-SJ-100234"}),
    )
    .await;
    assert!(!err);
    assert_eq!(res["resourceType"], "Bundle");
    let entries = res["entry"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["resource"]["id"], "pat-sj-001");
}

#[tokio::test]
async fn test_tool_search_patients_by_dob() {
    let server = create_test_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_search_patients",
        json!({"dob": "2018-05-14"}),
    )
    .await;
    assert!(!err);
    assert_eq!(res["resourceType"], "Bundle");
    let entries = res["entry"].as_array().unwrap();
    assert!(!entries.is_empty());
    assert_eq!(entries[0]["resource"]["birthDate"], "2018-05-14");
}

#[tokio::test]
async fn test_tool_search_patients_combined_filters() {
    let server = create_test_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_search_patients",
        json!({
            "name": "Chen",
            "identifier": "MRN-SJ-100234",
            "dob": "2018-05-14"
        }),
    )
    .await;
    assert!(!err);
    let entries = res["entry"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["resource"]["id"], "pat-sj-001");
}

#[tokio::test]
async fn test_tool_search_patients_non_matching_returns_empty_bundle() {
    let server = create_test_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_search_patients",
        json!({"name": "NonExistentNameXYZ123"}),
    )
    .await;
    assert!(!err);
    assert_eq!(res["resourceType"], "Bundle");
    let entries = res["entry"].as_array().map(|a| a.len()).unwrap_or(0);
    assert_eq!(entries, 0);
    assert_eq!(res["total"], 0);
}

#[tokio::test]
async fn test_tool_search_patients_detail_levels() {
    let server = create_test_server(false);
    for tier in &["compact", "standard", "executive"] {
        let (err, res) = call_tool(
            &server,
            "medplum_search_patients",
            json!({"name": "Lin", "detail_level": tier}),
        )
        .await;
        assert!(!err, "Search failed for tier {}", tier);
        assert_eq!(res["resourceType"], "Bundle");
    }
}

// ===========================================================================
// 2. medplum_get_patient
// ===========================================================================

#[tokio::test]
async fn test_tool_get_patient_valid_id() {
    let server = create_test_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_get_patient",
        json!({"patient_id": "pat-sj-001"}),
    )
    .await;
    assert!(!err);
    assert_eq!(res["resourceType"], "Patient");
    assert_eq!(res["id"], "pat-sj-001");
}

#[tokio::test]
async fn test_tool_get_patient_prefixed_id() {
    let server = create_test_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_get_patient",
        json!({"patient_id": "Patient/pat-sj-001"}),
    )
    .await;
    assert!(!err);
    assert_eq!(res["resourceType"], "Patient");
    assert_eq!(res["id"], "pat-sj-001");
}

#[tokio::test]
async fn test_tool_get_patient_non_existent_id() {
    let server = create_test_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_get_patient",
        json!({"patient_id": "nonexistent-patient-999"}),
    )
    .await;
    assert!(err);
    let err_msg = res.as_str().unwrap_or("");
    assert!(err_msg.contains("not found"));
}

#[tokio::test]
async fn test_tool_get_patient_detail_levels() {
    let server = create_test_server(false);

    // Compact
    let (err1, compact) = call_tool(
        &server,
        "medplum_get_patient",
        json!({"patient_id": "pat-sj-001", "detail_level": "compact"}),
    )
    .await;
    assert!(!err1);
    assert_eq!(compact["resourceType"], "Patient");
    assert_eq!(compact["id"], "pat-sj-001");

    // Standard
    let (err2, standard) = call_tool(
        &server,
        "medplum_get_patient",
        json!({"patient_id": "pat-sj-001", "detail_level": "standard"}),
    )
    .await;
    assert!(!err2);
    assert_eq!(standard["resourceType"], "Patient");

    // Executive
    let (err3, executive) = call_tool(
        &server,
        "medplum_get_patient",
        json!({"patient_id": "pat-sj-001", "detail_level": "executive"}),
    )
    .await;
    assert!(!err3);
    assert_eq!(executive["resourceType"], "Patient");
}

// ===========================================================================
// 3. medplum_list_observations
// ===========================================================================

#[tokio::test]
async fn test_tool_list_observations_all() {
    let server = create_test_server(false);
    let (err, res) = call_tool(&server, "medplum_list_observations", json!({})).await;
    assert!(!err);
    assert_eq!(res["resourceType"], "Bundle");
    let entries = res["entry"].as_array().unwrap();
    assert!(!entries.is_empty());
}

#[tokio::test]
async fn test_tool_list_observations_by_patient_id() {
    let server = create_test_server(false);
    // Bare ID
    let (err1, res1) = call_tool(
        &server,
        "medplum_list_observations",
        json!({"patient_id": "pat-sj-001"}),
    )
    .await;
    assert!(!err1);
    let count1 = res1["entry"].as_array().unwrap().len();
    assert!(count1 > 0);

    // Prefixed ID
    let (err2, res2) = call_tool(
        &server,
        "medplum_list_observations",
        json!({"patient_id": "Patient/pat-sj-001"}),
    )
    .await;
    assert!(!err2);
    let count2 = res2["entry"].as_array().unwrap().len();
    assert_eq!(count1, count2);
}

#[tokio::test]
async fn test_tool_list_observations_by_category() {
    let server = create_test_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_list_observations",
        json!({"category": "laboratory"}),
    )
    .await;
    assert!(!err);
    assert_eq!(res["resourceType"], "Bundle");
    let entries = res["entry"].as_array().unwrap();
    assert!(!entries.is_empty());
}

#[tokio::test]
async fn test_tool_list_observations_by_loinc_code() {
    let server = create_test_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_list_observations",
        json!({"code": "26499-4"}),
    )
    .await;
    assert!(!err);
    let entries = res["entry"].as_array().unwrap();
    assert!(!entries.is_empty());
}

#[tokio::test]
async fn test_tool_list_observations_detail_levels() {
    let server = create_test_server(false);
    for tier in &["compact", "standard", "executive"] {
        let (err, res) = call_tool(
            &server,
            "medplum_list_observations",
            json!({"patient_id": "pat-sj-001", "detail_level": tier}),
        )
        .await;
        assert!(!err, "list_observations failed for tier {}", tier);
        assert_eq!(res["resourceType"], "Bundle");
    }
}

// ===========================================================================
// 4. medplum_get_observation
// ===========================================================================

#[tokio::test]
async fn test_tool_get_observation_valid_id() {
    let server = create_test_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_get_observation",
        json!({"observation_id": "obs-sj-001"}),
    )
    .await;
    assert!(!err);
    assert_eq!(res["resourceType"], "Observation");
    assert_eq!(res["id"], "obs-sj-001");
}

#[tokio::test]
async fn test_tool_get_observation_prefixed_id() {
    let server = create_test_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_get_observation",
        json!({"observation_id": "Observation/obs-sj-001"}),
    )
    .await;
    assert!(!err);
    assert_eq!(res["resourceType"], "Observation");
    assert_eq!(res["id"], "obs-sj-001");
}

#[tokio::test]
async fn test_tool_get_observation_non_existent_id() {
    let server = create_test_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_get_observation",
        json!({"observation_id": "obs-nonexistent-999"}),
    )
    .await;
    assert!(err);
    assert!(res.as_str().unwrap_or("").contains("not found"));
}

#[tokio::test]
async fn test_tool_get_observation_detail_levels() {
    let server = create_test_server(false);
    for tier in &["compact", "standard", "executive"] {
        let (err, res) = call_tool(
            &server,
            "medplum_get_observation",
            json!({"observation_id": "obs-sj-001", "detail_level": tier}),
        )
        .await;
        assert!(!err, "get_observation failed for tier {}", tier);
        assert_eq!(res["resourceType"], "Observation");
    }
}

// ===========================================================================
// 5. medplum_create_observation_draft
// ===========================================================================

#[tokio::test]
async fn test_tool_create_observation_draft_writes_allowed() {
    let server = create_test_server(true);
    let (err, res) = call_tool(
        &server,
        "medplum_create_observation_draft",
        json!({
            "patient_id": "pat-sj-001",
            "code": "26499-4",
            "value_quantity": 1800.0,
            "unit": "/uL",
            "display": "Absolute Neutrophil Count",
            "status": "preliminary"
        }),
    )
    .await;
    assert!(!err);
    assert_eq!(res["resourceType"], "Observation");
    assert_eq!(res["status"], "preliminary");
    assert!(res["id"].as_str().unwrap().starts_with("obs-"));
}

#[tokio::test]
async fn test_tool_create_observation_draft_writes_disabled() {
    let (server, audit, _tmp) = create_audited_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_create_observation_draft",
        json!({
            "patient_id": "pat-sj-001",
            "code": "26499-4",
            "value_quantity": 1800.0,
            "unit": "/uL"
        }),
    )
    .await;
    assert!(err);
    assert!(res.as_str().unwrap_or("").contains("disabled"));

    let entries = audit.lock().unwrap().get_entries().unwrap();
    assert_eq!(entries.last().unwrap().action_status, ActionStatus::Blocked);
}

#[tokio::test]
async fn test_tool_create_observation_draft_forbidden_status_final() {
    let (server, audit, _tmp) = create_audited_server(true);
    let (err, res) = call_tool(
        &server,
        "medplum_create_observation_draft",
        json!({
            "patient_id": "pat-sj-001",
            "code": "26499-4",
            "value_quantity": 1800.0,
            "unit": "/uL",
            "status": "final"
        }),
    )
    .await;
    assert!(err);
    assert!(res.as_str().unwrap_or("").contains("Safety violation"));

    let entries = audit.lock().unwrap().get_entries().unwrap();
    assert_eq!(entries.last().unwrap().action_status, ActionStatus::Blocked);
}

#[tokio::test]
async fn test_tool_create_observation_draft_forbidden_status_amended() {
    let (server, audit, _tmp) = create_audited_server(true);
    let (err, res) = call_tool(
        &server,
        "medplum_create_observation_draft",
        json!({
            "patient_id": "pat-sj-001",
            "code": "26499-4",
            "value_quantity": 1800.0,
            "unit": "/uL",
            "status": "amended"
        }),
    )
    .await;
    assert!(err);
    assert!(res.as_str().unwrap_or("").contains("Safety violation"));

    let entries = audit.lock().unwrap().get_entries().unwrap();
    assert_eq!(entries.last().unwrap().action_status, ActionStatus::Blocked);
}

#[tokio::test]
async fn test_tool_create_observation_draft_missing_required_code() {
    let server = create_test_server(true);
    let (err, res) = call_tool(
        &server,
        "medplum_create_observation_draft",
        json!({
            "patient_id": "pat-sj-001",
            "value_quantity": 1800.0
        }),
    )
    .await;
    assert!(err);
    assert!(res.as_str().unwrap_or("").contains("Missing code"));
}

#[tokio::test]
async fn test_tool_create_observation_draft_missing_required_value() {
    let server = create_test_server(true);
    let (err, res) = call_tool(
        &server,
        "medplum_create_observation_draft",
        json!({
            "patient_id": "pat-sj-001",
            "code": "26499-4"
        }),
    )
    .await;
    assert!(err);
    assert!(res
        .as_str()
        .unwrap_or("")
        .contains("Missing value_quantity"));
}

#[tokio::test]
async fn test_tool_create_observation_draft_missing_required_patient_id() {
    let server = create_test_server(true);
    let (err, res) = call_tool(
        &server,
        "medplum_create_observation_draft",
        json!({
            "code": "26499-4",
            "value_quantity": 1800.0
        }),
    )
    .await;
    assert!(err);
    assert!(res.as_str().unwrap_or("").contains("Missing patient_id"));
}

// ===========================================================================
// 6. medplum_list_conditions
// ===========================================================================

#[tokio::test]
async fn test_tool_list_conditions_all() {
    let server = create_test_server(false);
    let (err, res) = call_tool(&server, "medplum_list_conditions", json!({})).await;
    assert!(!err);
    assert_eq!(res["resourceType"], "Bundle");
    let entries = res["entry"].as_array().unwrap();
    assert!(!entries.is_empty());
}

#[tokio::test]
async fn test_tool_list_conditions_by_patient() {
    let server = create_test_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_list_conditions",
        json!({"patient_id": "pat-sj-001"}),
    )
    .await;
    assert!(!err);
    assert_eq!(res["resourceType"], "Bundle");
    let entries = res["entry"].as_array().unwrap();
    assert!(!entries.is_empty());
}

#[tokio::test]
async fn test_tool_list_conditions_by_status() {
    let server = create_test_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_list_conditions",
        json!({"clinical_status": "active"}),
    )
    .await;
    assert!(!err);
    assert_eq!(res["resourceType"], "Bundle");
}

#[tokio::test]
async fn test_tool_list_conditions_detail_levels() {
    let server = create_test_server(false);
    for tier in &["compact", "standard", "executive"] {
        let (err, res) = call_tool(
            &server,
            "medplum_list_conditions",
            json!({"patient_id": "pat-sj-001", "detail_level": tier}),
        )
        .await;
        assert!(!err, "list_conditions failed for tier {}", tier);
        assert_eq!(res["resourceType"], "Bundle");
    }
}

// ===========================================================================
// 7. medplum_get_condition
// ===========================================================================

#[tokio::test]
async fn test_tool_get_condition_valid_id() {
    let server = create_test_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_get_condition",
        json!({"condition_id": "cond-sj-001"}),
    )
    .await;
    assert!(!err);
    assert_eq!(res["resourceType"], "Condition");
    assert_eq!(res["id"], "cond-sj-001");
}

#[tokio::test]
async fn test_tool_get_condition_prefixed_id() {
    let server = create_test_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_get_condition",
        json!({"condition_id": "Condition/cond-sj-001"}),
    )
    .await;
    assert!(!err);
    assert_eq!(res["resourceType"], "Condition");
    assert_eq!(res["id"], "cond-sj-001");
}

#[tokio::test]
async fn test_tool_get_condition_non_existent_id() {
    let server = create_test_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_get_condition",
        json!({"condition_id": "cond-nonexistent-999"}),
    )
    .await;
    assert!(err);
    assert!(res.as_str().unwrap_or("").contains("not found"));
}

#[tokio::test]
async fn test_tool_get_condition_detail_levels() {
    let server = create_test_server(false);
    for tier in &["compact", "standard", "executive"] {
        let (err, res) = call_tool(
            &server,
            "medplum_get_condition",
            json!({"condition_id": "cond-sj-001", "detail_level": tier}),
        )
        .await;
        assert!(!err, "get_condition failed for tier {}", tier);
        assert_eq!(res["resourceType"], "Condition");
    }
}

// ===========================================================================
// 8. medplum_list_medication_requests
// ===========================================================================

#[tokio::test]
async fn test_tool_list_medication_requests_all() {
    let server = create_test_server(false);
    let (err, res) = call_tool(&server, "medplum_list_medication_requests", json!({})).await;
    assert!(!err);
    assert_eq!(res["resourceType"], "Bundle");
    let entries = res["entry"].as_array().unwrap();
    assert!(!entries.is_empty());
}

#[tokio::test]
async fn test_tool_list_medication_requests_by_patient() {
    let server = create_test_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_list_medication_requests",
        json!({"patient_id": "pat-sj-001"}),
    )
    .await;
    assert!(!err);
    assert_eq!(res["resourceType"], "Bundle");
    let entries = res["entry"].as_array().unwrap();
    assert!(!entries.is_empty());
}

#[tokio::test]
async fn test_tool_list_medication_requests_detail_levels() {
    let server = create_test_server(false);
    for tier in &["compact", "standard", "executive"] {
        let (err, res) = call_tool(
            &server,
            "medplum_list_medication_requests",
            json!({"patient_id": "pat-sj-001", "detail_level": tier}),
        )
        .await;
        assert!(!err, "list_medications failed for tier {}", tier);
        assert_eq!(res["resourceType"], "Bundle");
    }
}

// ===========================================================================
// 9. medplum_get_medication_request
// ===========================================================================

#[tokio::test]
async fn test_tool_get_medication_request_valid_id() {
    let server = create_test_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_get_medication_request",
        json!({"request_id": "med-sj-001"}),
    )
    .await;
    assert!(!err);
    assert_eq!(res["resourceType"], "MedicationRequest");
    assert_eq!(res["id"], "med-sj-001");
}

#[tokio::test]
async fn test_tool_get_medication_request_prefixed_id() {
    let server = create_test_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_get_medication_request",
        json!({"request_id": "MedicationRequest/med-sj-001"}),
    )
    .await;
    assert!(!err);
    assert_eq!(res["resourceType"], "MedicationRequest");
    assert_eq!(res["id"], "med-sj-001");
}

#[tokio::test]
async fn test_tool_get_medication_request_non_existent_id() {
    let server = create_test_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_get_medication_request",
        json!({"request_id": "med-nonexistent-999"}),
    )
    .await;
    assert!(err);
    assert!(res.as_str().unwrap_or("").contains("not found"));
}

#[tokio::test]
async fn test_tool_get_medication_request_detail_levels() {
    let server = create_test_server(false);
    for tier in &["compact", "standard", "executive"] {
        let (err, res) = call_tool(
            &server,
            "medplum_get_medication_request",
            json!({"request_id": "med-sj-001", "detail_level": tier}),
        )
        .await;
        assert!(!err, "get_medication failed for tier {}", tier);
        assert_eq!(res["resourceType"], "MedicationRequest");
    }
}

// ===========================================================================
// 10. medplum_create_medication_draft
// ===========================================================================

#[tokio::test]
async fn test_tool_create_medication_draft_writes_allowed() {
    let server = create_test_server(true);
    let (err, res) = call_tool(
        &server,
        "medplum_create_medication_draft",
        json!({
            "patient_id": "pat-sj-001",
            "medication_code": "6851",
            "medication_display": "Mercaptopurine 50mg",
            "dosage": "50 mg/m2",
            "instructions": "Take daily at bedtime",
            "status": "draft"
        }),
    )
    .await;
    assert!(!err);
    assert_eq!(res["resourceType"], "MedicationRequest");
    assert_eq!(res["status"], "draft");
    assert!(res["id"].as_str().unwrap().starts_with("med-"));
}

#[tokio::test]
async fn test_tool_create_medication_draft_writes_disabled() {
    let (server, audit, _tmp) = create_audited_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_create_medication_draft",
        json!({
            "patient_id": "pat-sj-001",
            "medication_code": "6851",
            "dosage": "50 mg/m2"
        }),
    )
    .await;
    assert!(err);
    assert!(res.as_str().unwrap_or("").contains("disabled"));

    let entries = audit.lock().unwrap().get_entries().unwrap();
    assert_eq!(entries.last().unwrap().action_status, ActionStatus::Blocked);
}

#[tokio::test]
async fn test_tool_create_medication_draft_forbidden_status_active() {
    let (server, audit, _tmp) = create_audited_server(true);
    let (err, res) = call_tool(
        &server,
        "medplum_create_medication_draft",
        json!({
            "patient_id": "pat-sj-001",
            "medication_code": "6851",
            "status": "active"
        }),
    )
    .await;
    assert!(err);
    assert!(res.as_str().unwrap_or("").contains("Safety violation"));

    let entries = audit.lock().unwrap().get_entries().unwrap();
    assert_eq!(entries.last().unwrap().action_status, ActionStatus::Blocked);
}

#[tokio::test]
async fn test_tool_create_medication_draft_forbidden_status_completed() {
    let (server, audit, _tmp) = create_audited_server(true);
    let (err, res) = call_tool(
        &server,
        "medplum_create_medication_draft",
        json!({
            "patient_id": "pat-sj-001",
            "medication_code": "6851",
            "status": "completed"
        }),
    )
    .await;
    assert!(err);
    assert!(res.as_str().unwrap_or("").contains("Safety violation"));

    let entries = audit.lock().unwrap().get_entries().unwrap();
    assert_eq!(entries.last().unwrap().action_status, ActionStatus::Blocked);
}

#[tokio::test]
async fn test_tool_create_medication_draft_missing_medication_code() {
    let server = create_test_server(true);
    let (err, res) = call_tool(
        &server,
        "medplum_create_medication_draft",
        json!({
            "patient_id": "pat-sj-001",
            "dosage": "50 mg/m2"
        }),
    )
    .await;
    assert!(err);
    assert!(res
        .as_str()
        .unwrap_or("")
        .contains("Missing medication_code"));
}

#[tokio::test]
async fn test_tool_create_medication_draft_missing_patient_id() {
    let server = create_test_server(true);
    let (err, res) = call_tool(
        &server,
        "medplum_create_medication_draft",
        json!({
            "medication_code": "6851",
            "dosage": "50 mg/m2"
        }),
    )
    .await;
    assert!(err);
    assert!(res.as_str().unwrap_or("").contains("Missing patient_id"));
}

// ===========================================================================
// 11. medplum_list_allergy_intolerances
// ===========================================================================

#[tokio::test]
async fn test_tool_list_allergy_intolerances_all() {
    let server = create_test_server(false);
    let (err, res) = call_tool(&server, "medplum_list_allergy_intolerances", json!({})).await;
    assert!(!err);
    assert_eq!(res["resourceType"], "Bundle");
    let entries = res["entry"].as_array().unwrap();
    assert!(!entries.is_empty());
}

#[tokio::test]
async fn test_tool_list_allergy_intolerances_by_patient() {
    let server = create_test_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_list_allergy_intolerances",
        json!({"patient_id": "pat-sj-001"}),
    )
    .await;
    assert!(!err);
    assert_eq!(res["resourceType"], "Bundle");
    let entries = res["entry"].as_array().unwrap();
    assert!(!entries.is_empty());
}

#[tokio::test]
async fn test_tool_list_allergy_intolerances_detail_levels() {
    let server = create_test_server(false);
    for tier in &["compact", "standard", "executive"] {
        let (err, res) = call_tool(
            &server,
            "medplum_list_allergy_intolerances",
            json!({"patient_id": "pat-sj-001", "detail_level": tier}),
        )
        .await;
        assert!(!err, "list_allergies failed for tier {}", tier);
        assert_eq!(res["resourceType"], "Bundle");
    }
}

// ===========================================================================
// 12. medplum_get_allergy_intolerance
// ===========================================================================

#[tokio::test]
async fn test_tool_get_allergy_intolerance_valid_id() {
    let server = create_test_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_get_allergy_intolerance",
        json!({"allergy_id": "allg-sj-001"}),
    )
    .await;
    assert!(!err);
    assert_eq!(res["resourceType"], "AllergyIntolerance");
    assert_eq!(res["id"], "allg-sj-001");
}

#[tokio::test]
async fn test_tool_get_allergy_intolerance_prefixed_id() {
    let server = create_test_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_get_allergy_intolerance",
        json!({"allergy_id": "AllergyIntolerance/allg-sj-001"}),
    )
    .await;
    assert!(!err);
    assert_eq!(res["resourceType"], "AllergyIntolerance");
    assert_eq!(res["id"], "allg-sj-001");
}

#[tokio::test]
async fn test_tool_get_allergy_intolerance_non_existent_id() {
    let server = create_test_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_get_allergy_intolerance",
        json!({"allergy_id": "allg-nonexistent-999"}),
    )
    .await;
    assert!(err);
    assert!(res.as_str().unwrap_or("").contains("not found"));
}

#[tokio::test]
async fn test_tool_get_allergy_intolerance_detail_levels() {
    let server = create_test_server(false);
    for tier in &["compact", "standard", "executive"] {
        let (err, res) = call_tool(
            &server,
            "medplum_get_allergy_intolerance",
            json!({"allergy_id": "allg-sj-001", "detail_level": tier}),
        )
        .await;
        assert!(!err, "get_allergy failed for tier {}", tier);
        assert_eq!(res["resourceType"], "AllergyIntolerance");
    }
}

// ===========================================================================
// 13. medplum_list_diagnostic_reports
// ===========================================================================

#[tokio::test]
async fn test_tool_list_diagnostic_reports_all() {
    let server = create_test_server(false);
    let (err, res) = call_tool(&server, "medplum_list_diagnostic_reports", json!({})).await;
    assert!(!err);
    assert_eq!(res["resourceType"], "Bundle");
    let entries = res["entry"].as_array().unwrap();
    assert!(!entries.is_empty());
}

#[tokio::test]
async fn test_tool_list_diagnostic_reports_by_patient() {
    let server = create_test_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_list_diagnostic_reports",
        json!({"patient_id": "pat-sj-001"}),
    )
    .await;
    assert!(!err);
    assert_eq!(res["resourceType"], "Bundle");
    let entries = res["entry"].as_array().unwrap();
    assert!(!entries.is_empty());
}

#[tokio::test]
async fn test_tool_list_diagnostic_reports_by_code() {
    let server = create_test_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_list_diagnostic_reports",
        json!({"code": "26436-6"}),
    )
    .await;
    assert!(!err);
    assert_eq!(res["resourceType"], "Bundle");
}

#[tokio::test]
async fn test_tool_list_diagnostic_reports_detail_levels() {
    let server = create_test_server(false);
    for tier in &["compact", "standard", "executive"] {
        let (err, res) = call_tool(
            &server,
            "medplum_list_diagnostic_reports",
            json!({"patient_id": "pat-sj-001", "detail_level": tier}),
        )
        .await;
        assert!(!err, "list_diagnostic_reports failed for tier {}", tier);
        assert_eq!(res["resourceType"], "Bundle");
    }
}

// ===========================================================================
// 14. medplum_get_diagnostic_report
// ===========================================================================

#[tokio::test]
async fn test_tool_get_diagnostic_report_valid_id() {
    let server = create_test_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_get_diagnostic_report",
        json!({"report_id": "rep-sj-001"}),
    )
    .await;
    assert!(!err);
    assert_eq!(res["resourceType"], "DiagnosticReport");
    assert_eq!(res["id"], "rep-sj-001");
}

#[tokio::test]
async fn test_tool_get_diagnostic_report_prefixed_id() {
    let server = create_test_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_get_diagnostic_report",
        json!({"report_id": "DiagnosticReport/rep-sj-001"}),
    )
    .await;
    assert!(!err);
    assert_eq!(res["resourceType"], "DiagnosticReport");
    assert_eq!(res["id"], "rep-sj-001");
}

#[tokio::test]
async fn test_tool_get_diagnostic_report_non_existent_id() {
    let server = create_test_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_get_diagnostic_report",
        json!({"report_id": "rep-nonexistent-999"}),
    )
    .await;
    assert!(err);
    assert!(res.as_str().unwrap_or("").contains("not found"));
}

#[tokio::test]
async fn test_tool_get_diagnostic_report_detail_levels() {
    let server = create_test_server(false);
    for tier in &["compact", "standard", "executive"] {
        let (err, res) = call_tool(
            &server,
            "medplum_get_diagnostic_report",
            json!({"report_id": "rep-sj-001", "detail_level": tier}),
        )
        .await;
        assert!(!err, "get_diagnostic_report failed for tier {}", tier);
        assert_eq!(res["resourceType"], "DiagnosticReport");
    }
}

// ===========================================================================
// 15. medplum_list_encounters
// ===========================================================================

#[tokio::test]
async fn test_tool_list_encounters_all() {
    let server = create_test_server(false);
    let (err, res) = call_tool(&server, "medplum_list_encounters", json!({})).await;
    assert!(!err);
    assert_eq!(res["resourceType"], "Bundle");
    let entries = res["entry"].as_array().unwrap();
    assert!(!entries.is_empty());
}

#[tokio::test]
async fn test_tool_list_encounters_by_patient() {
    let server = create_test_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_list_encounters",
        json!({"patient_id": "pat-sj-001"}),
    )
    .await;
    assert!(!err);
    assert_eq!(res["resourceType"], "Bundle");
    let entries = res["entry"].as_array().unwrap();
    assert!(!entries.is_empty());
}

#[tokio::test]
async fn test_tool_list_encounters_non_matching_patient() {
    let server = create_test_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_list_encounters",
        json!({"patient_id": "pat-nonexistent-999"}),
    )
    .await;
    assert!(!err);
    assert_eq!(res["resourceType"], "Bundle");
    let count = res["entry"].as_array().map(|a| a.len()).unwrap_or(0);
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_tool_list_encounters_detail_levels() {
    let server = create_test_server(false);
    for tier in &["compact", "standard", "executive"] {
        let (err, res) = call_tool(
            &server,
            "medplum_list_encounters",
            json!({"patient_id": "pat-sj-001", "detail_level": tier}),
        )
        .await;
        assert!(!err, "list_encounters failed for tier {}", tier);
        assert_eq!(res["resourceType"], "Bundle");
    }
}

// ===========================================================================
// 16. medplum_list_care_plans
// ===========================================================================

#[tokio::test]
async fn test_tool_list_care_plans_all() {
    let server = create_test_server(false);
    let (err, res) = call_tool(&server, "medplum_list_care_plans", json!({})).await;
    assert!(!err);
    assert_eq!(res["resourceType"], "Bundle");
    let entries = res["entry"].as_array().unwrap();
    assert!(!entries.is_empty());
}

#[tokio::test]
async fn test_tool_list_care_plans_by_patient() {
    let server = create_test_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_list_care_plans",
        json!({"patient_id": "pat-sj-001"}),
    )
    .await;
    assert!(!err);
    assert_eq!(res["resourceType"], "Bundle");
    let entries = res["entry"].as_array().unwrap();
    assert!(!entries.is_empty());
}

#[tokio::test]
async fn test_tool_list_care_plans_non_matching_patient() {
    let server = create_test_server(false);
    let (err, res) = call_tool(
        &server,
        "medplum_list_care_plans",
        json!({"patient_id": "pat-nonexistent-999"}),
    )
    .await;
    assert!(!err);
    assert_eq!(res["resourceType"], "Bundle");
    let count = res["entry"].as_array().map(|a| a.len()).unwrap_or(0);
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_tool_list_care_plans_detail_levels() {
    let server = create_test_server(false);
    for tier in &["compact", "standard", "executive"] {
        let (err, res) = call_tool(
            &server,
            "medplum_list_care_plans",
            json!({"patient_id": "pat-sj-001", "detail_level": tier}),
        )
        .await;
        assert!(!err, "list_care_plans failed for tier {}", tier);
        assert_eq!(res["resourceType"], "Bundle");
    }
}

// ===========================================================================
// Protocol and Error Cases
// ===========================================================================

#[tokio::test]
async fn test_tool_unknown_tool_call_returns_error() {
    let server = create_test_server(false);
    let (err, res) = call_tool(&server, "medplum_completely_nonexistent_tool", json!({})).await;
    assert!(err);
    assert!(res.as_str().unwrap_or("").contains("not found"));
}

#[tokio::test]
async fn test_tool_missing_name_returns_error() {
    let server = create_test_server(false);
    let (err, res) = call_tool(&server, "", json!({})).await;
    assert!(err);
    assert!(res.as_str().unwrap_or("").contains("not found"));
}

#[tokio::test]
async fn test_tool_call_malformed_json_returns_parse_error() {
    let server = create_test_server(false);
    let resp_str = server
        .handle_jsonrpc_message("{invalid-json-body")
        .await
        .expect("response");
    let resp: Value = serde_json::from_str(&resp_str).expect("valid json response");
    assert_eq!(resp["error"]["code"], -32700);
}

#[tokio::test]
async fn test_tool_call_missing_required_params_reports_error() {
    let server = create_test_server(false);
    // get_patient with no arguments
    let (err, res) = call_tool(&server, "medplum_get_patient", json!({})).await;
    assert!(err);
    assert!(res
        .as_str()
        .unwrap_or("")
        .contains("Missing required parameter: patient_id"));
}

#[tokio::test]
async fn test_tool_audit_flight_recording_for_every_tool() {
    let (server, audit, _tmp) = create_audited_server(true);

    // Call 3 different tools
    let _ = call_tool(&server, "medplum_search_patients", json!({"name": "Lin"})).await;
    let _ = call_tool(
        &server,
        "medplum_get_patient",
        json!({"patient_id": "pat-sj-001"}),
    )
    .await;
    let _ = call_tool(
        &server,
        "medplum_list_conditions",
        json!({"patient_id": "pat-sj-001"}),
    )
    .await;

    let entries = audit.lock().unwrap().get_entries().unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].tool_name, "medplum_search_patients");
    assert_eq!(entries[1].tool_name, "medplum_get_patient");
    assert_eq!(entries[2].tool_name, "medplum_list_conditions");
    for entry in entries {
        assert_eq!(entry.action_status, ActionStatus::Allowed);
        assert!(!entry.signature.is_empty());
    }
}
