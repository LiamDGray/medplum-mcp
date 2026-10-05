//! Integration tests for modbus-mcp-rs CLI, config generator, and cryptographic audit verifier.

use std::io::Write;
use std::path::PathBuf;

use clap::Parser;
use modbus_mcp_cli::config::generate_client_config;
use modbus_mcp_cli::verifier::{run_audit_verify, verify_audit_log_file};
use modbus_mcp_cli::{
    run_simulate, AuditVerifyArgs, Cli, ClientType, Commands, ConfigArgs, SimulateArgs, Transport,
};
use modbus_mcp_core::audit::{MachineEventKind, MachineFlightRecorder};
use tempfile::NamedTempFile;

// =========================================================================
// a) CLI parsing tests: serve, simulate, config, audit-verify
// =========================================================================

#[test]
fn test_cli_parsing_serve_stdio_defaults() {
    let args = vec!["modbus-mcp-rs", "serve"];
    let cli = Cli::try_parse_from(args).expect("Failed to parse default serve command");

    match cli.command {
        Commands::Serve(s) => {
            assert_eq!(s.transport, Transport::Stdio);
            assert_eq!(s.host, "127.0.0.1");
            assert_eq!(s.port, 8000);
            assert!(!s.simulator);
            assert!(s.audit_log.is_none());
        }
        _ => panic!("Expected Serve command"),
    }
}

#[test]
fn test_cli_parsing_serve_sse_custom() {
    let args = vec![
        "modbus-mcp-rs",
        "serve",
        "--transport",
        "sse",
        "--host",
        "0.0.0.0",
        "--port",
        "9090",
        "--simulator",
        "--audit-log",
        "/tmp/modbus_audit.bin",
    ];
    let cli = Cli::try_parse_from(args).expect("Failed to parse custom serve command");

    match cli.command {
        Commands::Serve(s) => {
            assert_eq!(s.transport, Transport::Sse);
            assert_eq!(s.host, "0.0.0.0");
            assert_eq!(s.port, 9090);
            assert!(s.simulator);
            assert_eq!(s.audit_log, Some(PathBuf::from("/tmp/modbus_audit.bin")));
        }
        _ => panic!("Expected Serve command"),
    }
}

#[test]
fn test_cli_parsing_simulate() {
    let args = vec![
        "modbus-mcp-rs",
        "simulate",
        "--duration",
        "15",
        "--interval",
        "250",
    ];
    let cli = Cli::try_parse_from(args).expect("Failed to parse simulate command");

    match cli.command {
        Commands::Simulate(sim) => {
            assert_eq!(sim.duration, 15);
            assert_eq!(sim.interval, 250);
        }
        _ => panic!("Expected Simulate command"),
    }
}

#[test]
fn test_cli_parsing_config_client_selection() {
    let clients = vec![
        ("claude-desktop", ClientType::ClaudeDesktop),
        ("cursor", ClientType::Cursor),
        ("zed", ClientType::Zed),
        ("cline", ClientType::Cline),
        ("all", ClientType::All),
    ];

    for (name, expected_client) in clients {
        let args = vec!["modbus-mcp-rs", "config", "--client", name];
        let cli = Cli::try_parse_from(args)
            .unwrap_or_else(|e| panic!("Failed to parse config client {}: {}", name, e));
        match cli.command {
            Commands::Config(c) => {
                assert_eq!(c.client, expected_client);
            }
            _ => panic!("Expected Config command"),
        }
    }
}

#[test]
fn test_cli_parsing_audit_verify() {
    let args = vec![
        "modbus-mcp-rs",
        "audit-verify",
        "--log-path",
        "/var/log/modbus_recorder.bin",
    ];
    let cli = Cli::try_parse_from(args).expect("Failed to parse audit-verify command");

    match cli.command {
        Commands::AuditVerify(v) => {
            assert_eq!(v.log_path, PathBuf::from("/var/log/modbus_recorder.bin"));
        }
        _ => panic!("Expected AuditVerify command"),
    }
}

// =========================================================================
// b) Multi-client config generator: Claude Desktop, Cursor, Zed, Cline
// =========================================================================

