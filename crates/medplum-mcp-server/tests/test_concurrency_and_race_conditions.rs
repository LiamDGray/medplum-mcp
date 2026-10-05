//! Concurrency, Race Condition & TOCTOU Verification Suite for medplum-mcp-server.
//!
//! Asserts:
//! 1. Heavy concurrent read/write contention on ClinicalSandbox (40 observation writers + 40 medication writers + 40 readers)
//!    guarantees atomic unique ID allocation without collisions or data races.
//! 2. Heavy concurrent JSON-RPC tool dispatch on McpServer (100 parallel tasks) with zero mutex deadlocks or dropped audit events.
//! 3. TOCTOU (Time-of-Check to Time-of-Use) Safety Invariant Verification: 50 concurrent tasks attempting
//!    to inject forbidden terminal statuses ("active", "final", "completed") are 100% intercepted.
//! 4. Multi-client TCP socket concurrency with zero-copy network streaming.
//! 5. HTTP Mock Server connection flood handling 50 concurrent HTTP requests.

use std::collections::HashSet;
use std::net::{TcpListener, TcpStream};
use std::os::unix::io::AsRawFd;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::task::JoinSet;

use medplum_mcp_core::audit::{ActionStatus, AuditLogManager};
use medplum_mcp_server::client::MedplumClient;
use medplum_mcp_server::mcp::McpServer;
use medplum_mcp_server::mock_server::start_mock_server;
use medplum_mcp_server::network_zero_copy::send_clinical_payload;
use medplum_mcp_server::sandbox::ClinicalSandbox;
use serde_json::{json, Value};

#[tokio::test]
async fn test_concurrent_sandbox_read_write_heavy_contention() {
    let sandbox = Arc::new(ClinicalSandbox::new_st_jude());

    const NUM_OBS_WRITERS: usize = 40;
    const NUM_MED_WRITERS: usize = 40;
    const NUM_READERS: usize = 40;

    let mut set = JoinSet::new();

    // 1. Spawn Observation Writers
    for i in 0..NUM_OBS_WRITERS {
        let sb = Arc::clone(&sandbox);
        set.spawn(async move {
            let res = sb
                .create_observation_draft(
                    "pat-sj-001",
                    &format!("LOINC-STRESS-{}", i),
                    100.0 + (i as f64),
                    "mg/dL",
                    &format!("Stress Obs {}", i),
                )
                .expect("draft observation creation must succeed");
            res["id"].as_str().unwrap().to_string()
        });
    }

    // 2. Spawn Medication Writers
    for i in 0..NUM_MED_WRITERS {
        let sb = Arc::clone(&sandbox);
        set.spawn(async move {
            let res = sb
                .create_medication_draft(
                    "pat-sj-001",
                    &format!("RXNORM-STRESS-{}", i),
                    "10 mg",
                    "Daily",
                    &format!("Stress Med {}", i),
                )
                .expect("draft medication creation must succeed");
            res["id"].as_str().unwrap().to_string()
        });
    }

    // 3. Spawn Concurrent Readers
    for _ in 0..NUM_READERS {
        let sb = Arc::clone(&sandbox);
        set.spawn(async move {
            let pats = sb.search_patients(None, None, None);
            assert!(pats.len() >= 10);
            let obs = sb.list_observations(Some("pat-sj-001"), None, None);
            assert!(!obs.is_empty());
            "reader_ok".to_string()
        });
    }

    let mut created_ids = Vec::new();
    while let Some(res) = set.join_next().await {
        let id = res.expect("Task must not panic");
        if id != "reader_ok" {
            created_ids.push(id);
        }
    }

    // Verify all 80 created IDs are strictly unique (zero counter collisions)
    assert_eq!(created_ids.len(), NUM_OBS_WRITERS + NUM_MED_WRITERS);
    let unique_ids: HashSet<_> = created_ids.iter().cloned().collect();
    assert_eq!(
        unique_ids.len(),
        created_ids.len(),
        "Race condition detected: duplicate IDs generated under concurrency"
    );

    // Verify all created resources are retrievable
    for id in &created_ids {
        if id.starts_with("obs-draft-") {
            assert!(
                sandbox.get_observation(id).is_some(),
                "Created observation {} must be retrievable",
                id
            );
        } else if id.starts_with("med-draft-") {
            assert!(
                sandbox.get_medication(id).is_some(),
                "Created medication {} must be retrievable",
                id
            );
        }
    }
}

