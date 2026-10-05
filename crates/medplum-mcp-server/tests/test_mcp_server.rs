//! Integration tests for High-Performance MCP 2.3+ Server & Streaming Transport.
//!
//! Follows strict TDD verifying JSON-RPC 2.0 protocol handlers, all 16 clinical tools,
//! 4 native FHIR resources, 2 clinical prompts, HMAC-SHA256 flight recorder interceptor,
//! and zero-copy streaming stdio / SSE router.

use axum::body::to_bytes;
use axum::http::Request;
use medplum_mcp_core::audit::{ActionStatus, AuditLogManager};
use medplum_mcp_server::client::MedplumClient;
use medplum_mcp_server::mcp::McpServer;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tokio::io::AsyncWriteExt;
use tower::ServiceExt;

#[tokio::test]
async fn test_mcp_protocol_initialize() {
    let client = MedplumClient::new_demo_default();
    let server = McpServer::new(client, None, false);

    let init_req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test-client", "version": "1.0.0" }
        }
    });

    let resp_str = server
        .handle_jsonrpc_message(&init_req.to_string())
        .await
        .expect("response");
    let resp: Value = serde_json::from_str(&resp_str).expect("valid json response");

    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 1);
    let result = &resp["result"];
    assert_eq!(result["serverInfo"]["name"], "medplum-mcp-rs");
    assert_eq!(result["serverInfo"]["version"], "0.1.0");
    assert!(result["capabilities"]["tools"].is_object());
    assert!(result["capabilities"]["resources"].is_object());
    assert!(result["capabilities"]["prompts"].is_object());
}

#[tokio::test]
async fn test_mcp_tools_list_registers_16_clinical_tools() {
    let client = MedplumClient::new_demo_default();
    let server = McpServer::new(client, None, false);

    let list_req = json!({
        "jsonrpc": "2.0",
        "id": "list-1",
        "method": "tools/list",
        "params": {}
    });

    let resp_str = server
        .handle_jsonrpc_message(&list_req.to_string())
        .await
        .expect("response");
    let resp: Value = serde_json::from_str(&resp_str).expect("valid json");

    let tools = resp["result"]["tools"].as_array().expect("tools array");
    assert_eq!(tools.len(), 16, "Must register exactly 16 clinical tools");

    let expected_tools = [
        "medplum_search_patients",
        "medplum_get_patient",
        "medplum_list_observations",
        "medplum_get_observation",
        "medplum_create_observation_draft",
        "medplum_list_conditions",
        "medplum_get_condition",
        "medplum_list_medication_requests",
        "medplum_get_medication_request",
        "medplum_create_medication_draft",
        "medplum_list_allergy_intolerances",
        "medplum_get_allergy_intolerance",
        "medplum_list_diagnostic_reports",
        "medplum_get_diagnostic_report",
        "medplum_list_encounters",
        "medplum_list_care_plans",
    ];

    let registered_names: Vec<&str> = tools
        .iter()
        .map(|t| t["name"].as_str().expect("tool name"))
        .collect();

    for expected in expected_tools {
        assert!(
            registered_names.contains(&expected),
            "Missing registered tool: {}",
            expected
        );
    }
}

#[tokio::test]
async fn test_mcp_resources_list_registers_4_fhir_resources() {
    let client = MedplumClient::new_demo_default();
    let server = McpServer::new(client, None, false);

    let list_req = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "resources/list",
        "params": {}
    });

    let resp_str = server
        .handle_jsonrpc_message(&list_req.to_string())
        .await
        .expect("response");
    let resp: Value = serde_json::from_str(&resp_str).expect("valid json");

    let resources = resp["result"]["resources"]
        .as_array()
        .expect("resources array");
    assert_eq!(resources.len(), 4, "Must register 4 FHIR resources");

    let uris: Vec<&str> = resources
        .iter()
        .map(|r| r["uri"].as_str().expect("resource uri"))
        .collect();

    assert!(uris.contains(&"fhir://patients/{patient_id}/clinical-summary"));
    assert!(uris.contains(&"fhir://benchmarks"));
    assert!(uris.contains(&"fhir://audit/latest"));
    assert!(uris.contains(&"fhir://verification/status"));
}

