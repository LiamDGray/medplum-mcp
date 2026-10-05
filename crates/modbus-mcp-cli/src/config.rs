//! AI agent client configuration generator for Modbus MCP.

use serde_json::{json, Value};
use thiserror::Error;

use crate::{ClientType, ConfigArgs, Transport};

pub const DEFAULT_SSE_URL: &str = "http://localhost:8000/sse";

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("YAML serialization error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("Unsupported client: {0}")]
    Unsupported(String),
}

/// Generates the server entry for `modbus-mcp-rs` in standard MCP format.
pub fn generate_server_entry(simulator: bool, transport: Transport, url: Option<&str>) -> Value {
    match transport {
        Transport::Sse => {
            let sse_url = url.unwrap_or(DEFAULT_SSE_URL);
            json!({
                "url": sse_url
            })
        }
        Transport::Stdio => {
            let mut args = vec!["serve".to_string()];
            if simulator {
                args.push("--simulator".to_string());
            }
            json!({
                "command": "modbus-mcp-rs",
                "args": args
            })
        }
    }
}

/// Generates the configuration JSON string for the requested client.
pub fn generate_client_config(args: &ConfigArgs) -> Result<String, ConfigError> {
    if args.client == ClientType::All {
        return generate_all_configs(args);
    }

    let server_entry = generate_server_entry(args.simulator, args.transport, args.url.as_deref());

    let val = match args.client {
        ClientType::ClaudeDesktop | ClientType::Cursor | ClientType::Cline => {
            json!({
                "mcpServers": {
                    "modbus": server_entry
                }
            })
        }
        ClientType::Zed => {
            let zed_entry = match args.transport {
                Transport::Sse => {
                    let sse_url = args.url.as_deref().unwrap_or(DEFAULT_SSE_URL);
                    json!({
                        "url": sse_url
                    })
                }
                Transport::Stdio => {
                    let mut cmd_args = vec!["serve".to_string()];
                    if args.simulator {
                        cmd_args.push("--simulator".to_string());
                    }
                    json!({
                        "command": {
                            "path": "modbus-mcp-rs",
                            "args": cmd_args
                        }
                    })
                }
            };
            json!({
                "context_servers": {
                    "modbus": zed_entry
                }
            })
        }
        ClientType::All => unreachable!(),
    };

    Ok(serde_json::to_string_pretty(&val)? + "\n")
}

/// Generates configurations for all supported AI desktop clients.
fn generate_all_configs(args: &ConfigArgs) -> Result<String, ConfigError> {
    let clients = [
        ClientType::ClaudeDesktop,
        ClientType::Cursor,
        ClientType::Zed,
        ClientType::Cline,
    ];

    let mut map = serde_json::Map::new();
    for c in clients {
        let mut sub_args = args.clone();
        sub_args.client = c;
        let rendered = generate_client_config(&sub_args)?;
        let parsed: Value = serde_json::from_str(&rendered)?;
        let key = match c {
            ClientType::ClaudeDesktop => "claude-desktop",
            ClientType::Cursor => "cursor",
            ClientType::Zed => "zed",
            ClientType::Cline => "cline",
            ClientType::All => unreachable!(),
        };
        map.insert(key.to_string(), parsed);
    }

    Ok(serde_json::to_string_pretty(&Value::Object(map))? + "\n")
}
