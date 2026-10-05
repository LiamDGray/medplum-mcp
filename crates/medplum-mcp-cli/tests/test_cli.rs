//! Integration and unit tests for the medplum-mcp-rs CLI.

use std::io::Write;
use std::path::PathBuf;

use clap::Parser;
use medplum_mcp_cli::bench::run_micro_benchmarks;
use medplum_mcp_cli::cli::{
    BenchArgs, Cli, ClientType, Commands, ConfigArgs, OutputFormat, Transport, VerifyArgs,
};
use medplum_mcp_cli::config::generate_client_config;
use medplum_mcp_cli::verify::run_verification;
use medplum_mcp_core::audit::{
    compute_entry_signature, compute_payload_digest, ActionStatus, GENESIS_PREV_SIGNATURE,
};
use serde_json::json;
use tempfile::NamedTempFile;

#[test]
fn test_cli_parsing_serve() {
    let args = vec!["medplum-mcp-rs", "serve"];
    let cli = Cli::try_parse_from(args).expect("Should parse default serve");
    match cli.command {
        Commands::Serve(s) => {
            assert_eq!(s.transport, Transport::Stdio);
            assert_eq!(s.host, "0.0.0.0");
            assert_eq!(s.port, 8000);
            assert!(!s.demo);
            assert!(!s.allow_writes);
            assert_eq!(s.base_url, "https://api.medplum.com");
            assert_eq!(s.audit_key, "default-medplum-audit-key");
        }
        _ => panic!("Expected Serve command"),
    }

    let custom_args = vec![
        "medplum-mcp-rs",
        "serve",
        "--transport",
        "sse",
        "--host",
        "127.0.0.1",
        "--port",
        "9000",
        "--demo",
        "--allow-writes",
        "--base-url",
        "https://custom.medplum.org",
        "--audit-key",
        "secret-key-123",
    ];
    let cli = Cli::try_parse_from(custom_args).expect("Should parse custom serve");
    match cli.command {
        Commands::Serve(s) => {
            assert_eq!(s.transport, Transport::Sse);
            assert_eq!(s.host, "127.0.0.1");
            assert_eq!(s.port, 9000);
            assert!(s.demo);
            assert!(s.allow_writes);
            assert_eq!(s.base_url, "https://custom.medplum.org");
            assert_eq!(s.audit_key, "secret-key-123");
        }
        _ => panic!("Expected Serve command"),
    }
}

#[test]
fn test_cli_parsing_mock_server() {
    let args = vec!["medplum-mcp-rs", "mock-server"];
    let cli = Cli::try_parse_from(args).expect("Should parse default mock-server");
    match cli.command {
        Commands::MockServer(m) => {
            assert_eq!(m.host, "127.0.0.1");
            assert_eq!(m.port, 8080);
        }
        _ => panic!("Expected MockServer command"),
    }

    let custom = vec![
        "medplum-mcp-rs",
        "mock-server",
        "--host",
        "0.0.0.0",
        "--port",
        "8888",
    ];
    let cli = Cli::try_parse_from(custom).expect("Should parse custom mock-server");
    match cli.command {
        Commands::MockServer(m) => {
            assert_eq!(m.host, "0.0.0.0");
            assert_eq!(m.port, 8888);
        }
        _ => panic!("Expected MockServer command"),
    }
}

#[test]
fn test_cli_parsing_verify() {
    let args = vec!["medplum-mcp-rs", "verify"];
    let cli = Cli::try_parse_from(args).expect("Should parse default verify");
    match cli.command {
        Commands::Verify(v) => {
            assert_eq!(v.output, OutputFormat::Terminal);
            assert!(!v.strict);
            assert!(v.audit_log.is_none());
        }
        _ => panic!("Expected Verify command"),
    }

    let custom = vec![
        "medplum-mcp-rs",
        "verify",
        "--audit-log",
        "/tmp/audit.jsonl",
        "--audit-key",
        "test-secret",
        "--strict",
        "--output",
        "json",
    ];
    let cli = Cli::try_parse_from(custom).expect("Should parse custom verify");
    match cli.command {
        Commands::Verify(v) => {
            assert_eq!(v.output, OutputFormat::Json);
            assert!(v.strict);
            assert_eq!(v.audit_log, Some(PathBuf::from("/tmp/audit.jsonl")));
            assert_eq!(v.audit_key.as_deref(), Some("test-secret"));
        }
        _ => panic!("Expected Verify command"),
    }
}

