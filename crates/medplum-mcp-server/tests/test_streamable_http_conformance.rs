use axum::body::Body;
use axum::http::{Request, StatusCode};
use medplum_mcp_server::mcp::McpServer;
use serde_json::{json, Value};
use tower::ServiceExt;

#[tokio::test]
async fn test_streamable_http_initialization_and_session_id() {
    let client = medplum_mcp_server::client::MedplumClient::new_demo_default();
    let server = McpServer::new(client, None, false);
    let app = server.into_router();

    // 1. Client sends POST to /mcp with initialize request
    let init_req = json!({
        "jsonrpc": "2.0",
        "id": "1",
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "clientInfo": {
                "name": "conformance-client",
                "version": "1.0.0"
            },
            "capabilities": {}
        }
    });

    let req = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .body(Body::from(init_req.to_string()))
        .unwrap();

    let response = app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let session_header = response.headers().get("mcp-session-id");
    assert!(session_header.is_some(), "Expected mcp-session-id header");
    let session_id = session_header.unwrap().to_str().unwrap().to_string();
    assert!(!session_id.is_empty());

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(body_json["jsonrpc"], "2.0");
    assert_eq!(body_json["id"], "1");
    assert_eq!(body_json["result"]["protocolVersion"], "2025-11-25");
    assert_eq!(body_json["result"]["serverInfo"]["name"], "medplum-mcp-rs");

    // 2. Client sends notifications/initialized (notification, no id)
    let notif_req = json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });

    let req = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("Content-Type", "application/json")
        .header("mcp-session-id", &session_id)
        .body(Body::from(notif_req.to_string()))
        .unwrap();

    let response = app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    // 3. Client sends ping
    let ping_req = json!({
        "jsonrpc": "2.0",
        "id": "2",
        "method": "ping"
    });

    let req = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("Content-Type", "application/json")
        .header("mcp-session-id", &session_id)
        .body(Body::from(ping_req.to_string()))
        .unwrap();

    let response = app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(body_json["result"], json!({}));

    // 4. Client sends logging/setLevel
    let log_req = json!({
        "jsonrpc": "2.0",
        "id": "3",
        "method": "logging/setLevel",
        "params": {
            "level": "info"
        }
    });

    let req = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("Content-Type", "application/json")
        .header("mcp-session-id", &session_id)
        .body(Body::from(log_req.to_string()))
        .unwrap();

    let response = app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(body_json["result"], json!({}));

    // 5. Client sends completion/complete
    let comp_req = json!({
        "jsonrpc": "2.0",
        "id": "4",
        "method": "completion/complete",
        "params": {
            "ref": {
                "type": "ref/prompt",
                "name": "test_prompt"
            },
            "argument": {
                "name": "arg1",
                "value": "par"
            }
        }
    });

    let req = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("Content-Type", "application/json")
        .header("mcp-session-id", &session_id)
        .body(Body::from(comp_req.to_string()))
        .unwrap();

    let response = app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert!(body_json["result"]["completion"]["values"].is_array());

    // 6. Client connects to GET /mcp for SSE
    let req = Request::builder()
        .method("GET")
        .uri("/mcp")
        .header("Accept", "text/event-stream")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/event-stream"
    );
    assert!(response.headers().get("mcp-session-id").is_some());

    // 7. Client sends request with evil.example.com Host header -> rejected (403)
    let evil_req = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("Content-Type", "application/json")
        .header("Host", "evil.example.com")
        .body(Body::from(init_req.to_string()))
        .unwrap();

    let response = app.clone().oneshot(evil_req).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
