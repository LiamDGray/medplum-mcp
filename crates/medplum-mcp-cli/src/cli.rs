//! CLI argument definitions and subcommand specifications for `medplum-mcp-rs`.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

/// Enterprise Model Context Protocol (MCP) Server for Medplum HL7 FHIR.
#[derive(Parser, Debug)]
#[command(
    name = "medplum-mcp-rs",
    about = "Production-grade Rust MCP server for Medplum HL7 FHIR with HIPAA cryptographic audit and deterministic safety gates",
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

    /// Start the OpenAPI 3.0 conforming FHIR R4 mock HTTP server
    MockServer(MockServerArgs),

    /// Verify deterministic safety gates, FSM reachability invariants, and HIPAA audit ledger
    Verify(VerifyArgs),

    /// Generate or install AI agent client configuration (Claude, Cursor, Windsurf, etc.)
    Config(ConfigArgs),

    /// Run empirical high-throughput microsecond performance benchmarks
    Bench(BenchArgs),

    /// Run long-duration soak test, continuous fuzzing, and invariant validation
    Soak(SoakArgs),

    /// Launch the live interactive 60 FPS terminal dashboard (Ratatui TUI)
    Tui(TuiArgs),

    /// Export the MCP tools as an OpenAPI 3.1.0 specification with optional Overlay 1.0 annotations
    Openapi(OpenapiArgs),
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

/// Output formatting option for verification certificates.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum OutputFormat {
    #[default]
    #[value(name = "terminal")]
    Terminal,
    #[value(name = "json")]
    Json,
    #[value(name = "markdown")]
    Markdown,
}

/// Supported AI agent client ecosystems.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ClientType {
    #[default]
    #[value(name = "claude-desktop")]
    ClaudeDesktop,
    #[value(name = "claude-code")]
    ClaudeCode,
    #[value(name = "cursor")]
    Cursor,
    #[value(name = "windsurf")]
    Windsurf,
    #[value(name = "pi-agent")]
    PiAgent,
    #[value(name = "hermes-agent")]
    HermesAgent,
    #[value(name = "codex-cli")]
    CodexCli,
    #[value(name = "all")]
    All,
}

/// Audit ledger storage format (jsonl, binary, dual).
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum AuditFormat {
    #[default]
    #[value(name = "jsonl")]
    Jsonl,
    #[value(name = "binary")]
    Binary,
    #[value(name = "dual")]
    Dual,
}

/// Arguments for `serve` subcommand.
#[derive(Args, Debug, Clone)]
pub struct ServeArgs {
    /// MCP transport protocol (stdio or sse)
    #[arg(long, default_value = "stdio", value_enum)]
    pub transport: Transport,

    /// Host interface to bind SSE listener
    #[arg(long, default_value = "0.0.0.0")]
    pub host: String,

    /// Port to bind SSE listener
    #[arg(long, default_value_t = 8000)]
    pub port: u16,

    /// Run with St. Jude Children's Research Hospital synthetic demo sandbox
    #[arg(long, default_value_t = false)]
    pub demo: bool,

    /// Permit clinical draft mutations (strictly restricted to draft state)
    #[arg(long, default_value_t = false)]
    pub allow_writes: bool,

    /// Upstream Medplum FHIR base URL
    #[arg(long, default_value = "https://api.medplum.com")]
    pub base_url: String,

    /// Path to append-only HIPAA cryptographic audit flight recorder (.jsonl or .bin)
    #[arg(long)]
    pub audit_log: Option<PathBuf>,

    /// Secret key for HMAC-SHA256 audit ledger signatures
    #[arg(long, default_value = "default-medplum-audit-key")]
    pub audit_key: String,

    /// Format for audit flight recorder (jsonl, binary, or dual)
    #[arg(long, default_value = "jsonl", value_enum)]
    pub audit_format: AuditFormat,
}