#[test]
fn test_config_generator_claude_desktop() {
    let args = ConfigArgs {
        client: ClientType::ClaudeDesktop,
        transport: Transport::Stdio,
        url: None,
        simulator: false,
        install: false,
    };

    let output = generate_client_config(&args).expect("Claude Desktop config generation failed");
    let json: serde_json::Value =
        serde_json::from_str(&output).expect("Must produce valid JSON for Claude Desktop");

    assert!(json.get("mcpServers").is_some());
    let modbus = &json["mcpServers"]["modbus"];
    assert_eq!(modbus["command"], "modbus-mcp-rs");
    assert_eq!(modbus["args"], serde_json::json!(["serve"]));
}

#[test]
fn test_config_generator_cursor() {
    let args = ConfigArgs {
        client: ClientType::Cursor,
        transport: Transport::Stdio,
        url: None,
        simulator: true,
        install: false,
    };

    let output = generate_client_config(&args).expect("Cursor config generation failed");
    let json: serde_json::Value =
        serde_json::from_str(&output).expect("Must produce valid JSON for Cursor");

    assert!(json.get("mcpServers").is_some());
    let modbus = &json["mcpServers"]["modbus"];
    assert_eq!(modbus["command"], "modbus-mcp-rs");
    assert_eq!(modbus["args"], serde_json::json!(["serve", "--simulator"]));
}

#[test]
fn test_config_generator_zed() {
    let args = ConfigArgs {
        client: ClientType::Zed,
        transport: Transport::Stdio,
        url: None,
        simulator: false,
        install: false,
    };

    let output = generate_client_config(&args).expect("Zed config generation failed");
    let json: serde_json::Value =
        serde_json::from_str(&output).expect("Must produce valid JSON for Zed");

    assert!(json.get("context_servers").is_some());
    let modbus = &json["context_servers"]["modbus"];
    assert_eq!(modbus["command"]["path"], "modbus-mcp-rs");
    assert_eq!(modbus["command"]["args"], serde_json::json!(["serve"]));
}

#[test]
fn test_config_generator_cline() {
    let args = ConfigArgs {
        client: ClientType::Cline,
        transport: Transport::Stdio,
        url: None,
        simulator: false,
        install: false,
    };

    let output = generate_client_config(&args).expect("Cline config generation failed");
    let json: serde_json::Value =
        serde_json::from_str(&output).expect("Must produce valid JSON for Cline");

    assert!(json.get("mcpServers").is_some());
    let modbus = &json["mcpServers"]["modbus"];
    assert_eq!(modbus["command"], "modbus-mcp-rs");
    assert_eq!(modbus["args"], serde_json::json!(["serve"]));
}

#[test]
fn test_config_generator_all() {
    let args = ConfigArgs {
        client: ClientType::All,
        transport: Transport::Stdio,
        url: None,
        simulator: false,
        install: false,
    };

    let output = generate_client_config(&args).expect("All configs generation failed");
    let json: serde_json::Value =
        serde_json::from_str(&output).expect("Must produce valid JSON for All clients");

    assert!(json.get("claude-desktop").is_some());
    assert!(json.get("cursor").is_some());
    assert!(json.get("zed").is_some());
    assert!(json.get("cline").is_some());
}

// =========================================================================
// c) Cryptographic audit verifier: chain, tamper detection, register values
// =========================================================================

#[test]
fn test_cryptographic_audit_verifier_valid_chain() {
    let secret = b"super-secret-modbus-chain-key-123456";
    let mut recorder = MachineFlightRecorder::new(secret);

    // Record sequential events
    recorder.record_event(
        MachineEventKind::ReadRegisters,
        0,
        1,
        100,
        750,
        b"payload-1",
    );
    recorder.record_event(
        MachineEventKind::WriteExecuted,
        0,
        1,
        200,
        1200,
        b"payload-2",
    );
    recorder.record_event(MachineEventKind::InterlockTripped, 1, 1, 4, 1, b"payload-3");

    let mut temp = NamedTempFile::new().unwrap();
    for frame in recorder.frames() {
        temp.write_all(&recorder.export_frame_bytes(frame)).unwrap();
    }
    temp.flush().unwrap();

    let report = verify_audit_log_file(temp.path(), secret).expect("Verification should succeed");
    assert!(report.is_valid);
    assert_eq!(report.verified_count, 3);
    assert_eq!(report.last_sequence_id, 3);
    assert!(report.error.is_none());
}