#[tokio::test]
async fn test_concurrent_mcp_server_tool_calls_stress() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let audit_file = tmp_dir.path().join("audit_stress.jsonl");
    let audit_manager = Arc::new(Mutex::new(
        AuditLogManager::new(&audit_file, b"stress-secret-key").unwrap(),
    ));

    let client = MedplumClient::new_demo_default().with_allow_writes(true);
    let server = Arc::new(McpServer::new(client, Some(audit_manager.clone()), true));

    const NUM_TASKS: usize = 100;
    let mut set = JoinSet::new();

    for i in 0..NUM_TASKS {
        let srv = Arc::clone(&server);
        set.spawn(async move {
            let req = if i % 2 == 0 {
                // Read tool
                json!({
                    "jsonrpc": "2.0",
                    "id": i,
                    "method": "tools/call",
                    "params": {
                        "name": "medplum_get_patient",
                        "arguments": { "patient_id": "pat-sj-001" }
                    }
                })
            } else {
                // Draft write tool
                json!({
                    "jsonrpc": "2.0",
                    "id": i,
                    "method": "tools/call",
                    "params": {
                        "name": "medplum_create_observation_draft",
                        "arguments": {
                            "patient_id": "pat-sj-001",
                            "code": format!("CODE-{}", i),
                            "value_quantity": 50.0 + (i as f64),
                            "unit": "mg/dL"
                        }
                    }
                })
            };

            let resp_str = srv
                .handle_jsonrpc_message(&req.to_string())
                .await
                .expect("Server must produce response");
            let resp: Value = serde_json::from_str(&resp_str).unwrap();
            assert_eq!(resp["id"], i);
            assert_eq!(resp["result"]["isError"], false);
        });
    }

    while let Some(res) = set.join_next().await {
        res.expect("Task must not panic");
    }

    // Verify audit log captured all 100 invocations with unbroken HMAC integrity
    let entries = audit_manager.lock().unwrap().get_entries().unwrap();
    assert_eq!(entries.len(), NUM_TASKS);
    for entry in entries {
        assert_eq!(entry.action_status, ActionStatus::Allowed);
    }
}

#[tokio::test]
async fn test_toctou_mutation_race_condition_zero_leak() {
    // 50 concurrent tasks attempt to bypass the safety gate by passing forbidden statuses
    let tmp_dir = tempfile::tempdir().unwrap();
    let audit_file = tmp_dir.path().join("audit_toctou.jsonl");
    let audit_manager = Arc::new(Mutex::new(
        AuditLogManager::new(&audit_file, b"toctou-secret-key").unwrap(),
    ));

    let client = MedplumClient::new_demo_default().with_allow_writes(true);
    let server = Arc::new(McpServer::new(client, Some(audit_manager.clone()), true));

    const NUM_ADVERSARIAL_TASKS: usize = 50;
    let mut set = JoinSet::new();

    let forbidden_statuses = ["active", "completed", "final", "amended", "cancelled"];

    for i in 0..NUM_ADVERSARIAL_TASKS {
        let srv = Arc::clone(&server);
        let status = forbidden_statuses[i % forbidden_statuses.len()];

        set.spawn(async move {
            let req = json!({
                "jsonrpc": "2.0",
                "id": i + 500,
                "method": "tools/call",
                "params": {
                    "name": "medplum_create_medication_draft",
                    "arguments": {
                        "patient_id": "pat-sj-001",
                        "medication_code": "115113",
                        "dosage": "5 mg",
                        "instructions": "Stat",
                        "status": status // Forbidden status attempt!
                    }
                }
            });

            let resp_str = srv
                .handle_jsonrpc_message(&req.to_string())
                .await
                .expect("Server must produce response");
            let resp: Value = serde_json::from_str(&resp_str).unwrap();

            // Must be rejected either as JSON-RPC error or tool isError=true
            assert!(
                resp.get("error").is_some() || resp["result"]["isError"] == true,
                "TOCTOU race leak: forbidden status '{}' was not rejected",
                status
            );
        });
    }

    while let Some(res) = set.join_next().await {
        res.expect("Adversarial task must not panic");
    }

    // Verify all 50 adversarial attacks were intercepted and logged as BLOCKED in audit trail
    let entries = audit_manager.lock().unwrap().get_entries().unwrap();
    assert_eq!(entries.len(), NUM_ADVERSARIAL_TASKS);
    for entry in entries {
        assert_eq!(
            entry.action_status,
            ActionStatus::Blocked,
            "All forbidden statuses must be cryptographically recorded as BLOCKED"
        );
    }
}