#[tokio::test]
async fn test_mcp_prompts_list_registers_2_clinical_prompts() {
    let client = MedplumClient::new_demo_default();
    let server = McpServer::new(client, None, false);

    let list_req = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "prompts/list",
        "params": {}
    });

    let resp_str = server
        .handle_jsonrpc_message(&list_req.to_string())
        .await
        .expect("response");
    let resp: Value = serde_json::from_str(&resp_str).expect("valid json");

    let prompts = resp["result"]["prompts"].as_array().expect("prompts array");
    assert_eq!(prompts.len(), 2, "Must register 2 clinical prompts");

    let names: Vec<&str> = prompts
        .iter()
        .map(|p| p["name"].as_str().expect("prompt name"))
        .collect();

    assert!(names.contains(&"clinical_encounter_triage"));
    assert!(names.contains(&"drug_interaction_audit"));
}

#[tokio::test]
async fn test_invoking_read_tools_with_audit_trail() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let audit_file = tmp_dir.path().join("audit.jsonl");
    let audit_manager = Arc::new(Mutex::new(
        AuditLogManager::new(&audit_file, b"secret-test-key").unwrap(),
    ));

    let client = MedplumClient::new_demo_default();
    let server = McpServer::new(client, Some(audit_manager.clone()), false);

    // 1. Search patients
    let search_req = json!({
        "jsonrpc": "2.0",
        "id": 10,
        "method": "tools/call",
        "params": {
            "name": "medplum_search_patients",
            "arguments": {
                "name": "Lin"
            }
        }
    });

    let search_resp = server
        .handle_jsonrpc_message(&search_req.to_string())
        .await
        .unwrap();
    let val: Value = serde_json::from_str(&search_resp).unwrap();
    assert_eq!(val["id"], 10);
    assert_eq!(val["result"]["isError"], false);
    let text = val["result"]["content"][0]["text"].as_str().unwrap();
    let bundle: Value = serde_json::from_str(text).unwrap();
    assert_eq!(bundle["resourceType"], "Bundle");
    let pat_id = bundle["entry"][0]["resource"]["id"].as_str().unwrap();

    // 2. Get patient
    let get_req = json!({
        "jsonrpc": "2.0",
        "id": 11,
        "method": "tools/call",
        "params": {
            "name": "medplum_get_patient",
            "arguments": {
                "patient_id": pat_id
            }
        }
    });
    let get_resp = server
        .handle_jsonrpc_message(&get_req.to_string())
        .await
        .unwrap();
    let val2: Value = serde_json::from_str(&get_resp).unwrap();
    let pat_text = val2["result"]["content"][0]["text"].as_str().unwrap();
    let pat: Value = serde_json::from_str(pat_text).unwrap();
    assert_eq!(pat["id"], pat_id);

    // 3. List observations
    let obs_req = json!({
        "jsonrpc": "2.0",
        "id": 12,
        "method": "tools/call",
        "params": {
            "name": "medplum_list_observations",
            "arguments": {
                "patient_id": pat_id,
                "detail_level": "standard"
            }
        }
    });
    let obs_resp = server
        .handle_jsonrpc_message(&obs_req.to_string())
        .await
        .unwrap();
    let val3: Value = serde_json::from_str(&obs_resp).unwrap();
    let obs_bundle: Value =
        serde_json::from_str(val3["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(obs_bundle["resourceType"], "Bundle");

    // Verify audit log has 3 entries with ActionStatus::ALLOWED
    let entries = audit_manager.lock().unwrap().get_entries().unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].tool_name, "medplum_search_patients");
    assert_eq!(entries[0].action_status, ActionStatus::Allowed);
    assert_eq!(entries[1].tool_name, "medplum_get_patient");
    assert_eq!(entries[1].action_status, ActionStatus::Allowed);
    assert_eq!(entries[2].tool_name, "medplum_list_observations");
    assert_eq!(entries[2].action_status, ActionStatus::Allowed);
}