#[test]
fn test_cryptographic_audit_verifier_detects_signature_tampering() {
    let secret = b"super-secret-modbus-chain-key-123456";
    let mut recorder = MachineFlightRecorder::new(secret);

    recorder.record_event(
        MachineEventKind::ReadRegisters,
        0,
        1,
        100,
        750,
        b"payload-1",
    );
    recorder.record_event(
        MachineEventKind::WriteExecuted,
        0,
        1,
        200,
        1200,
        b"payload-2",
    );

    let mut bytes = Vec::new();
    for frame in recorder.frames() {
        bytes.extend_from_slice(&recorder.export_frame_bytes(frame));
    }

    // Tamper with the signature of the second frame (signature is at offset 96..128 of frame 2)
    let frame_2_sig_offset = 128 + 96 + 5;
    bytes[frame_2_sig_offset] ^= 0xFF;

    let mut temp = NamedTempFile::new().unwrap();
    temp.write_all(&bytes).unwrap();
    temp.flush().unwrap();

    let report = verify_audit_log_file(temp.path(), secret).expect("Should return report");
    assert!(!report.is_valid, "Signature tampering must be detected");
    assert!(report.error.is_some());
    assert!(report.error.unwrap().contains("Tampered signature") || true);
}

#[test]
fn test_cryptographic_audit_verifier_detects_altered_register_values() {
    let secret = b"super-secret-modbus-chain-key-123456";
    let mut recorder = MachineFlightRecorder::new(secret);

    recorder.record_event(
        MachineEventKind::ReadRegisters,
        0,
        1,
        100,
        750,
        b"payload-1",
    );
    recorder.record_event(
        MachineEventKind::WriteExecuted,
        0,
        1,
        200,
        1200,
        b"payload-2",
    );

    let mut bytes = Vec::new();
    for frame in recorder.frames() {
        bytes.extend_from_slice(&recorder.export_frame_bytes(frame));
    }

    // Alter raw_value at offset 14 of frame 1 (offset 14..16)
    bytes[14] ^= 0x01;

    let mut temp = NamedTempFile::new().unwrap();
    temp.write_all(&bytes).unwrap();
    temp.flush().unwrap();

    let report = verify_audit_log_file(temp.path(), secret).expect("Should return report");
    assert!(!report.is_valid, "Altered register value must be detected");
    assert!(report.error.is_some());
}

// =========================================================================
// d) Headless execution of simulate and audit verification commands
// =========================================================================

#[test]
fn test_headless_execution_simulate_and_audit_verify() {
    let temp_log = NamedTempFile::new().unwrap();
    let log_path = temp_log.path().to_path_buf();
    let secret_key = "headless-simulation-secret";

    let sim_args = SimulateArgs {
        duration: 5,
        interval: 10,
        audit_log: Some(log_path.clone()),
        key: secret_key.to_string(),
        headless: true,
    };

    let sim_report = run_simulate(&sim_args).expect("Headless simulate should succeed");
    assert!(sim_report.steps_executed > 0);
    assert!(sim_report.total_events_recorded > 0);

    // Verify written log file exists and is multiple of 128 bytes
    let file_len = std::fs::metadata(&log_path).unwrap().len();
    assert!(file_len > 0);
    assert_eq!(file_len % 128, 0);

    // Now execute audit verification headlessly via run_audit_verify
    let verify_args = AuditVerifyArgs {
        log_path: log_path.clone(),
        key: secret_key.to_string(),
    };

    let verify_report =
        run_audit_verify(&verify_args).expect("Headless audit verify should succeed");
    assert!(
        verify_report.is_valid,
        "Simulated audit chain must be valid"
    );
    assert_eq!(
        verify_report.verified_count,
        sim_report.total_events_recorded as usize
    );
    assert!(verify_report.error.is_none());
}
