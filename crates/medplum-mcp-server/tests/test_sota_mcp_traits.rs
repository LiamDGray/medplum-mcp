//! State-of-the-Art (October 2026) MCP Server Trait Verification Suite.
//!
//! Tests:
//! 1. RFC 9728 OAuth Protected Resource Metadata (`/.well-known/oauth-protected-resource`)
//! 2. Cooperative Wire Cancellation (`notifications/cancelled`)
//! 3. OpenAPI 3.1 & Overlay 1.0 Specification Export with AI docstrings
//! 4. Context Window Optimization via Category-Filtered Tool Discovery (`tools/list`)

use medplum_mcp_core::audit::AuditLogManager;
use medplum_mcp_server::client::MedplumClient;
use medplum_mcp_server::mcp::McpServer;
use medplum_mcp_server::mock_server::start_mock_server;
use medplum_mcp_server::sandbox::ClinicalSandbox;
use serde_json::json;
use std::sync::{Arc, Mutex};
use tempfile::NamedTempFile;

#[tokio::test]
async fn test_rfc_9728_oauth_protected_resource_metadata() {
    let sandbox = ClinicalSandbox::new_st_jude();
    let server = start_mock_server(0, sandbox)
        .await
        .expect("start mock server");

    let client = reqwest::Client::new();
    let url = format!("{}/.well-known/oauth-protected-resource", server.base_url());
    let resp = client.get(&url).send().await.expect("send request");

    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let meta: serde_json::Value = resp.json().await.expect("parse json");
    assert!(meta.get("resource").is_some());
    assert!(meta.get("authorization_servers").is_some());
    assert!(meta.get("scopes_supported").is_some());
    assert!(meta.get("bearer_methods_supported").is_some());

    let scopes = meta["scopes_supported"].as_array().expect("scopes array");
    assert!(scopes.iter().any(|s| s == "patient/*.read"));
}

#[tokio::test]
async fn test_cooperative_cancellation_notification() {
    let client = MedplumClient::new_demo(ClinicalSandbox::new_st_jude());
    let temp_file = NamedTempFile::new().unwrap();
    let audit_manager = Arc::new(Mutex::new(
        AuditLogManager::new(temp_file.path(), b"test-key").unwrap(),
    ));

    let server = McpServer::new(client, Some(audit_manager.clone()), true);

    // Client sends standard MCP notifications/cancelled frame
    let cancel_msg = json!({
        "jsonrpc": "2.0",
        "method": "notifications/cancelled",
        "params": {
            "requestId": 105,
            "reason": "user_cancelled_long_running_triage"
        }
    })
    .to_string();

    let resp = server.handle_jsonrpc_message(&cancel_msg).await;
    // Notifications must return None (no direct JSON-RPC response body)
    assert!(resp.is_none());

    // Verify cancellation was recorded in audit flight recorder
    let mgr = audit_manager.lock().unwrap();
    let entries = mgr.get_entries().unwrap();
    assert!(!entries.is_empty());
    let last = entries.last().unwrap();
    assert_eq!(last.tool_name, "notifications/cancelled");
}

#[tokio::test]
async fn test_openapi_3_1_and_overlay_export() {
    let client = MedplumClient::new_demo(ClinicalSandbox::new_st_jude());
    let server = McpServer::new(client, None, false);

    // Export without overlay
    let spec = server.export_openapi_spec(false);
    assert_eq!(spec["openapi"], "3.1.0");
    assert!(spec["paths"].get("/tools/get_patient").is_some());
    assert!(spec["paths"]
        .get("/tools/create_medication_draft")
        .is_some());

    // Export with OpenAPI Overlay 1.0 AI-friendly docstrings
    let spec_overlay = server.export_openapi_spec(true);
    let get_patient_desc = spec_overlay["paths"]["/tools/get_patient"]["post"]["description"]
        .as_str()
        .unwrap();
    assert!(get_patient_desc.contains("Use when:"));
}

#[tokio::test]
async fn test_tool_discovery_filtering_prevents_context_bloat() {
    let client = MedplumClient::new_demo(ClinicalSandbox::new_st_jude());
    let server = McpServer::new(client, None, true);

    // 1. Default tools/list returns all 16 tools
    let list_all_req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list",
        "params": {}
    })
    .to_string();

    let resp_all: serde_json::Value =
        serde_json::from_str(&server.handle_jsonrpc_message(&list_all_req).await.unwrap()).unwrap();
    let tools_all = resp_all["result"]["tools"].as_array().unwrap();
    assert_eq!(tools_all.len(), 16);

    // 2. Filter by category "drafts" returns only the 2 mutation tools
    let list_drafts_req = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {
            "category": "drafts"
        }
    })
    .to_string();

    let resp_drafts: serde_json::Value = serde_json::from_str(
        &server
            .handle_jsonrpc_message(&list_drafts_req)
            .await
            .unwrap(),
    )
    .unwrap();
    let tools_drafts = resp_drafts["result"]["tools"].as_array().unwrap();
    assert_eq!(tools_drafts.len(), 2);
    assert!(tools_drafts
        .iter()
        .all(|t| t["name"].as_str().unwrap().contains("draft")));

    // 3. Filter by category "query" returns only the 14 read-only tools
    let list_query_req = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/list",
        "params": {
            "category": "query"
        }
    })
    .to_string();

    let resp_query: serde_json::Value = serde_json::from_str(
        &server
            .handle_jsonrpc_message(&list_query_req)
            .await
            .unwrap(),
    )
    .unwrap();
    let tools_query = resp_query["result"]["tools"].as_array().unwrap();
    assert_eq!(tools_query.len(), 14);
}
