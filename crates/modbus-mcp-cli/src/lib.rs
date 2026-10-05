//! Industrial IoT Modbus Bridge CLI, Server, Simulator, and Audit Verifier.

pub mod config;
pub mod verifier;

use std::path::PathBuf;
use std::sync::Arc;

use clap::{Args, Parser, Subcommand, ValueEnum};
use modbus_mcp_server::mcp::ModbusMcpServer;
use modbus_mcp_server::simulator::VirtualPlcSimulator;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tracing::info;

/// High-Assurance Industrial IoT Modbus Bridge MCP Server CLI.
#[derive(Parser, Debug)]
#[command(
    name = "modbus-mcp-rs",
    about = "Industrial IoT Modbus Bridge MCP Server with cryptographic flight recording and physical simulator",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start the Model Context Protocol (MCP) server
    Serve(ServeArgs),

    /// Run the physical virtual PLC reactor and pump simulator
    Simulate(SimulateArgs),

    /// Generate AI agent client configuration (Claude Desktop, Cursor, Zed, Cline)
    Config(ConfigArgs),

    /// Verify cryptographic machine flight recorder audit chain
    AuditVerify(AuditVerifyArgs),
}

/// Transport protocol for MCP communication.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Transport {
    #[default]
    #[value(name = "stdio")]
    Stdio,
    #[value(name = "sse")]
    Sse,
}

/// Supported AI agent client ecosystems.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ClientType {
    #[default]
    #[value(name = "claude-desktop")]
    ClaudeDesktop,
    #[value(name = "cursor")]
    Cursor,
    #[value(name = "zed")]
    Zed,
    #[value(name = "cline")]
    Cline,
    #[value(name = "all")]
    All,
}

/// Arguments for `serve` subcommand.
#[derive(Args, Debug, Clone)]
pub struct ServeArgs {
    /// MCP transport protocol (stdio or sse)
    #[arg(long, default_value = "stdio", value_enum)]
    pub transport: Transport,

    /// Host interface to bind SSE listener
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// Port to bind SSE listener
    #[arg(long, default_value_t = 8000)]
    pub port: u16,

    /// Enable the physical virtual PLC reactor and pump simulator
    #[arg(long, default_value_t = false)]
    pub simulator: bool,

    /// Path to binary cryptographic flight recorder audit log (.bin)
    #[arg(long)]
    pub audit_log: Option<PathBuf>,

    /// Secret key for HMAC-SHA256 audit ledger signatures
    #[arg(long, default_value = "modbus-audit-secret")]
    pub audit_key: String,
}

/// Arguments for `simulate` subcommand.
#[derive(Args, Debug, Clone)]
pub struct SimulateArgs {
    /// Simulation duration in seconds or cycles
    #[arg(long, default_value_t = 1, alias = "duration-secs")]
    pub duration: u64,

    /// Simulation step interval in milliseconds
    #[arg(long, default_value_t = 100, alias = "interval-ms")]
    pub interval: u64,

    /// Audit log file path to write flight recorder frames
    #[arg(long, alias = "output-log")]
    pub audit_log: Option<PathBuf>,

    /// Secret key for HMAC-SHA256 flight recording
    #[arg(long, default_value = "modbus-audit-secret")]
    pub key: String,

    /// Run in fast headless mode without wall-clock sleeps
    #[arg(long, default_value_t = false)]
    pub headless: bool,
}

/// Arguments for `config` subcommand.
#[derive(Args, Debug, Clone)]
pub struct ConfigArgs {
    /// Target AI agent client ecosystem
    #[arg(long, default_value = "claude-desktop", value_enum)]
    pub client: ClientType,

    /// Server transport protocol in generated config
    #[arg(long, default_value = "stdio", value_enum)]
    pub transport: Transport,

    /// Server SSE endpoint URL when transport is sse
    #[arg(long)]
    pub url: Option<String>,