#[tokio::test]
async fn test_write_tools_mutation_permissions_and_forbidden_status() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let audit_file = tmp_dir.path().join("audit.jsonl");
    let audit_manager = Arc::new(Mutex::new(
        AuditLogManager::new(&audit_file, b"secret-test-key").unwrap(),
    ));

    // 1. allow_writes = true -> succeeds
    let client_rw = MedplumClient::new_demo_default().with_allow_writes(true);
    let server_rw = McpServer::new(client_rw, Some(audit_manager.clone()), true);

    let create_req = json!({
        "jsonrpc": "2.0",
        "id": 20,
        "method": "tools/call",
        "params": {
            "name": "medplum_create_observation_draft",
            "arguments": {
                "patient_id": "pat-sj-001",
                "code": "26499-4",
                "value_quantity": 1500.0,
                "unit": "/uL",
                "display": "Absolute Neutrophil Count",
                "status": "registered"
            }
        }
    });
    let resp_str = server_rw
        .handle_jsonrpc_message(&create_req.to_string())
        .await
        .unwrap();
    let resp: Value = serde_json::from_str(&resp_str).unwrap();
    assert_eq!(resp["result"]["isError"], false);
    let obs: Value =
        serde_json::from_str(resp["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(obs["status"], "registered");

    let entries = audit_manager.lock().unwrap().get_entries().unwrap();
    assert_eq!(entries.last().unwrap().action_status, ActionStatus::Allowed);

    // 2. allow_writes = false -> fails and logs BLOCKED
    let client_ro = MedplumClient::new_demo_default().with_allow_writes(false);
    let server_ro = McpServer::new(client_ro, Some(audit_manager.clone()), false);

    let block_req = json!({
        "jsonrpc": "2.0",
        "id": 21,
        "method": "tools/call",
        "params": {
            "name": "medplum_create_observation_draft",
            "arguments": {
                "patient_id": "pat-sj-001",
                "code": "26499-4",
                "value_quantity": 1500.0,
                "unit": "/uL"
            }
        }
    });
    let resp_block_str = server_ro
        .handle_jsonrpc_message(&block_req.to_string())
        .await
        .unwrap();
    let resp_block: Value = serde_json::from_str(&resp_block_str).unwrap();
    // Either isError=true or jsonrpc error
    assert!(
        resp_block.get("error").is_some() || resp_block["result"]["isError"] == true,
        "Write should be rejected when allow_writes=false"
    );

    let entries2 = audit_manager.lock().unwrap().get_entries().unwrap();
    assert_eq!(
        entries2.last().unwrap().action_status,
        ActionStatus::Blocked
    );
    assert_eq!(
        entries2.last().unwrap().tool_name,
        "medplum_create_observation_draft"
    );

    // 3. Attempt forbidden status -> fails and logs BLOCKED
    let forbidden_req = json!({
        "jsonrpc": "2.0",
        "id": 22,
        "method": "tools/call",
        "params": {
            "name": "medplum_create_observation_draft",
            "arguments": {
                "patient_id": "pat-sj-001",
                "code": "26499-4",
                "value_quantity": 1500.0,
                "unit": "/uL",
                "status": "final" // Forbidden!
            }
        }
    });
    let resp_forbid_str = server_rw
        .handle_jsonrpc_message(&forbidden_req.to_string())
        .await
        .unwrap();
    let resp_forbid: Value = serde_json::from_str(&resp_forbid_str).unwrap();
    assert!(
        resp_forbid.get("error").is_some() || resp_forbid["result"]["isError"] == true,
        "Forbidden status 'final' must be rejected"
    );

    let entries3 = audit_manager.lock().unwrap().get_entries().unwrap();
    assert_eq!(
        entries3.last().unwrap().action_status,
        ActionStatus::Blocked
    );
}

#[tokio::test]
async fn test_native_fhir_resources_read() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let audit_file = tmp_dir.path().join("audit.jsonl");
    let audit_manager = Arc::new(Mutex::new(
        AuditLogManager::new(&audit_file, b"secret-test-key").unwrap(),
    ));

    let client = MedplumClient::new_demo_default();
    let server = McpServer::new(client, Some(audit_manager.clone()), false);

    // Call one tool to populate audit
    let get_req = json!({
        "jsonrpc": "2.0",
        "id": 30,
        "method": "tools/call",
        "params": {
            "name": "medplum_get_patient",
            "arguments": { "patient_id": "pat-sj-001" }
        }
    });
    let _ = server
        .handle_jsonrpc_message(&get_req.to_string())
        .await
        .unwrap();

    // 1. Read fhir://benchmarks
    let bench_req = json!({
        "jsonrpc": "2.0",
        "id": 31,
        "method": "resources/read",
        "params": { "uri": "fhir://benchmarks" }
    });
    let bench_resp = server
        .handle_jsonrpc_message(&bench_req.to_string())
        .await
        .unwrap();
    let val: Value = serde_json::from_str(&bench_resp).unwrap();
    let bench_text = val["result"]["contents"][0]["text"].as_str().unwrap();
    let bench_json: Value = serde_json::from_str(bench_text).unwrap();
    assert!(bench_json["overall_reduction_pct"].as_f64().unwrap() >= 80.0);

    // 2. Read fhir://verification/status
    let verif_req = json!({
        "jsonrpc": "2.0",
        "id": 32,
        "method": "resources/read",
        "params": { "uri": "fhir://verification/status" }
    });
    let verif_resp = server
        .handle_jsonrpc_message(&verif_req.to_string())
        .await
        .unwrap();
    let val2: Value = serde_json::from_str(&verif_resp).unwrap();
    let verif_text = val2["result"]["contents"][0]["text"].as_str().unwrap();
    let verif_json: Value = serde_json::from_str(verif_text).unwrap();
    assert_eq!(verif_json["overall_status"], "PROVEN");

    // 3. Read fhir://audit/latest
    let audit_req = json!({
        "jsonrpc": "2.0",
        "id": 33,
        "method": "resources/read",
        "params": { "uri": "fhir://audit/latest" }
    });
    let audit_resp = server
        .handle_jsonrpc_message(&audit_req.to_string())
        .await
        .unwrap();
    let val3: Value = serde_json::from_str(&audit_resp).unwrap();
    let audit_text = val3["result"]["contents"][0]["text"].as_str().unwrap();
    let audit_entries: Value = serde_json::from_str(audit_text).unwrap();
    assert!(!audit_entries.as_array().unwrap().is_empty());

    // 4. Read fhir://patients/pat-sj-001/clinical-summary
    let summary_req = json!({
        "jsonrpc": "2.0",
        "id": 34,
        "method": "resources/read",
        "params": { "uri": "fhir://patients/pat-sj-001/clinical-summary" }
    });
    let summary_resp = server
        .handle_jsonrpc_message(&summary_req.to_string())
        .await
        .unwrap();
    let val4: Value = serde_json::from_str(&summary_resp).unwrap();
    let summary_text = val4["result"]["contents"][0]["text"].as_str().unwrap();
    let summary_json: Value = serde_json::from_str(summary_text).unwrap();
    assert_eq!(summary_json["patient_id"], "pat-sj-001");
    assert!(summary_json.get("conditions").is_some());
    assert!(summary_json.get("medications").is_some());
}

