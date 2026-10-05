use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use modbus_mcp_core::distillation::calculate_token_reduction;
use modbus_mcp_core::safety::OperatorWitness;
use modbus_mcp_server::mcp::ModbusMcpServer;
use modbus_mcp_server::simulator::VirtualPlcSimulator;
use serde_json::json;
use tower::ServiceExt;

const SECRET: &[u8] = b"industrial_iot_modbus_hmac_secret_key_2026";

fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[tokio::test]
async fn test_virtual_plc_simulator_reactor_dynamics() {
    let mut plc = VirtualPlcSimulator::new(SECRET);

    // Initial state check
    assert_eq!(
        plc.read_coils(0, 5).unwrap(),
        vec![false, false, false, false, false]
    );
    assert_eq!(
        plc.read_discrete_inputs(0, 4).unwrap(),
        vec![false, false, false, false]
    );

    // Initial temperature and pressure in input registers
    let initial_inputs = plc.read_input_registers(0, 4).unwrap();
    assert_eq!(initial_inputs.len(), 4);
    let initial_temp = initial_inputs[0];
    assert!(initial_temp > 0, "Initial temperature should be positive");

    // Actuate pump 1 and heater using valid witnesses
    let now = current_time_ms();
    let witness_pump = OperatorWitness::issue("sup_01", "badge_42", 0, 1, 60_000, now, SECRET);
    plc.write_coil_interlocked(0, true, Some(&witness_pump), now)
        .unwrap();

    let witness_heater = OperatorWitness::issue("sup_01", "badge_42", 3, 1, 60_000, now, SECRET);
    plc.write_coil_interlocked(3, true, Some(&witness_heater), now)
        .unwrap();

    assert!(plc.read_coils(0, 1).unwrap()[0]);
    assert!(plc.read_coils(3, 1).unwrap()[0]);

    // Advance simulator dynamics
    for _ in 0..5 {
        plc.step();
    }

    // Discrete inputs: pump 1 running feedback should now be true
    let discrete = plc.read_discrete_inputs(0, 4).unwrap();
    assert!(
        discrete[0],
        "Pump 1 running discrete input should be active"
    );

    // Input registers: dynamic flow and temperature should have increased
    let dynamic_inputs = plc.read_input_registers(0, 4).unwrap();
    assert!(
        dynamic_inputs[0] >= initial_temp,
        "Reactor temp should increase or stay warm with heater on"
    );
    assert!(
        dynamic_inputs[3] > 0,
        "Flow rate should be positive when pump 1 runs"
    );
}

#[tokio::test]
async fn test_mcp_tool_read_discrete_inputs_and_coils() {
    let plc = Arc::new(tokio::sync::Mutex::new(VirtualPlcSimulator::new(SECRET)));
    let server = ModbusMcpServer::new(plc.clone(), SECRET.to_vec());

    // 1. Read Discrete Inputs
    let req_di = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "modbus_read_discrete_inputs",
            "arguments": {
                "address": 0,
                "count": 4
            }
        }
    });

    let resp_di_str = server
        .handle_jsonrpc_message(&req_di.to_string())
        .await
        .expect("response");
    let resp_di: serde_json::Value = serde_json::from_str(&resp_di_str).unwrap();
    assert!(resp_di["error"].is_null());
    let text = resp_di["result"]["content"][0]["text"].as_str().unwrap();
    let values: Vec<bool> = serde_json::from_str(text).unwrap();
    assert_eq!(values.len(), 4);

    // 2. Read Coils
    let req_coils = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "modbus_read_coils",
            "arguments": {
                "address": 0,
                "count": 5
            }
        }
    });

    let resp_coils_str = server
        .handle_jsonrpc_message(&req_coils.to_string())
        .await
        .expect("response");
    let resp_coils: serde_json::Value = serde_json::from_str(&resp_coils_str).unwrap();
    assert!(resp_coils["error"].is_null());
    let text_c = resp_coils["result"]["content"][0]["text"].as_str().unwrap();
    let coils: Vec<bool> = serde_json::from_str(text_c).unwrap();
    assert_eq!(coils.len(), 5);
}