#[test]
fn test_cli_parsing_config() {
    let args = vec!["medplum-mcp-rs", "config"];
    let cli = Cli::try_parse_from(args).expect("Should parse default config");
    match cli.command {
        Commands::Config(c) => {
            assert_eq!(c.client, ClientType::ClaudeDesktop);
            assert_eq!(c.transport, Transport::Stdio);
            assert!(!c.demo);
            assert!(!c.install);
        }
        _ => panic!("Expected Config command"),
    }

    let custom = vec![
        "medplum-mcp-rs",
        "config",
        "--client",
        "windsurf",
        "--demo",
        "--transport",
        "sse",
        "--url",
        "https://mcp.hospital.org/sse",
        "--install",
    ];
    let cli = Cli::try_parse_from(custom).expect("Should parse custom config");
    match cli.command {
        Commands::Config(c) => {
            assert_eq!(c.client, ClientType::Windsurf);
            assert!(c.demo);
            assert_eq!(c.transport, Transport::Sse);
            assert_eq!(c.url.as_deref(), Some("https://mcp.hospital.org/sse"));
            assert!(c.install);
        }
        _ => panic!("Expected Config command"),
    }
}

#[test]
fn test_cli_parsing_bench() {
    let args = vec!["medplum-mcp-rs", "bench"];
    let cli = Cli::try_parse_from(args).expect("Should parse bench command");
    match cli.command {
        Commands::Bench(b) => {
            assert_eq!(b.iterations, 1000);
        }
        _ => panic!("Expected Bench command"),
    }
}

#[test]
fn test_verify_valid_audit_log() {
    let mut file = NamedTempFile::new().unwrap();
    let key = b"test-verification-secret";
    let ts1 = "2026-10-04T12:00:00Z";
    let payload1 = json!({"resourceType": "Patient", "id": "p1"});
    let digest1 = compute_payload_digest(Some(&payload1));
    let sig1 = compute_entry_signature(
        key,
        1,
        ts1,
        ActionStatus::Allowed.as_str(),
        "medplum_get_patient",
        &digest1,
        GENESIS_PREV_SIGNATURE,
    )
    .unwrap();

    let entry1 = json!({
        "sequence_id": 1,
        "timestamp": ts1,
        "action_status": "ALLOWED",
        "tool_name": "medplum_get_patient",
        "payload_digest": digest1,
        "prev_signature": GENESIS_PREV_SIGNATURE,
        "signature": sig1,
        "payload": payload1
    });

    let ts2 = "2026-10-04T12:00:01Z";
    let payload2 = json!({"resourceType": "Observation", "id": "obs1"});
    let digest2 = compute_payload_digest(Some(&payload2));
    let sig2 = compute_entry_signature(
        key,
        2,
        ts2,
        ActionStatus::Allowed.as_str(),
        "medplum_get_observation",
        &digest2,
        &sig1,
    )
    .unwrap();

    let entry2 = json!({
        "sequence_id": 2,
        "timestamp": ts2,
        "action_status": "ALLOWED",
        "tool_name": "medplum_get_observation",
        "payload_digest": digest2,
        "prev_signature": sig1,
        "signature": sig2,
        "payload": payload2
    });

    writeln!(file, "{}", entry1).unwrap();
    writeln!(file, "{}", entry2).unwrap();
    file.flush().unwrap();

    let args = VerifyArgs {
        audit_log: Some(file.path().to_path_buf()),
        audit_key: Some(String::from_utf8_lossy(key).to_string()),
        strict: true,
        output: OutputFormat::Json,
    };

    let report = run_verification(&args).expect("Verification should execute");
    assert!(
        report.success,
        "Valid audit log and FSM should pass verification"
    );
    assert_eq!(report.audit_block_height, 2);
    assert!(report.fsm_invariants_proven);
    assert!(report.smt_theorems_proven);
}

#[test]
fn test_verify_tampered_audit_log_strict_failure() {
    let mut file = NamedTempFile::new().unwrap();
    let key = b"test-verification-secret";
    let ts1 = "2026-10-04T12:00:00Z";
    let payload1 = json!({"resourceType": "Patient", "id": "p1"});
    let digest1 = compute_payload_digest(Some(&payload1));
    let sig1 = compute_entry_signature(
        key,
        1,
        ts1,
        ActionStatus::Allowed.as_str(),
        "medplum_get_patient",
        &digest1,
        GENESIS_PREV_SIGNATURE,
    )
    .unwrap();

    // Tamper payload digest
    let entry1 = json!({
        "sequence_id": 1,
        "timestamp": ts1,
        "action_status": "ALLOWED",
        "tool_name": "medplum_get_patient",
        "payload_digest": "tampered_digest_value",
        "prev_signature": GENESIS_PREV_SIGNATURE,
        "signature": sig1,
        "payload": payload1
    });

    writeln!(file, "{}", entry1).unwrap();
    file.flush().unwrap();

    let args = VerifyArgs {
        audit_log: Some(file.path().to_path_buf()),
        audit_key: Some(String::from_utf8_lossy(key).to_string()),
        strict: true,
        output: OutputFormat::Json,
    };

    let report = run_verification(&args);
    // In strict mode, tampered audit log fails (either Err or report.success == false)
    if let Ok(r) = report {
        assert!(
            !r.success,
            "Report must not be marked success on tampered log"
        );
    }
}

