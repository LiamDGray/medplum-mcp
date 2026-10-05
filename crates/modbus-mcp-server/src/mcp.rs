//! High-Assurance MCP 2.3+ Protocol Server for Industrial Modbus.
//!
//! Provides JSON-RPC 2.0 dispatch for Modbus discrete inputs, coils, input/holding
//! registers with token-diet distillation, affine operator safety interlocks, and emergency trip.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use modbus_mcp_core::distillation::{distill_register_map, DetailLevel};
use modbus_mcp_core::safety::OperatorWitness;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::simulator::VirtualPlcSimulator;

fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

/// Incoming JSON-RPC 2.0 request or notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

/// Outgoing JSON-RPC 2.0 response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// High-Assurance MCP Modbus Server.
#[derive(Clone)]
pub struct ModbusMcpServer {
    name: String,
    version: String,
    simulator: Arc<Mutex<VirtualPlcSimulator>>,
    #[allow(dead_code)]
    secret: Vec<u8>,
}

impl ModbusMcpServer {
    /// Creates a new Modbus MCP Server wrapping the given virtual PLC simulator.
    pub fn new(simulator: Arc<Mutex<VirtualPlcSimulator>>, secret: Vec<u8>) -> Self {
        Self {
            name: "modbus-mcp-server".to_string(),
            version: "0.1.0".to_string(),
            simulator,
            secret,
        }
    }

    /// Access the server name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Access the server version.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Access the underlying virtual PLC simulator.
    pub fn simulator(&self) -> &Arc<Mutex<VirtualPlcSimulator>> {
        &self.simulator
    }

