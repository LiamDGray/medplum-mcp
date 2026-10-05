use std::sync::{Arc, Mutex};

use medplum_mcp_core::audit::{ActionStatus, AuditLogManager};
use medplum_mcp_core::typestate::PhysicianWitness;
use medplum_mcp_server::client::MedplumClient;
use medplum_mcp_server::mcp::McpServer;
use serde_json::{json, Value};
use tempfile::NamedTempFile;

#[tokio::test]
async fn test_elicitation_physician_witness_safety_gate_blocking() {
    let temp_log = NamedTempFile::new().unwrap();
    let audit_mgr = Arc::new(Mutex::new(
        AuditLogManager::new(temp_log.path(), b"test-secret-key-32-bytes-long!").unwrap(),
    ));

    let client = MedplumClient::new_demo_default();
    // Allow writes is true, but physician witness is required for active orders
    let server = McpServer::new(client, Some(audit_mgr.clone()), true);

    // 1. Attempt to issue medication order WITHOUT physician witness -> MUST BE BLOCKED
    let call_without_witness = json!({
        "jsonrpc": "2.0",
        "id": "1",
        "method": "tools/call",
        "params": {
            "name": "medplum_issue_medication_order",
            "arguments": {
                "patient_id": "pat-synthetic-001",
                "medication_code": "860975",
                "medication_display": "Metformin 500mg",
                "dosage": "500mg daily"
            }
        }
    });

    let resp_str = server
        .handle_jsonrpc_message(&call_without_witness.to_string())
        .await
        .expect("Expected response");
    let resp: Value = serde_json::from_str(&resp_str).unwrap();

    // Verify error is returned and draft is not activated autonomously
    assert!(
        resp.get("error").is_some() || resp["result"]["isError"] == true,
        "Autonomous activation without witness must return error: {:?}",
        resp
    );

    // Verify audit log recorded Blocked
    let last_event = audit_mgr
        .lock()
        .unwrap()
        .get_entries()
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(last_event.tool_name, "medplum_issue_medication_order");
    assert_eq!(last_event.action_status, ActionStatus::Blocked);
}

#[tokio::test]
async fn test_elicitation_physician_witness_successful_activation() {
    let temp_log = NamedTempFile::new().unwrap();
    let audit_mgr = Arc::new(Mutex::new(
        AuditLogManager::new(temp_log.path(), b"test-secret-key-32-bytes-long!").unwrap(),
    ));

    let client = MedplumClient::new_demo_default();
    let server = McpServer::new(client, Some(audit_mgr.clone()), true);

    // 2. Issue medication order WITH valid physician witness -> MUST SUCCEED
    let witness = PhysicianWitness::new(
        "Practitioner/dr-house-001",
        "1234567890",
        "sig-hmac-sha256-verified-witness",
    );

    let call_with_witness = json!({
        "jsonrpc": "2.0",
        "id": "2",
        "method": "tools/call",
        "params": {
            "name": "medplum_issue_medication_order",
            "arguments": {
                "patient_id": "pat-synthetic-001",
                "medication_code": "860975",
                "medication_display": "Metformin 500mg",
                "dosage": "500mg daily",
                "witness": {
                    "physician_id": witness.physician_id(),
                    "npi": witness.npi(),
                    "signature_token": witness.signature_token()
                }
            }
        }
    });

    let resp_str = server
        .handle_jsonrpc_message(&call_with_witness.to_string())
        .await
        .expect("Expected response");
    let resp: Value = serde_json::from_str(&resp_str).unwrap();

    assert!(resp.get("error").is_none(), "Unexpected error: {:?}", resp);
    assert_eq!(resp["result"]["isError"], false);
    let text = resp["result"]["content"][0]["text"].as_str().unwrap();
    let resource: Value = serde_json::from_str(text).unwrap();

    assert_eq!(resource["resourceType"], "MedicationRequest");
    assert_eq!(resource["status"], "active");
    assert_eq!(resource["witness"]["npi"], "1234567890");

    // Verify audit log recorded Allowed
    let last_event = audit_mgr
        .lock()
        .unwrap()
        .get_entries()
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(last_event.tool_name, "medplum_issue_medication_order");
    assert_eq!(last_event.action_status, ActionStatus::Allowed);
}

#[tokio::test]
async fn test_conformance_fixture_test_elicitation() {
    let client = MedplumClient::new_demo_default();
    let server = McpServer::new(client, None, true);

    let call_elicitation = json!({
        "jsonrpc": "2.0",
        "id": "3",
        "method": "tools/call",
        "params": {
            "name": "test_elicitation",
            "arguments": {
                "message": "Please confirm clinical sign-off"
            }
        }
    });

    let resp_str = server
        .handle_jsonrpc_message(&call_elicitation.to_string())
        .await
        .expect("Expected response");
    let resp: Value = serde_json::from_str(&resp_str).unwrap();
    assert!(resp.get("error").is_none());
    assert_eq!(resp["result"]["isError"], false);
    assert!(resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Please confirm clinical sign-off"));
}