#[test]
fn test_config_generates_valid_claude_desktop_json() {
    let args = ConfigArgs {
        client: ClientType::ClaudeDesktop,
        demo: true,
        transport: Transport::Stdio,
        url: None,
        install: false,
    };

    let config_str = generate_client_config(&args).expect("Config generation should succeed");
    let parsed: serde_json::Value = serde_json::from_str(&config_str).expect("Must be valid JSON");

    assert!(parsed.get("mcpServers").is_some());
    let medplum = &parsed["mcpServers"]["medplum"];
    assert_eq!(medplum["command"], "medplum-mcp-rs");
    assert_eq!(medplum["args"], json!(["--demo"]));
}

#[test]
fn test_config_generates_all_clients() {
    let clients = vec![
        ClientType::ClaudeDesktop,
        ClientType::ClaudeCode,
        ClientType::Cursor,
        ClientType::Windsurf,
        ClientType::PiAgent,
        ClientType::HermesAgent,
        ClientType::CodexCli,
    ];

    for client in clients {
        let args = ConfigArgs {
            client,
            demo: false,
            transport: Transport::Stdio,
            url: None,
            install: false,
        };
        let output = generate_client_config(&args).expect("Should generate config for client");
        assert!(!output.trim().is_empty());
        assert!(output.contains("medplum-mcp-rs") || output.contains("medplum"));
    }
}

#[test]
fn test_bench_executes_and_returns_metrics() {
    let args = BenchArgs {
        iterations: 100,
        json: false,
    };

    let report = run_micro_benchmarks(&args).expect("Micro benchmarks should execute successfully");
    assert!(report.distill_records_per_sec > 0.0);
    assert!(report.distill_mean_micros > 0.0);
    assert!(report.safety_checks_per_sec > 0.0);
    assert!(report.audit_records_per_sec > 0.0);
}

#[test]
fn test_cli_parsing_soak() {
    let args = vec![
        "medplum-mcp-rs",
        "soak",
        "--duration-secs",
        "120",
        "--workers",
        "8",
        "--report-interval-secs",
        "10",
        "--log-path",
        "custom_soak.log",
    ];

    let cli = Cli::try_parse_from(args).expect("Should parse soak command");
    match cli.command {
        Commands::Soak(s) => {
            assert_eq!(s.duration_secs, 120);
            assert_eq!(s.workers, 8);
            assert_eq!(s.report_interval_secs, 10);
            assert_eq!(s.log_path, PathBuf::from("custom_soak.log"));
        }
        _ => panic!("Expected Soak command"),
    }
}

#[test]
fn test_soak_execution_short_duration() {
    let temp_log = NamedTempFile::new().unwrap();
    let args = medplum_mcp_cli::cli::SoakArgs {
        duration_secs: 2,
        workers: 4,
        report_interval_secs: 1,
        log_path: temp_log.path().to_path_buf(),
    };

    let res = medplum_mcp_cli::soak::run_soak_test(&args);
    assert!(
        res.is_ok(),
        "Short soak test must run and complete with zero errors"
    );
}

#[test]
fn test_cli_parsing_tui() {
    let args = vec![
        "medplum-mcp-rs",
        "tui",
        "--demo",
        "--tick-rate-ms",
        "100",
        "--headless-ticks",
        "5",
    ];

    let cli = Cli::try_parse_from(args).expect("Should parse tui command");
    match cli.command {
        Commands::Tui(t) => {
            assert!(t.demo);
            assert_eq!(t.tick_rate_ms, 100);
            assert_eq!(t.headless_ticks, Some(5));
        }
        _ => panic!("Expected Tui command"),
    }
}

#[test]
fn test_tui_app_metrics_and_state() {
    use medplum_mcp_cli::tui::TuiApp;

    let app = TuiApp::new_demo();
    assert!(app.total_distillations > 0);
    assert!(app.tokens_saved > 0);
    assert_eq!(app.violations_count, 0);
    assert!(app.recent_audit_entries.len() >= 3);
    assert!(app.token_reduction_pct >= 85.0);
}

#[test]
fn test_tui_headless_execution() {
    use medplum_mcp_cli::cli::TuiArgs;
    use medplum_mcp_cli::tui::run_tui_app;

    let args = TuiArgs {
        demo: true,
        tick_rate_ms: 10,
        headless_ticks: Some(3),
    };

    let res = run_tui_app(&args);
    assert!(
        res.is_ok(),
        "Headless TUI app must execute and exit cleanly"
    );
}