#[tokio::test]
async fn test_native_prompts_get() {
    let client = MedplumClient::new_demo_default();
    let server = McpServer::new(client, None, false);

    // 1. clinical_encounter_triage
    let prompt1_req = json!({
        "jsonrpc": "2.0",
        "id": 40,
        "method": "prompts/get",
        "params": {
            "name": "clinical_encounter_triage",
            "arguments": {
                "patient_id": "pat-sj-001",
                "encounter_id": "enc-sj-001"
            }
        }
    });
    let p1_resp = server
        .handle_jsonrpc_message(&prompt1_req.to_string())
        .await
        .unwrap();
    let val1: Value = serde_json::from_str(&p1_resp).unwrap();
    let msg1 = val1["result"]["messages"][0]["content"]["text"]
        .as_str()
        .unwrap();
    assert!(msg1.contains("pat-sj-001"));
    assert!(msg1.contains("enc-sj-001"));
    assert!(msg1.to_lowercase().contains("witness"));

    // 2. drug_interaction_audit
    let prompt2_req = json!({
        "jsonrpc": "2.0",
        "id": 41,
        "method": "prompts/get",
        "params": {
            "name": "drug_interaction_audit",
            "arguments": {
                "patient_id": "pat-sj-001",
                "proposed_medication": "Methotrexate"
            }
        }
    });
    let p2_resp = server
        .handle_jsonrpc_message(&prompt2_req.to_string())
        .await
        .unwrap();
    let val2: Value = serde_json::from_str(&p2_resp).unwrap();
    let msg2 = val2["result"]["messages"][0]["content"]["text"]
        .as_str()
        .unwrap();
    assert!(msg2.contains("pat-sj-001"));
    assert!(msg2.contains("Methotrexate"));
    assert!(msg2.to_lowercase().contains("interaction"));
}