    /// Process a raw JSON-RPC 2.0 string message and return response string if applicable.
    pub async fn handle_jsonrpc_message(&self, raw: &str) -> Option<String> {
        let req: JsonRpcRequest = match serde_json::from_str(raw) {
            Ok(r) => r,
            Err(e) => {
                let err_resp = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: None,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32700,
                        message: format!("Parse error: {}", e),
                        data: None,
                    }),
                };
                return serde_json::to_string(&err_resp).ok();
            }
        };

        let is_notification = req.id.is_none();
        let resp = self.dispatch_request(req).await;

        if is_notification {
            None
        } else {
            resp.and_then(|r| serde_json::to_string(&r).ok())
        }
    }

    /// Dispatches an incoming parsed JSON-RPC request.
    pub async fn dispatch_request(&self, req: JsonRpcRequest) -> Option<JsonRpcResponse> {
        let id = req.id.clone();
        let method = req.method.as_str();
        let params = req.params.unwrap_or(Value::Null);

        match method {
            "initialize" => {
                let client_protocol_version = params
                    .get("protocolVersion")
                    .and_then(|v| v.as_str())
                    .unwrap_or("2024-11-05");
                let result = json!({
                    "protocolVersion": client_protocol_version,
                    "capabilities": {
                        "tools": { "listChanged": false }
                    },
                    "serverInfo": {
                        "name": self.name,
                        "version": self.version
                    },
                    "instructions": "Industrial IoT Modbus Bridge MCP Server with affine safety interlocks"
                });
                Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(result),
                    error: None,
                })
            }
            "notifications/initialized" => {
                if id.is_some() {
                    Some(JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: Some(json!({})),
                        error: None,
                    })
                } else {
                    None
                }
            }
            "tools/list" => {
                let tools = self.list_tools();
                Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(json!({ "tools": tools })),
                    error: None,
                })
            }
            "tools/call" => {
                let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);

                let result = self.execute_tool(tool_name, arguments).await;
                Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(result),
                    error: None,
                })
            }
            _ => Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32601,
                    message: format!("Method not found: {}", method),
                    data: None,
                }),
            }),
        }
    }

    /// List all 7 Industrial Modbus MCP tools and JSON schemas.
    pub fn list_tools(&self) -> Vec<Value> {
        vec![
            json!({
                "name": "modbus_read_discrete_inputs",
                "description": "Read one or more 1-bit discrete inputs from the Modbus controller / virtual PLC",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "address": { "type": "integer", "description": "Starting register address (0..=65535)" },
                        "count": { "type": "integer", "description": "Number of discrete inputs to read (1..=2000)" }
                    },
                    "required": ["address", "count"]
                }
            }),
            json!({
                "name": "modbus_read_coils",
                "description": "Read one or more 1-bit coils from the Modbus controller / virtual PLC",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "address": { "type": "integer", "description": "Starting coil address (0..=65535)" },
                        "count": { "type": "integer", "description": "Number of coils to read (1..=2000)" }
                    },
                    "required": ["address", "count"]
                }
            }),
            json!({
                "name": "modbus_read_input_registers",
                "description": "Read 16-bit input registers with token-diet distillation (detail levels: raw, standard, compact)",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "address": { "type": "integer", "description": "Starting register address (0..=65535)" },
                        "count": { "type": "integer", "description": "Number of input registers to read (1..=125)" },
                        "detail_level": {
                            "type": "string",
                            "enum": ["raw", "standard", "compact"],
                            "description": "Fidelity level for token diet reduction ('raw', 'standard', 'compact')"
                        }
                    },
                    "required": ["address", "count"]
                }
            }),
            json!({
                "name": "modbus_read_holding_registers",
                "description": "Read 16-bit holding registers from the Modbus controller / virtual PLC",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "address": { "type": "integer", "description": "Starting holding register address (0..=65535)" },
                        "count": { "type": "integer", "description": "Number of holding registers to read (1..=125)" }
                    },
                    "required": ["address", "count"]
                }
            }),
            json!({
                "name": "modbus_write_coil_interlocked",
                "description": "Execute a safety-critical coil write protected by cryptographic OperatorWitness interlock",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "address": { "type": "integer", "description": "Target coil address (0..=65535)" },
                        "value": { "type": "boolean", "description": "Target boolean value to write" },
                        "witness": {
                            "type": "object",
                            "description": "Cryptographically signed supervisor witness token"
                        }
                    },
                    "required": ["address", "value"]
                }
            }),
            json!({
                "name": "modbus_write_holding_register_interlocked",
                "description": "Execute a safety-critical holding register write protected by cryptographic OperatorWitness interlock",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "address": { "type": "integer", "description": "Target holding register address (0..=65535)" },
                        "value": { "type": "integer", "description": "Target 16-bit unsigned integer value to write" },
                        "witness": {
                            "type": "object",
                            "description": "Cryptographically signed supervisor witness token"
                        }
                    },
                    "required": ["address", "value"]
                }
            }),
            json!({
                "name": "modbus_emergency_stop",
                "description": "Trigger an immediate emergency stop (halts all pumps and trips safety interlock coil)",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "reason": { "type": "string", "description": "Optional hazard reason for tripping emergency stop" }
                    }
                }
            }),
        ]
    }

    /// Executes an individual Modbus tool call.
    async fn execute_tool(&self, name: &str, arguments: Value) -> Value {
        match name {
            "modbus_read_discrete_inputs" => {
                let address = arguments
                    .get("address")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u16;
                let count = arguments.get("count").and_then(|v| v.as_u64()).unwrap_or(1) as u16;

                let plc = self.simulator.lock().await;
                match plc.read_discrete_inputs(address, count) {
                    Ok(vals) => json!({
                        "content": [{ "type": "text", "text": serde_json::to_string(&vals).unwrap_or_default() }],
                        "isError": false
                    }),
                    Err(e) => json!({
                        "content": [{ "type": "text", "text": format!("Error reading discrete inputs: {e}") }],
                        "isError": true
                    }),
                }
            }
            "modbus_read_coils" => {
                let address = arguments
                    .get("address")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u16;
                let count = arguments.get("count").and_then(|v| v.as_u64()).unwrap_or(1) as u16;

                let plc = self.simulator.lock().await;
                match plc.read_coils(address, count) {
                    Ok(vals) => json!({
                        "content": [{ "type": "text", "text": serde_json::to_string(&vals).unwrap_or_default() }],
                        "isError": false
                    }),
                    Err(e) => json!({
                        "content": [{ "type": "text", "text": format!("Error reading coils: {e}") }],
                        "isError": true
                    }),
                }
            }
            "modbus_read_input_registers" => {
                let address = arguments
                    .get("address")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u16;
                let count = arguments.get("count").and_then(|v| v.as_u64()).unwrap_or(1) as u16;
                let detail_level_opt = arguments.get("detail_level").and_then(|v| v.as_str());

                let plc = self.simulator.lock().await;

                if let Some(level_str) = detail_level_opt {
                    let level = match level_str {
                        "raw" => DetailLevel::Raw,
                        "compact" => DetailLevel::Compact,
                        _ => DetailLevel::Standard,
                    };
                    match distill_register_map(plc.bank(), plc.tags(), level) {
                        Ok(distilled) => json!({
                            "content": [{ "type": "text", "text": distilled }],
                            "isError": false
                        }),
                        Err(e) => json!({
                            "content": [{ "type": "text", "text": format!("Distillation error: {e}") }],
                            "isError": true
                        }),
                    }
                } else {
                    match plc.read_input_registers(address, count) {
                        Ok(vals) => json!({
                            "content": [{ "type": "text", "text": serde_json::to_string(&vals).unwrap_or_default() }],
                            "isError": false
                        }),
                        Err(e) => json!({
                            "content": [{ "type": "text", "text": format!("Error reading input registers: {e}") }],
                            "isError": true
                        }),
                    }
                }
            }
            "modbus_read_holding_registers" => {
                let address = arguments
                    .get("address")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u16;
                let count = arguments.get("count").and_then(|v| v.as_u64()).unwrap_or(1) as u16;

                let plc = self.simulator.lock().await;
                match plc.read_holding_registers(address, count) {
                    Ok(vals) => json!({
                        "content": [{ "type": "text", "text": serde_json::to_string(&vals).unwrap_or_default() }],
                        "isError": false
                    }),
                    Err(e) => json!({
                        "content": [{ "type": "text", "text": format!("Error reading holding registers: {e}") }],
                        "isError": true
                    }),
                }
            }
            "modbus_write_coil_interlocked" => {
                let address = arguments
                    .get("address")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u16;
                let value = arguments
                    .get("value")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let witness: Option<OperatorWitness> = arguments
                    .get("witness")
                    .and_then(|w| serde_json::from_value(w.clone()).ok());

                let now = current_time_ms();
                let mut plc = self.simulator.lock().await;

                match plc.write_coil_interlocked(address, value, witness.as_ref(), now) {
                    Ok(()) => json!({
                        "content": [{ "type": "text", "text": format!("Successfully actuated coil {address} to {value}") }],
                        "isError": false
                    }),
                    Err(e) => json!({
                        "content": [{ "type": "text", "text": format!("Interlock blocked write: {e}") }],
                        "isError": true
                    }),
                }
            }
            "modbus_write_holding_register_interlocked" => {
                let address = arguments
                    .get("address")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u16;
                let value = arguments.get("value").and_then(|v| v.as_u64()).unwrap_or(0) as u16;
                let witness: Option<OperatorWitness> = arguments
                    .get("witness")
                    .and_then(|w| serde_json::from_value(w.clone()).ok());

                let now = current_time_ms();
                let mut plc = self.simulator.lock().await;

                match plc.write_holding_register_interlocked(address, value, witness.as_ref(), now)
                {
                    Ok(()) => json!({
                        "content": [{ "type": "text", "text": format!("Successfully wrote holding register {address} to {value}") }],
                        "isError": false
                    }),
                    Err(e) => json!({
                        "content": [{ "type": "text", "text": format!("Interlock blocked write: {e}") }],
                        "isError": true
                    }),
                }
            }
            "modbus_emergency_stop" => {
                let reason = arguments.get("reason").and_then(|v| v.as_str());
                let mut plc = self.simulator.lock().await;
                plc.emergency_stop(reason);

                json!({
                    "content": [{ "type": "text", "text": "Emergency stop executed: safety coil tripped and all pumps halted" }],
                    "isError": false
                })
            }
            _ => json!({
                "content": [{ "type": "text", "text": format!("Unknown tool: {name}") }],
                "isError": true
            }),
        }
    }

    /// Converts this server into an Axum HTTP router providing Streamable HTTP / SSE transport.
    pub fn into_router(self) -> axum::Router {
        crate::streaming::create_router(self)
    }
}