/// Arguments for `mock-server` subcommand.
#[derive(Args, Debug, Clone)]
pub struct MockServerArgs {
    /// Host interface to bind OpenAPI mock server
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// Port to bind OpenAPI mock server
    #[arg(long, default_value_t = 8080)]
    pub port: u16,
}

/// Arguments for `verify` subcommand.
#[derive(Args, Debug, Clone)]
pub struct VerifyArgs {
    /// Explicit path to HMAC-SHA256 audit ledger (.jsonl)
    #[arg(long)]
    pub audit_log: Option<PathBuf>,

    /// Secret HMAC key for signature verification
    #[arg(long)]
    pub audit_key: Option<String>,

    /// Exit with code 1 on any invariant failure or missing audit log
    #[arg(long, default_value_t = false)]
    pub strict: bool,

    /// Output report format (terminal, json, markdown)
    #[arg(long, default_value = "terminal", value_enum)]
    pub output: OutputFormat,
}

/// Arguments for `config` subcommand.
#[derive(Args, Debug, Clone)]
pub struct ConfigArgs {
    /// Target AI agent client ecosystem
    #[arg(long, default_value = "claude-desktop", value_enum)]
    pub client: ClientType,

    /// Enable `--demo` flag in generated server command
    #[arg(long, default_value_t = false)]
    pub demo: bool,

    /// Server transport protocol in generated config
    #[arg(long, default_value = "stdio", value_enum)]
    pub transport: Transport,

    /// Server SSE endpoint URL when transport is sse
    #[arg(long)]
    pub url: Option<String>,

    /// Automatically install or merge into the client's native configuration file
    #[arg(long, default_value_t = false)]
    pub install: bool,
}

/// Arguments for `bench` subcommand.
#[derive(Args, Debug, Clone)]
pub struct BenchArgs {
    /// Number of microsecond benchmark iterations
    #[arg(long, default_value_t = 1000)]
    pub iterations: usize,

    /// Output benchmark metrics in machine-readable JSON format
    #[arg(long, default_value_t = false)]
    pub json: bool,
}

/// Arguments for `soak` long-duration fuzzing and stress subcommand.
#[derive(Args, Debug, Clone)]
pub struct SoakArgs {
    /// Duration in seconds to run the continuous soak test (default: 5400 = 90 minutes; 0 = indefinite)
    #[arg(long, default_value_t = 5400)]
    pub duration_secs: u64,

    /// Number of concurrent worker threads
    #[arg(long, default_value_t = 16)]
    pub workers: usize,

    /// Telemetry and progress reporting interval in seconds
    #[arg(long, default_value_t = 30)]
    pub report_interval_secs: u64,

    /// Log file path to append human-readable progress and JSON metrics
    #[arg(long, default_value = "soak_test_report.log")]
    pub log_path: PathBuf,

    /// Optional path to live tally JSON file to record/accumulate operation totals indefinitely
    #[arg(long)]
    pub tally_file: Option<PathBuf>,
}

/// Arguments for `tui` live terminal dashboard subcommand.
#[derive(Args, Debug, Clone)]
pub struct TuiArgs {
    /// Enable rich synthetic St. Jude demo dataset with simulated traffic
    #[arg(long, default_value_t = true)]
    pub demo: bool,

    /// TUI refresh and event loop tick rate in milliseconds
    #[arg(long, default_value_t = 250)]
    pub tick_rate_ms: u64,

    /// Run for a fixed number of headless render ticks (used for testing and CI)
    #[arg(long)]
    pub headless_ticks: Option<u64>,
}

/// Arguments for `openapi` specification export subcommand.
#[derive(Args, Debug, Clone)]
pub struct OpenapiArgs {
    /// Inject OpenAPI Overlay 1.0 AI-friendly docstrings ("Use when: ...")
    #[arg(long, default_value_t = false)]
    pub overlay: bool,

    /// Output path to write the JSON specification (defaults to stdout)
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}