#[tokio::test]
async fn test_mcp_tool_read_input_registers_token_diet() {
    let plc = Arc::new(tokio::sync::Mutex::new(VirtualPlcSimulator::new(SECRET)));
    let server = ModbusMcpServer::new(plc.clone(), SECRET.to_vec());

    // 1. Raw detail level
    let req_raw = json!({
        "jsonrpc": "2.0",
        "id": 10,
        "method": "tools/call",
        "params": {
            "name": "modbus_read_input_registers",
            "arguments": {
                "address": 0,
                "count": 4,
                "detail_level": "raw"
            }
        }
    });
    let resp_raw_str = server
        .handle_jsonrpc_message(&req_raw.to_string())
        .await
        .expect("response");
    let resp_raw: serde_json::Value = serde_json::from_str(&resp_raw_str).unwrap();
    let text_raw = resp_raw["result"]["content"][0]["text"].as_str().unwrap();

    // 2. Standard detail level
    let req_std = json!({
        "jsonrpc": "2.0",
        "id": 11,
        "method": "tools/call",
        "params": {
            "name": "modbus_read_input_registers",
            "arguments": {
                "address": 0,
                "count": 4,
                "detail_level": "standard"
            }
        }
    });
    let resp_std_str = server
        .handle_jsonrpc_message(&req_std.to_string())
        .await
        .expect("response");
    let resp_std: serde_json::Value = serde_json::from_str(&resp_std_str).unwrap();
    let text_std = resp_std["result"]["content"][0]["text"].as_str().unwrap();

    // 3. Compact detail level
    let req_compact = json!({
        "jsonrpc": "2.0",
        "id": 12,
        "method": "tools/call",
        "params": {
            "name": "modbus_read_input_registers",
            "arguments": {
                "address": 0,
                "count": 4,
                "detail_level": "compact"
            }
        }
    });
    let resp_compact_str = server
        .handle_jsonrpc_message(&req_compact.to_string())
        .await
        .expect("response");
    let resp_compact: serde_json::Value = serde_json::from_str(&resp_compact_str).unwrap();
    let text_compact = resp_compact["result"]["content"][0]["text"]
        .as_str()
        .unwrap();

    // Verify token reduction between raw and compact
    let reduction = calculate_token_reduction(text_raw, text_compact);
    assert!(
        reduction >= 50.0,
        "Compact format must reduce tokens by at least 50% vs Raw, got {:.2}%",
        reduction
    );
    assert!(
        text_compact.contains('|'),
        "Compact format must be pipe-delimited"
    );
    assert!(
        text_raw.contains("measurements"),
        "Raw format must contain measurement details"
    );
    assert!(
        text_std.contains('{') && text_std.contains('}'),
        "Standard format must be JSON"
    );
}

#[tokio::test]
async fn test_mcp_tool_read_holding_registers() {
    let plc = Arc::new(tokio::sync::Mutex::new(VirtualPlcSimulator::new(SECRET)));
    let server = ModbusMcpServer::new(plc.clone(), SECRET.to_vec());

    let req = json!({
        "jsonrpc": "2.0",
        "id": 20,
        "method": "tools/call",
        "params": {
            "name": "modbus_read_holding_registers",
            "arguments": {
                "address": 0,
                "count": 3
            }
        }
    });

    let resp_str = server
        .handle_jsonrpc_message(&req.to_string())
        .await
        .expect("response");
    let resp: serde_json::Value = serde_json::from_str(&resp_str).unwrap();
    assert!(resp["error"].is_null());
    let text = resp["result"]["content"][0]["text"].as_str().unwrap();
    let values: Vec<u16> = serde_json::from_str(text).unwrap();
    assert_eq!(values.len(), 3);
}

