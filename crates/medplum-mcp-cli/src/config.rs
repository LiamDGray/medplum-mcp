//! Multi-agent configuration generator and non-destructive installer.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use thiserror::Error;

use crate::cli::{ClientType, ConfigArgs, Transport};

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

/// Get the default client configuration file path for an AI agent ecosystem.
pub fn get_client_config_path(client: ClientType, base_dir: Option<&Path>) -> Option<PathBuf> {
    let home = match base_dir {
        Some(b) => b.to_path_buf(),
        None => {
            let h = std::env::var_os("HOME")?;
            PathBuf::from(h)
        }
    };

    match client {
        ClientType::ClaudeDesktop => {
            #[cfg(target_os = "macos")]
            {
                Some(
                    home.join("Library")
                        .join("Application Support")
                        .join("Claude")
                        .join("claude_desktop_config.json"),
                )
            }
            #[cfg(target_os = "windows")]
            {
                let appdata = std::env::var_os("APPDATA")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| home.join("AppData").join("Roaming"));
                Some(appdata.join("Claude").join("claude_desktop_config.json"))
            }
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            {
                Some(
                    home.join(".config")
                        .join("Claude")
                        .join("claude_desktop_config.json"),
                )
            }
        }
        ClientType::ClaudeCode => Some(home.join(".claude").join("mcp.json")),
        ClientType::Cursor => Some(home.join(".cursor").join("mcp.json")),
        ClientType::Windsurf => Some(
            home.join(".codeium")
                .join("windsurf")
                .join("mcp_config.json"),
        ),
        ClientType::PiAgent => Some(home.join(".pi").join("agent").join("mcp.json")),
        ClientType::HermesAgent => Some(home.join(".hermes").join("config.yaml")),
        ClientType::CodexCli => Some(home.join(".codex").join("config.yaml")),
        ClientType::All => None,
    }
}

/// Generate the server entry payload for `medplum-mcp-rs`.
pub fn generate_server_entry(demo: bool, transport: Transport, url: Option<&str>) -> Value {
    match transport {
        Transport::Sse => {
            let sse_url = url.unwrap_or(DEFAULT_SSE_URL);
            json!({
                "url": sse_url
            })
        }
        Transport::Stdio => {
            let mut args = Vec::new();
            if demo {
                args.push("--demo");
            }
            json!({
                "command": "medplum-mcp-rs",
                "args": args
            })
        }
    }
}

/// Generate configuration string for the given client.
pub fn generate_client_config(args: &ConfigArgs) -> Result<String, ConfigError> {
    if args.client == ClientType::All {
        return generate_all_configs(args);
    }

    let server_entry = generate_server_entry(args.demo, args.transport, args.url.as_deref());

    match args.client {
        ClientType::ClaudeDesktop
        | ClientType::ClaudeCode
        | ClientType::Cursor
        | ClientType::Windsurf
        | ClientType::PiAgent => {
            let config_json = json!({
                "mcpServers": {
                    "medplum": server_entry
                }
            });
            Ok(serde_json::to_string_pretty(&config_json)? + "\n")
        }
        ClientType::HermesAgent => {
            let config_yaml = json!({
                "mcp_servers": {
                    "medplum": server_entry
                }
            });
            let rendered = serde_yaml::to_string(&config_yaml)?;
            Ok(rendered)
        }
        ClientType::CodexCli => {
            let codex_item = match server_entry {
                Value::Object(mut map) => {
                    map.insert("name".to_string(), json!("medplum"));
                    Value::Object(map)
                }
                _ => json!({"name": "medplum"}),
            };
            let config_yaml = json!({
                "tools": {
                    "mcp": [codex_item]
                }
            });
            let rendered = serde_yaml::to_string(&config_yaml)?;
            Ok(rendered)
        }
        ClientType::All => unreachable!(),
    }
}

/// Generate combined configurations for all clients.
fn generate_all_configs(args: &ConfigArgs) -> Result<String, ConfigError> {
    let clients = [
        ClientType::ClaudeDesktop,
        ClientType::ClaudeCode,
        ClientType::Cursor,
        ClientType::Windsurf,
        ClientType::PiAgent,
        ClientType::HermesAgent,
        ClientType::CodexCli,
    ];

    let mut map = serde_json::Map::new();
    for c in clients {
        let mut sub_args = args.clone();
        sub_args.client = c;
        let rendered = generate_client_config(&sub_args)?;
        let val: Value = if c == ClientType::HermesAgent || c == ClientType::CodexCli {
            serde_yaml::from_str(&rendered)?
        } else {
            serde_json::from_str(&rendered)?
        };
        map.insert(format!("{:?}", c).to_lowercase(), val);
    }

    Ok(serde_json::to_string_pretty(&Value::Object(map))? + "\n")
}

/// Safely install or merge configuration into the target agent's config file.
pub fn install_client_config(
    args: &ConfigArgs,
    base_dir: Option<&Path>,
) -> Result<PathBuf, ConfigError> {
    let target_path = get_client_config_path(args.client, base_dir).ok_or_else(|| {
        ConfigError::Unsupported("Cannot resolve configuration path for target client".to_string())
    })?;

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let is_yaml = matches!(args.client, ClientType::HermesAgent | ClientType::CodexCli);

    let server_entry = generate_server_entry(args.demo, args.transport, args.url.as_deref());

    if target_path.exists() {
        let content = fs::read_to_string(&target_path)?;
        if is_yaml {
            let mut existing: Value = serde_yaml::from_str(&content).unwrap_or_else(|_| json!({}));
            if !existing.is_object() {
                existing = json!({});
            }

            match args.client {
                ClientType::HermesAgent => {
                    let servers = existing
                        .as_object_mut()
                        .unwrap()
                        .entry("mcp_servers")
                        .or_insert_with(|| json!({}));
                    if let Value::Object(m) = servers {
                        m.insert("medplum".to_string(), server_entry);
                    }
                }
                ClientType::CodexCli => {
                    let tools = existing
                        .as_object_mut()
                        .unwrap()
                        .entry("tools")
                        .or_insert_with(|| json!({}));
                    if let Value::Object(t) = tools {
                        let mcp_list = t.entry("mcp").or_insert_with(|| json!([]));
                        if let Value::Array(arr) = mcp_list {
                            let mut item = server_entry;
                            if let Value::Object(ref mut map) = item {
                                map.insert("name".to_string(), json!("medplum"));
                            }
                            if let Some(pos) = arr
                                .iter()
                                .position(|x| x.get("name") == Some(&json!("medplum")))
                            {
                                arr[pos] = item;
                            } else {
                                arr.push(item);
                            }
                        }
                    }
                }
                _ => {}
            }
            let updated = serde_yaml::to_string(&existing)?;
            fs::write(&target_path, updated)?;
        } else {
            let mut existing: Value = serde_json::from_str(&content).unwrap_or_else(|_| json!({}));
            if !existing.is_object() {
                existing = json!({});
            }
            let servers = existing
                .as_object_mut()
                .unwrap()
                .entry("mcpServers")
                .or_insert_with(|| json!({}));
            if let Value::Object(m) = servers {
                m.insert("medplum".to_string(), server_entry);
            }
            let updated = serde_json::to_string_pretty(&existing)? + "\n";
            fs::write(&target_path, updated)?;
        }
    } else {
        let content = generate_client_config(args)?;
        fs::write(&target_path, content)?;
    }

    Ok(target_path)
}