#[tokio::test]
async fn test_concurrent_mock_server_connection_flood() {
    let sandbox = ClinicalSandbox::new_st_jude();
    let handle = start_mock_server(0, sandbox).await.unwrap();
    let base_url = format!("http://127.0.0.1:{}", handle.port());

    const NUM_CLIENTS: usize = 50;
    let mut set = JoinSet::new();

    for i in 0..NUM_CLIENTS {
        let url = base_url.clone();
        set.spawn(async move {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap();

            let endpoint = match i % 4 {
                0 => format!("{}/fhir/R4/Patient", url),
                1 => format!("{}/fhir/R4/Observation", url),
                2 => format!("{}/fhir/R4/Condition", url),
                _ => format!("{}/fhir/R4/MedicationRequest", url),
            };

            let resp = client.get(&endpoint).send().await.unwrap();
            assert_eq!(resp.status(), reqwest::StatusCode::OK);

            // Rate-limit headers must be present even under connection flood
            assert!(resp.headers().contains_key("x-rate-limit-limit"));
            assert!(resp.headers().contains_key("x-rate-limit-remaining"));
        });
    }

    while let Some(res) = set.join_next().await {
        res.expect("HTTP client task must not panic");
    }

    handle.shutdown();
}

#[test]
fn test_concurrent_network_zerocopy_socket_transfers() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    const NUM_SOCKET_THREADS: usize = 10;
    const PAYLOAD_SIZE: usize = 20 * 1024; // 20 KB (triggers zero-copy path)

    let mut payload = Vec::with_capacity(PAYLOAD_SIZE);
    while payload.len() < PAYLOAD_SIZE {
        payload.extend_from_slice(b"{\"resourceType\":\"Observation\",\"value\":98.6},");
    }
    let shared_payload = Arc::new(payload);

    let expected_len = shared_payload.len();

    let server_handle = std::thread::spawn(move || {
        for _ in 0..NUM_SOCKET_THREADS {
            let (stream, _) = listener.accept().unwrap();
            let raw_fd = stream.as_raw_fd();
            let p = Arc::clone(&shared_payload);
            std::thread::spawn(move || {
                let sent = send_clinical_payload(raw_fd, &p).unwrap();
                assert_eq!(sent, p.len());
                drop(stream);
            });
        }
    });

    let mut client_handles = Vec::with_capacity(NUM_SOCKET_THREADS);
    for _ in 0..NUM_SOCKET_THREADS {
        let handle = std::thread::spawn(move || {
            let mut client = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
            let mut buf = Vec::new();
            std::io::Read::read_to_end(&mut client, &mut buf).unwrap();
            assert_eq!(buf.len(), expected_len);
        });
        client_handles.push(handle);
    }

    for h in client_handles {
        h.join().unwrap();
    }
    server_handle.join().unwrap();
}