    /// Include `--simulator` flag in command arguments
    #[arg(long, default_value_t = false)]
    pub simulator: bool,

    /// Automatically install or merge into the client's native configuration file
    #[arg(long, default_value_t = false)]
    pub install: bool,
}

/// Arguments for `audit-verify` subcommand.
#[derive(Args, Debug, Clone)]
pub struct AuditVerifyArgs {
    /// Path to binary audit log file (.bin)
    #[arg(long, alias = "audit-log")]
    pub log_path: PathBuf,

    /// Secret key for HMAC-SHA256 audit verification
    #[arg(long, default_value = "modbus-audit-secret", alias = "audit-key")]
    pub key: String,
}

/// Simulation execution report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulateReport {
    pub steps_executed: u64,
    pub total_events_recorded: u64,
    pub audit_log_path: Option<PathBuf>,
}

/// Runs the physical virtual PLC simulation headlessly or interactively.
pub fn run_simulate(args: &SimulateArgs) -> Result<SimulateReport, String> {
    let mut simulator = VirtualPlcSimulator::new(args.key.as_bytes());

    let interval = args.interval.max(1);
    let total_steps = if args.duration == 0 {
        1
    } else {
        ((args.duration * 1000) / interval)
            .max(args.duration)
            .min(1000)
    };

    for i in 0..total_steps {
        simulator.step();

        // Perform standard reads and state exercises to populate flight recorder frames
        let _ = simulator.read_input_registers(0, 4);

        if i % 10 == 0 {
            let _ = simulator.read_discrete_inputs(0, 4);
        }
        if i % 25 == 0 {
            let _ = simulator.read_coils(0, 5);
        }

        if !args.headless {
            std::thread::sleep(std::time::Duration::from_millis(args.interval.min(100)));
        }
    }

    let recorder = simulator.flight_recorder();
    let guard = recorder
        .lock()
        .map_err(|e| format!("Mutex poison error: {e}"))?;
    let total_events = guard.frames().len() as u64;

    if let Some(log_path) = &args.audit_log {
        let mut bytes = Vec::with_capacity(guard.frames().len() * 128);
        for frame in guard.frames() {
            bytes.extend_from_slice(&guard.export_frame_bytes(frame));
        }

        if let Some(parent) = log_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(log_path, &bytes)
            .map_err(|e| format!("Failed to write audit log to {log_path:?}: {e}"))?;
    }

    Ok(SimulateReport {
        steps_executed: total_steps,
        total_events_recorded: total_events,
        audit_log_path: args.audit_log.clone(),
    })
}

/// Runs the Modbus MCP server with stdio or SSE transport.
pub async fn run_serve(args: ServeArgs) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let simulator = VirtualPlcSimulator::new(args.audit_key.as_bytes());
    let sim_arc = Arc::new(Mutex::new(simulator));
    let server = ModbusMcpServer::new(sim_arc.clone(), args.audit_key.as_bytes().to_vec());

    match args.transport {
        Transport::Stdio => {
            info!("Starting Modbus MCP server on stdio transport");
            let stdin = tokio::io::stdin();
            let mut stdout = tokio::io::stdout();
            let mut reader = BufReader::new(stdin).lines();

            while let Some(line) = reader.next_line().await? {
                if let Some(resp) = server.handle_jsonrpc_message(&line).await {
                    stdout.write_all(resp.as_bytes()).await?;
                    stdout.write_all(b"\n").await?;
                    stdout.flush().await?;
                }
            }
        }
        Transport::Sse => {
            let bind_addr = format!("{}:{}", args.host, args.port);
            info!("Starting Modbus MCP SSE server on http://{}", bind_addr);
            eprintln!("Modbus MCP SSE server listening on http://{}", bind_addr);
            let listener = TcpListener::bind(&bind_addr).await?;
            let app = server.into_router();
            axum::serve(listener, app).await?;
        }
    }

    Ok(())
}