#[tokio::test]
async fn test_mcp_tool_write_coil_interlocked_unauthorized_blocked_and_audited() {
    let plc = Arc::new(tokio::sync::Mutex::new(VirtualPlcSimulator::new(SECRET)));
    let server = ModbusMcpServer::new(plc.clone(), SECRET.to_vec());

    // Attempt write WITHOUT OperatorWitness
    let req_unauth = json!({
        "jsonrpc": "2.0",
        "id": 30,
        "method": "tools/call",
        "params": {
            "name": "modbus_write_coil_interlocked",
            "arguments": {
                "address": 0,
                "value": true
            }
        }
    });

    let resp_unauth_str = server
        .handle_jsonrpc_message(&req_unauth.to_string())
        .await
        .expect("response");
    let resp_unauth: serde_json::Value = serde_json::from_str(&resp_unauth_str).unwrap();

    // Verify rejection
    assert_eq!(resp_unauth["result"]["isError"], true);
    let msg = resp_unauth["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(msg.to_lowercase().contains("witness") || msg.to_lowercase().contains("interlock"));

    // Verify coil state did not change
    let locked_plc = plc.lock().await;
    assert!(!locked_plc.read_coils(0, 1).unwrap()[0]);

    // Verify audit flight recorder captured blocked write
    let recorder = locked_plc.flight_recorder();
    let rec = recorder.lock().unwrap();
    assert!(rec.sequence_id() >= 1);
    rec.verify_chain()
        .expect("Audit chain should remain cryptographically intact");
}

#[tokio::test]
async fn test_mcp_tool_write_coil_interlocked_authorized_flips_state() {
    let plc = Arc::new(tokio::sync::Mutex::new(VirtualPlcSimulator::new(SECRET)));
    let server = ModbusMcpServer::new(plc.clone(), SECRET.to_vec());

    let now = current_time_ms();
    let witness =
        OperatorWitness::issue("supervisor_alpha", "badge_7788", 0, 1, 30_000, now, SECRET);

    let req_auth = json!({
        "jsonrpc": "2.0",
        "id": 31,
        "method": "tools/call",
        "params": {
            "name": "modbus_write_coil_interlocked",
            "arguments": {
                "address": 0,
                "value": true,
                "witness": witness
            }
        }
    });

    let resp_auth_str = server
        .handle_jsonrpc_message(&req_auth.to_string())
        .await
        .expect("response");
    let resp_auth: serde_json::Value = serde_json::from_str(&resp_auth_str).unwrap();

    // Verify success
    assert_ne!(resp_auth["result"]["isError"], true);

    // Verify coil flipped
    let locked_plc = plc.lock().await;
    assert!(locked_plc.read_coils(0, 1).unwrap()[0]);

    // Verify audit trail logged execution
    let recorder = locked_plc.flight_recorder();
    let rec = recorder.lock().unwrap();
    assert!(rec.sequence_id() >= 1);
    rec.verify_chain().expect("Audit chain valid");
}

#[tokio::test]
async fn test_mcp_tool_write_holding_register_interlocked() {
    let plc = Arc::new(tokio::sync::Mutex::new(VirtualPlcSimulator::new(SECRET)));
    let server = ModbusMcpServer::new(plc.clone(), SECRET.to_vec());

    // 1. Unauthorized attempt without witness should fail
    let req_unauth = json!({
        "jsonrpc": "2.0",
        "id": 40,
        "method": "tools/call",
        "params": {
            "name": "modbus_write_holding_register_interlocked",
            "arguments": {
                "address": 0,
                "value": 850
            }
        }
    });
    let resp_unauth_str = server
        .handle_jsonrpc_message(&req_unauth.to_string())
        .await
        .expect("response");
    let resp_unauth: serde_json::Value = serde_json::from_str(&resp_unauth_str).unwrap();
    assert_eq!(resp_unauth["result"]["isError"], true);

    // 2. Authorized attempt with valid witness
    let now = current_time_ms();
    let witness =
        OperatorWitness::issue("supervisor_beta", "badge_9900", 0, 850, 30_000, now, SECRET);

    let req_auth = json!({
        "jsonrpc": "2.0",
        "id": 41,
        "method": "tools/call",
        "params": {
            "name": "modbus_write_holding_register_interlocked",
            "arguments": {
                "address": 0,
                "value": 850,
                "witness": witness
            }
        }
    });

    let resp_auth_str = server
        .handle_jsonrpc_message(&req_auth.to_string())
        .await
        .expect("response");
    let resp_auth: serde_json::Value = serde_json::from_str(&resp_auth_str).unwrap();
    assert_ne!(resp_auth["result"]["isError"], true);

    // Verify holding register modified to 850
    let locked_plc = plc.lock().await;
    assert_eq!(locked_plc.read_holding_registers(0, 1).unwrap()[0], 850);
}

#[tokio::test]
async fn test_mcp_tool_emergency_stop() {
    let plc = Arc::new(tokio::sync::Mutex::new(VirtualPlcSimulator::new(SECRET)));
    let server = ModbusMcpServer::new(plc.clone(), SECRET.to_vec());

    // Start Pump 1 and Pump 2
    let now = current_time_ms();
    let w1 = OperatorWitness::issue("sup", "b1", 0, 1, 60_000, now, SECRET);
    let w2 = OperatorWitness::issue("sup", "b2", 1, 1, 60_000, now, SECRET);

    {
        let mut locked = plc.lock().await;
        locked
            .write_coil_interlocked(0, true, Some(&w1), now)
            .unwrap();
        locked
            .write_coil_interlocked(1, true, Some(&w2), now)
            .unwrap();
        assert!(locked.read_coils(0, 1).unwrap()[0]);
        assert!(locked.read_coils(1, 1).unwrap()[0]);
    }

    // Call modbus_emergency_stop
    let req_estop = json!({
        "jsonrpc": "2.0",
        "id": 50,
        "method": "tools/call",
        "params": {
            "name": "modbus_emergency_stop",
            "arguments": {
                "reason": "Test high temperature hazard"
            }
        }
    });

    let resp_str = server
        .handle_jsonrpc_message(&req_estop.to_string())
        .await
        .expect("response");
    let resp: serde_json::Value = serde_json::from_str(&resp_str).unwrap();
    assert_ne!(resp["result"]["isError"], true);

    // Verify safety coil is tripped (coil 4) and pumps (coils 0, 1) are halted (false)
    let locked = plc.lock().await;
    let coils = locked.read_coils(0, 5).unwrap();
    assert!(!coils[0], "Pump 1 must be halted");
    assert!(!coils[1], "Pump 2 must be halted");
    assert!(coils[4], "Safety coil must be tripped");

    // Flight recorder should contain emergency stop event
    let rec = locked.flight_recorder();
    let r = rec.lock().unwrap();
    r.verify_chain().expect("Audit chain intact");
}

#[tokio::test]
async fn test_streamable_http_transport_session_and_dns_rebinding() {
    let plc = Arc::new(tokio::sync::Mutex::new(VirtualPlcSimulator::new(SECRET)));
    let server = ModbusMcpServer::new(plc, SECRET.to_vec());
    let router = server.into_router();

    // 1. DNS rebinding attack blocked
    let req_rebind = Request::builder()
        .uri("/mcp")
        .method("POST")
        .header("host", "attacker-controlled-domain.com")
        .body(Body::from(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        ))
        .unwrap();

    let response_rebind = router.clone().oneshot(req_rebind).await.unwrap();
    assert_eq!(response_rebind.status(), StatusCode::FORBIDDEN);

    // 2. Localhost POST with generated session-id
    let req_valid = Request::builder()
        .uri("/mcp")
        .method("POST")
        .header("host", "localhost:8080")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        ))
        .unwrap();

    let response_valid = router.clone().oneshot(req_valid).await.unwrap();
    assert_eq!(response_valid.status(), StatusCode::OK);
    let session_id_hdr = response_valid.headers().get("mcp-session-id");
    assert!(session_id_hdr.is_some(), "Must return mcp-session-id");
    let session_id_val = session_id_hdr.unwrap().to_str().unwrap();
    assert!(!session_id_val.is_empty());

    let body_bytes = to_bytes(response_valid.into_body(), usize::MAX)
        .await
        .unwrap();
    let resp_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(resp_json["jsonrpc"], "2.0");
    assert_eq!(resp_json["id"], 1);
    assert_eq!(
        resp_json["result"]["serverInfo"]["name"],
        "modbus-mcp-server"
    );

    // 3. Localhost POST with client negotiated session-id
    let custom_session = "custom-client-session-uuid-9999";
    let req_negotiated = Request::builder()
        .uri("/mcp")
        .method("POST")
        .header("host", "127.0.0.1:8080")
        .header("mcp-session-id", custom_session)
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
        ))
        .unwrap();

    let response_negotiated = router.oneshot(req_negotiated).await.unwrap();
    assert_eq!(response_negotiated.status(), StatusCode::OK);
    assert_eq!(
        response_negotiated
            .headers()
            .get("mcp-session-id")
            .unwrap()
            .to_str()
            .unwrap(),
        custom_session
    );
    let body_neg_bytes = to_bytes(response_negotiated.into_body(), usize::MAX)
        .await
        .unwrap();
    let resp_tools: serde_json::Value = serde_json::from_slice(&body_neg_bytes).unwrap();
    let tools = resp_tools["result"]["tools"]
        .as_array()
        .expect("tools array");
    assert!(tools.len() >= 7, "Must expose all 7 required Modbus tools");
}