#[tokio::test]
async fn test_stdio_streaming_transport() {
    let client = MedplumClient::new_demo_default();
    let server = McpServer::new(client, None, false);

    // Prepare line-delimited input
    let req1 = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    });
    let req2 = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    });
    let input_bytes = format!("{}\n{}\n", req1, req2);

    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let (client_read, mut client_write) = tokio::io::split(client_io);
    let (server_read, server_write) = tokio::io::split(server_io);

    // Spawn server stdio loop
    let server_handle =
        tokio::spawn(async move { server.run_stdio(server_read, server_write).await });

    // Write requests from client side
    client_write
        .write_all(input_bytes.as_bytes())
        .await
        .unwrap();
    client_write.shutdown().await.unwrap();
    drop(client_write);

    // Read responses from client side
    use tokio::io::AsyncBufReadExt;
    let mut reader = tokio::io::BufReader::new(client_read);
    let mut line1 = String::new();
    reader.read_line(&mut line1).await.unwrap();
    assert!(
        !line1.is_empty(),
        "First line should be initialize response"
    );
    let resp1: Value = serde_json::from_str(&line1).unwrap();
    assert_eq!(resp1["id"], 1);
    assert_eq!(resp1["result"]["serverInfo"]["name"], "medplum-mcp-rs");

    let mut line2 = String::new();
    reader.read_line(&mut line2).await.unwrap();
    assert!(
        !line2.is_empty(),
        "Second line should be tools/list response"
    );
    let resp2: Value = serde_json::from_str(&line2).unwrap();
    assert_eq!(resp2["id"], 2);
    assert_eq!(resp2["result"]["tools"].as_array().unwrap().len(), 16);

    let _ = server_handle.await;
}

#[tokio::test]
async fn test_sse_http_transport() {
    let client = MedplumClient::new_demo_default();
    let server = McpServer::new(client, None, false);
    let app = server.into_router();

    // 1. POST /message with JSON-RPC initialize
    let init_req = json!({
        "jsonrpc": "2.0",
        "id": 100,
        "method": "initialize",
        "params": {}
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/message")
                .header("Content-Type", "application/json")
                .body(axum::body::Body::from(init_req.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let resp: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(resp["id"], 100);
    assert_eq!(resp["result"]["serverInfo"]["name"], "medplum-mcp-rs");

    // 2. GET /sse establishes SSE stream
    let sse_resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/sse")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(sse_resp.status(), axum::http::StatusCode::OK);
    let content_type = sse_resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(content_type.contains("text/event-stream"));
}
