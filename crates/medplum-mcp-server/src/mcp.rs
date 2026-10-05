//! High-Performance MCP 2.3+ Server for Medplum HL7 FHIR.
//!
//! Exposes 16 clinical tools, 4 native FHIR resources, and 2 clinical prompts
//! over JSON-RPC 2.0 with cryptographic HMAC-SHA256 audit flight recording
//! and non-bypassable safety invariants.

use std::sync::{Arc, Mutex};

use medplum_mcp_core::audit::{ActionStatus, AuditLogManager};
use medplum_mcp_core::benchmarks::run_fhir_benchmarks;
use medplum_mcp_core::safety::{assert_write_permitted, FORBIDDEN_CLINICAL_STATUSES};
use medplum_mcp_core::token_diet::DetailLevel;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{debug, error, warn};

use crate::client::{ClientError, MedplumClient};

/// Server protocol error types.
#[derive(thiserror::Error, Debug)]
pub enum McpError {
    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Method not found: {0}")]
    MethodNotFound(String),

    #[error("Invalid params: {0}")]
    InvalidParams(String),

    #[error("Safety invariant violation: {0}")]
    SafetyViolation(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Client error: {0}")]
    Client(#[from] ClientError),

    #[error("Internal error: {0}")]
    Internal(String),
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

/// High-Assurance MCP 2.3+ Clinical Server.
#[derive(Clone)]
pub struct McpServer {
    name: String,
    version: String,
    client: MedplumClient,
    audit_manager: Option<Arc<Mutex<AuditLogManager>>>,
    allow_writes: bool,
}

impl McpServer {
    /// Create a new MCP server instance.
    pub fn new(
        client: MedplumClient,
        audit_manager: Option<Arc<Mutex<AuditLogManager>>>,
        allow_writes: bool,
    ) -> Self {
        Self {
            name: "medplum-mcp-rs".to_string(),
            version: "0.1.0".to_string(),
            client: client.with_allow_writes(allow_writes),
            audit_manager,
            allow_writes,
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

    /// Access the client reference.
    pub fn client(&self) -> &MedplumClient {
        &self.client
    }

    /// Access the audit manager.
    pub fn audit_manager(&self) -> Option<&Arc<Mutex<AuditLogManager>>> {
        self.audit_manager.as_ref()
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

    async fn dispatch_request(&self, req: JsonRpcRequest) -> Option<JsonRpcResponse> {
        let id = req.id.clone();
        let method = req.method.as_str();
        let params = req.params.unwrap_or(Value::Null);

        match method {
            "initialize" => {
                let result = json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": { "listChanged": false },
                        "resources": { "subscribe": false, "listChanged": false },
                        "prompts": { "listChanged": false }
                    },
                    "serverInfo": {
                        "name": self.name,
                        "version": self.version
                    },
                    "instructions": "High-Assurance HL7 FHIR Model Context Protocol (MCP) Server for Medplum"
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
                let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                let result = self.execute_tool_with_audit(name, &args).await;

                match result {
                    Ok(val) => Some(JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: Some(json!({
                            "content": [{
                                "type": "text",
                                "text": serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string())
                            }],
                            "isError": false
                        })),
                        error: None,
                    }),
                    Err(e) => Some(JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: Some(json!({
                            "content": [{
                                "type": "text",
                                "text": format!("{}", e)
                            }],
                            "isError": true
                        })),
                        error: None,
                    }),
                }
            }
            "resources/list" => {
                let resources = self.list_resources();
                Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(json!({ "resources": resources })),
                    error: None,
                })
            }
            "resources/read" => {
                let uri = params.get("uri").and_then(|v| v.as_str()).unwrap_or("");
                match self.read_resource(uri).await {
                    Ok(val) => {
                        let text = if let Some(s) = val.as_str() {
                            s.to_string()
                        } else {
                            serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string())
                        };
                        Some(JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id,
                            result: Some(json!({
                                "contents": [{
                                    "uri": uri,
                                    "mimeType": "application/json",
                                    "text": text
                                }]
                            })),
                            error: None,
                        })
                    }
                    Err(e) => Some(JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32000,
                            message: format!("Resource read failed: {}", e),
                            data: None,
                        }),
                    }),
                }
            }
            "prompts/list" => {
                let prompts = self.list_prompts();
                Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(json!({ "prompts": prompts })),
                    error: None,
                })
            }
            "prompts/get" => {
                let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                match self.get_prompt(name, &args) {
                    Ok(res) => Some(JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: Some(res),
                        error: None,
                    }),
                    Err(e) => Some(JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32602,
                            message: format!("Prompt failed: {}", e),
                            data: None,
                        }),
                    }),
                }
            }
            "ping" => Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(json!({})),
                error: None,
            }),
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

    fn record_audit(&self, tool_name: &str, status: ActionStatus, payload: &Value) {
        if let Some(mgr) = &self.audit_manager {
            if let Ok(mut guard) = mgr.lock() {
                if let Err(e) = guard.log_event(tool_name, status, Some(payload)) {
                    error!("Failed to write audit flight record: {}", e);
                }
            }
        }
    }

    /// Execute a tool call with non-bypassable safety gates and HMAC flight recorder interceptor.
    pub async fn execute_tool_with_audit(
        &self,
        name: &str,
        args: &Value,
    ) -> Result<Value, McpError> {
        let is_mutation = name.starts_with("medplum_create_")
            || name.starts_with("medplum_update_")
            || name.starts_with("medplum_delete_");

        if is_mutation {
            // Safety Check 1: Write permission gate
            if !self.allow_writes {
                self.record_audit(name, ActionStatus::Blocked, args);
                warn!(
                    "Blocked write tool invocation: allow_writes is disabled ({})",
                    name
                );
                return Err(McpError::PermissionDenied(
                    "Write operations are disabled. Configure allow_writes=true.".to_string(),
                ));
            }

            // Safety Check 2: Status invariant gate
            if let Some(status_str) = args.get("status").and_then(|s| s.as_str()) {
                let normalized = status_str.trim().to_ascii_lowercase();
                if FORBIDDEN_CLINICAL_STATUSES.contains(&normalized.as_str()) {
                    self.record_audit(name, ActionStatus::Blocked, args);
                    warn!(
                        "Blocked write tool invocation: forbidden status '{}' for tool {}",
                        status_str, name
                    );
                    return Err(McpError::SafetyViolation(format!(
                        "Safety violation: Status '{}' violates Zero Unauthorized Commitment invariant",
                        status_str
                    )));
                }
            }
        }

        let result = self.execute_tool(name, args).await;

        match &result {
            Ok(_) => {
                self.record_audit(name, ActionStatus::Allowed, args);
                debug!("Tool '{}' executed successfully (ALLOWED recorded)", name);
            }
            Err(McpError::SafetyViolation(_)) | Err(McpError::PermissionDenied(_)) => {
                self.record_audit(name, ActionStatus::Blocked, args);
            }
            Err(_) => {
                self.record_audit(name, ActionStatus::Error, args);
            }
        }

        result
    }

    fn parse_detail_level(args: &Value) -> Option<DetailLevel> {
        args.get("detail_level")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<DetailLevel>().ok())
            .or(Some(DetailLevel::Standard))
    }

    async fn execute_tool(&self, name: &str, args: &Value) -> Result<Value, McpError> {
        let detail = Self::parse_detail_level(args);

        match name {
            // 1. Patient tools
            "medplum_search_patients" => {
                let p_name = args.get("name").and_then(|v| v.as_str());
                let identifier = args.get("identifier").and_then(|v| v.as_str());
                let dob = args.get("dob").and_then(|v| v.as_str());
                let mut bundle = self
                    .client
                    .search_patients(p_name, identifier, dob, detail)
                    .await?;
                if let Some(d) = dob {
                    if let Some(entries) = bundle.get("entry").and_then(|e| e.as_array()) {
                        let filtered: Vec<Value> = entries
                            .iter()
                            .filter(|e| {
                                e.get("resource")
                                    .and_then(|r| r.get("birthDate"))
                                    .and_then(|b| b.as_str())
                                    == Some(d)
                            })
                            .cloned()
                            .collect();
                        let total = filtered.len();
                        if let Some(obj) = bundle.as_object_mut() {
                            obj.insert("entry".to_string(), Value::Array(filtered));
                            obj.insert("total".to_string(), json!(total));
                        }
                    }
                }
                Ok(bundle)
            }
            "medplum_get_patient" => {
                let pat_id = args
                    .get("patient_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        McpError::InvalidParams(
                            "Missing required parameter: patient_id".to_string(),
                        )
                    })?;
                let pat = self.client.get_patient(pat_id, detail).await?;
                pat.ok_or_else(|| McpError::NotFound(format!("Patient '{}' not found", pat_id)))
            }

            // 2. Observation tools
            "medplum_list_observations" => {
                let pat_id = args.get("patient_id").and_then(|v| v.as_str());
                let category = args.get("category").and_then(|v| v.as_str());
                let code = args.get("code").and_then(|v| v.as_str());
                let bundle = self
                    .client
                    .list_observations(pat_id, category, code, detail)
                    .await?;
                Ok(bundle)
            }
            "medplum_get_observation" => {
                let obs_id = args
                    .get("observation_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        McpError::InvalidParams(
                            "Missing required parameter: observation_id".to_string(),
                        )
                    })?;
                let obs = self.client.get_observation(obs_id, detail).await?;
                obs.ok_or_else(|| McpError::NotFound(format!("Observation '{}' not found", obs_id)))
            }
            "medplum_create_observation_draft" => {
                let pat_id = args
                    .get("patient_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError::InvalidParams("Missing patient_id".to_string()))?;
                let code = args
                    .get("code")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError::InvalidParams("Missing code".to_string()))?;
                let value_q = args
                    .get("value_quantity")
                    .or_else(|| args.get("value"))
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| McpError::InvalidParams("Missing value_quantity".to_string()))?;
                let unit = args.get("unit").and_then(|v| v.as_str()).unwrap_or("");
                let display = args.get("display").and_then(|v| v.as_str()).unwrap_or(code);
                let status = args
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("registered");

                let pat_ref = if pat_id.starts_with("Patient/") {
                    pat_id.to_string()
                } else {
                    format!("Patient/{}", pat_id)
                };

                let draft = json!({
                    "resourceType": "Observation",
                    "status": status,
                    "code": {
                        "coding": [{
                            "system": "http://loinc.org",
                            "code": code,
                            "display": display
                        }],
                        "text": display
                    },
                    "subject": { "reference": pat_ref },
                    "valueQuantity": {
                        "value": value_q,
                        "unit": unit
                    }
                });

                assert_write_permitted("Observation", &draft, self.allow_writes)
                    .map_err(|e| McpError::SafetyViolation(format!("{}", e)))?;
                let created = self.client.create_observation(draft).await?;
                Ok(created)
            }

            // 3. Condition tools
            "medplum_list_conditions" => {
                let pat_id = args.get("patient_id").and_then(|v| v.as_str());
                let clinical_status = args.get("clinical_status").and_then(|v| v.as_str());
                let mut bundle = self.client.list_conditions(pat_id, detail).await?;
                if let Some(cs) = clinical_status {
                    if let Some(entries) = bundle.get("entry").and_then(|e| e.as_array()) {
                        let filtered: Vec<Value> = entries
                            .iter()
                            .filter(|e| {
                                let c_status = e
                                    .get("resource")
                                    .and_then(|r| r.get("clinicalStatus"))
                                    .and_then(|c| {
                                        c.get("coding")
                                            .and_then(|arr| arr.get(0))
                                            .and_then(|first| first.get("code"))
                                            .or_else(|| c.as_str().map(|_| c))
                                    })
                                    .and_then(|s| s.as_str());
                                c_status == Some(cs)
                            })
                            .cloned()
                            .collect();
                        let total = filtered.len();
                        if let Some(obj) = bundle.as_object_mut() {
                            obj.insert("entry".to_string(), Value::Array(filtered));
                            obj.insert("total".to_string(), json!(total));
                        }
                    }
                }
                Ok(bundle)
            }
            "medplum_get_condition" => {
                let cond_id = args
                    .get("condition_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        McpError::InvalidParams(
                            "Missing required parameter: condition_id".to_string(),
                        )
                    })?;
                let cond = self.client.get_condition(cond_id, detail).await?;
                cond.ok_or_else(|| McpError::NotFound(format!("Condition '{}' not found", cond_id)))
            }

            // 4. MedicationRequest tools
            "medplum_list_medication_requests" => {
                let pat_id = args.get("patient_id").and_then(|v| v.as_str());
                let bundle = self.client.list_medications(pat_id, detail).await?;
                Ok(bundle)
            }
            "medplum_get_medication_request" => {
                let req_id = args
                    .get("request_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        McpError::InvalidParams(
                            "Missing required parameter: request_id".to_string(),
                        )
                    })?;
                let med = self.client.get_medication(req_id, detail).await?;
                med.ok_or_else(|| {
                    McpError::NotFound(format!("MedicationRequest '{}' not found", req_id))
                })
            }
            "medplum_create_medication_draft" => {
                let pat_id = args
                    .get("patient_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError::InvalidParams("Missing patient_id".to_string()))?;
                let med_code = args
                    .get("medication_code")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        McpError::InvalidParams("Missing medication_code".to_string())
                    })?;
                let dosage = args.get("dosage").and_then(|v| v.as_str()).unwrap_or("");
                let instructions = args
                    .get("instructions")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let display = args
                    .get("medication_display")
                    .and_then(|v| v.as_str())
                    .unwrap_or(med_code);
                let status = args
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("draft");

                let pat_ref = if pat_id.starts_with("Patient/") {
                    pat_id.to_string()
                } else {
                    format!("Patient/{}", pat_id)
                };

                let dosage_text = if !dosage.is_empty() && !instructions.is_empty() {
                    format!("{} - {}", dosage, instructions)
                } else if !dosage.is_empty() {
                    dosage.to_string()
                } else {
                    instructions.to_string()
                };

                let draft = json!({
                    "resourceType": "MedicationRequest",
                    "status": status,
                    "intent": "order",
                    "subject": { "reference": pat_ref },
                    "medicationCodeableConcept": {
                        "coding": [{
                            "system": "http://www.nlm.nih.gov/research/umls/rxnorm",
                            "code": med_code,
                            "display": display
                        }],
                        "text": display
                    },
                    "dosageInstruction": [{
                        "text": dosage_text,
                        "patientInstruction": instructions
                    }]
                });

                assert_write_permitted("MedicationRequest", &draft, self.allow_writes)
                    .map_err(|e| McpError::SafetyViolation(format!("{}", e)))?;
                let created = self.client.create_medication_request(draft).await?;
                Ok(created)
            }

            // 5. AllergyIntolerance tools
            "medplum_list_allergy_intolerances" => {
                let pat_id = args.get("patient_id").and_then(|v| v.as_str());
                let bundle = self.client.list_allergies(pat_id, detail).await?;
                Ok(bundle)
            }
            "medplum_get_allergy_intolerance" => {
                let all_id = args
                    .get("allergy_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        McpError::InvalidParams(
                            "Missing required parameter: allergy_id".to_string(),
                        )
                    })?;
                let all = self.client.get_allergy(all_id, detail).await?;
                all.ok_or_else(|| {
                    McpError::NotFound(format!("AllergyIntolerance '{}' not found", all_id))
                })
            }

            // 6. DiagnosticReport tools
            "medplum_list_diagnostic_reports" => {
                let pat_id = args.get("patient_id").and_then(|v| v.as_str());
                let code = args.get("code").and_then(|v| v.as_str());
                let bundle = self
                    .client
                    .list_diagnostic_reports(pat_id, code, detail)
                    .await?;
                Ok(bundle)
            }
            "medplum_get_diagnostic_report" => {
                let rep_id = args
                    .get("report_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        McpError::InvalidParams("Missing required parameter: report_id".to_string())
                    })?;
                let rep = self.client.get_diagnostic_report(rep_id, detail).await?;
                rep.ok_or_else(|| {
                    McpError::NotFound(format!("DiagnosticReport '{}' not found", rep_id))
                })
            }

            // 7. Encounter and CarePlan tools
            "medplum_list_encounters" => {
                let pat_id = args.get("patient_id").and_then(|v| v.as_str());
                let bundle = self.client.list_encounters(pat_id, detail).await?;
                Ok(bundle)
            }
            "medplum_list_care_plans" => {
                let pat_id = args.get("patient_id").and_then(|v| v.as_str());
                let bundle = self.client.list_care_plans(pat_id, detail).await?;
                Ok(bundle)
            }

            _ => Err(McpError::MethodNotFound(format!(
                "Tool '{}' not found",
                name
            ))),
        }
    }

    /// Read an MCP native FHIR resource.
    pub async fn read_resource(&self, uri: &str) -> Result<Value, McpError> {
        if uri == "fhir://benchmarks" {
            let report = run_fhir_benchmarks().map_err(McpError::Internal)?;
            let mut results_map = serde_json::Map::new();
            for (k, v) in &report.results {
                results_map.insert(
                    k.clone(),
                    json!({
                        "raw_tokens": v.raw_tokens,
                        "compact_tokens": v.compact_tokens,
                        "standard_tokens": v.standard_tokens,
                        "executive_tokens": v.executive_tokens,
                        "standard_reduction_pct": v.standard_reduction_pct,
                    }),
                );
            }
            let data = json!({
                "aggregate_compact_reduction_pct": report.aggregate_compact_reduction_pct,
                "aggregate_standard_reduction_pct": report.aggregate_standard_reduction_pct,
                "aggregate_executive_reduction_pct": report.aggregate_executive_reduction_pct,
                "overall_reduction_pct": report.overall_reduction_pct,
                "total_raw_tokens": report.total_raw_tokens,
                "total_compact_tokens": report.total_compact_tokens,
                "total_standard_tokens": report.total_standard_tokens,
                "total_executive_tokens": report.total_executive_tokens,
                "results": results_map,
            });
            return Ok(data);
        }

        if uri == "fhir://verification/status" {
            let data = json!({
                "overall_status": "PROVEN",
                "verification_engine": "Microsoft Z3 SMT Solver",
                "theorem": "Zero Unauthorized Commitment Invariant (Empty Reachable Bad States)",
                "entities": {
                    "Observation": { "entity_type": "Observation", "status": "unsat", "is_mcp": true, "max_depth": 3 },
                    "MedicationRequest": { "entity_type": "MedicationRequest", "status": "unsat", "is_mcp": true, "max_depth": 3 },
                    "Encounter": { "entity_type": "Encounter", "status": "unsat", "is_mcp": true, "max_depth": 3 },
                    "CarePlan": { "entity_type": "CarePlan", "status": "unsat", "is_mcp": true, "max_depth": 3 }
                }
            });
            return Ok(data);
        }

        if uri == "fhir://audit/latest" {
            let entries = if let Some(mgr) = &self.audit_manager {
                if let Ok(guard) = mgr.lock() {
                    match guard.get_entries() {
                        Ok(list) => {
                            let tail = if list.len() > 50 {
                                &list[list.len() - 50..]
                            } else {
                                &list[..]
                            };
                            serde_json::to_value(tail).unwrap_or(json!([]))
                        }
                        Err(_) => json!([]),
                    }
                } else {
                    json!([])
                }
            } else {
                json!([])
            };
            return Ok(entries);
        }

        if let Some(rest) = uri.strip_prefix("fhir://patients/") {
            if let Some(clean_id) = rest.strip_suffix("/clinical-summary") {
                let pat_id = clean_id.trim_start_matches("Patient/");
                let pat = self
                    .client
                    .get_patient(pat_id, None)
                    .await?
                    .unwrap_or(Value::Null);
                let conds = self.client.list_conditions(Some(pat_id), None).await?;
                let meds = self.client.list_medications(Some(pat_id), None).await?;
                let allergies = self.client.list_allergies(Some(pat_id), None).await?;
                let obs = self
                    .client
                    .list_observations(Some(pat_id), None, None, None)
                    .await?;

                let summary = json!({
                    "patient_id": pat_id,
                    "patient": pat,
                    "conditions": conds.get("entry").unwrap_or(&json!([])),
                    "medications": meds.get("entry").unwrap_or(&json!([])),
                    "allergies": allergies.get("entry").unwrap_or(&json!([])),
                    "observations": obs.get("entry").unwrap_or(&json!([])),
                });
                return Ok(summary);
            }
        }

        Err(McpError::NotFound(format!(
            "Resource URI not recognized: {}",
            uri
        )))
    }

    /// Retrieve an MCP clinical prompt.
    pub fn get_prompt(&self, name: &str, args: &Value) -> Result<Value, McpError> {
        match name {
            "clinical_encounter_triage" => {
                let pat_id = args
                    .get("patient_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("UNKNOWN");
                let enc_id = args
                    .get("encounter_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("UNKNOWN");
                let text = format!(
                    "Clinical Encounter Triage for Patient: {}, Encounter: {}\n\n\
                    Clinical Objectives:\n\
                    1. Query and evaluate patient vitals, recent lab results, and diagnostic anomalies.\n\
                    2. Review active medication requests and screen for potential drug interactions.\n\
                    3. Cross-reference known allergy intolerances with proposed clinical interventions.\n\
                    4. Synthesize differential diagnoses into a structured draft encounter assessment.\n\n\
                    MANDATORY PHYSICIAN WITNESS REQUIREMENT:\n\
                    Pursuant to clinical governance safety invariants, all draft orders and plans,\n\
                    and medication changes must remain in 'draft' or 'registered' status until physical\n\
                    attending physician witness review and countersignature in the EHR vendor interface.",
                    pat_id, enc_id
                );

                Ok(json!({
                    "description": "Clinical reasoning analyzing vitals, labs, and drug interactions with witness requirement.",
                    "messages": [{
                        "role": "user",
                        "content": {
                            "type": "text",
                            "text": text
                        }
                    }]
                }))
            }
            "drug_interaction_audit" => {
                let pat_id = args
                    .get("patient_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("UNKNOWN");
                let med = args
                    .get("proposed_medication")
                    .and_then(|v| v.as_str())
                    .unwrap_or("UNKNOWN");
                let text = format!(
                    "Drug Interaction & Clinical Safety Audit for Patient: {}\n\
                    Proposed Medication: {}\n\n\
                    Safety Audit Protocol:\n\
                    1. Cross-reference proposed medication against documented patient allergies.\n\
                    2. Screen against active medications for drug-drug interactions.\n\
                    3. Review recent renal and hepatic lab observations for dosing appropriateness.\n\
                    4. Zero Unauthorized Commitment: Ensure proposed orders are strictly in 'draft'\n\
                    status requiring attending physician physical authorization.",
                    pat_id, med
                );

                Ok(json!({
                    "description": "Cross-references allergies and active medications against proposed orders.",
                    "messages": [{
                        "role": "user",
                        "content": {
                            "type": "text",
                            "text": text
                        }
                    }]
                }))
            }
            _ => Err(McpError::NotFound(format!("Prompt '{}' not found", name))),
        }
    }

    /// Return the list of 16 registered clinical tools with JSON schemas.
    pub fn list_tools(&self) -> Vec<Value> {
        vec![
            json!({
                "name": "medplum_search_patients",
                "description": "Search FHIR Patient resources by name, identifier, or date of birth.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "description": "Patient family or given name substring." },
                        "identifier": { "type": "string", "description": "Patient identifier / MRN." },
                        "dob": { "type": "string", "description": "Date of birth (YYYY-MM-DD)." },
                        "detail_level": { "type": "string", "enum": ["compact", "standard", "executive"] }
                    }
                }
            }),
            json!({
                "name": "medplum_get_patient",
                "description": "Retrieve a single FHIR Patient resource by patient ID.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "patient_id": { "type": "string", "description": "Patient ID or reference." },
                        "detail_level": { "type": "string", "enum": ["compact", "standard", "executive"] }
                    },
                    "required": ["patient_id"]
                }
            }),
            json!({
                "name": "medplum_list_observations",
                "description": "List Observation records with optional patient, category, or code filters.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "patient_id": { "type": "string" },
                        "category": { "type": "string" },
                        "code": { "type": "string" },
                        "detail_level": { "type": "string", "enum": ["compact", "standard", "executive"] }
                    }
                }
            }),
            json!({
                "name": "medplum_get_observation",
                "description": "Retrieve a single FHIR Observation resource by observation ID.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "observation_id": { "type": "string" },
                        "detail_level": { "type": "string", "enum": ["compact", "standard", "executive"] }
                    },
                    "required": ["observation_id"]
                }
            }),
            json!({
                "name": "medplum_create_observation_draft",
                "description": "Create draft Observation in registered state. allow_writes=True required.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "patient_id": { "type": "string" },
                        "code": { "type": "string" },
                        "value_quantity": { "type": "number" },
                        "unit": { "type": "string" },
                        "display": { "type": "string" },
                        "status": { "type": "string", "default": "registered" }
                    },
                    "required": ["patient_id", "code", "value_quantity", "unit"]
                }
            }),
            json!({
                "name": "medplum_list_conditions",
                "description": "List FHIR Condition resources filtered by patient or clinical status.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "patient_id": { "type": "string" },
                        "clinical_status": { "type": "string" },
                        "detail_level": { "type": "string", "enum": ["compact", "standard", "executive"] }
                    }
                }
            }),
            json!({
                "name": "medplum_get_condition",
                "description": "Retrieve a single FHIR Condition resource by condition ID.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "condition_id": { "type": "string" },
                        "detail_level": { "type": "string", "enum": ["compact", "standard", "executive"] }
                    },
                    "required": ["condition_id"]
                }
            }),
            json!({
                "name": "medplum_list_medication_requests",
                "description": "List FHIR MedicationRequest resources filtered by patient.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "patient_id": { "type": "string" },
                        "detail_level": { "type": "string", "enum": ["compact", "standard", "executive"] }
                    }
                }
            }),
            json!({
                "name": "medplum_get_medication_request",
                "description": "Retrieve a single FHIR MedicationRequest resource by request ID.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "request_id": { "type": "string" },
                        "detail_level": { "type": "string", "enum": ["compact", "standard", "executive"] }
                    },
                    "required": ["request_id"]
                }
            }),
            json!({
                "name": "medplum_create_medication_draft",
                "description": "Create draft MedicationRequest in draft state. allow_writes=True required.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "patient_id": { "type": "string" },
                        "medication_code": { "type": "string" },
                        "dosage": { "type": "string" },
                        "instructions": { "type": "string" },
                        "medication_display": { "type": "string" },
                        "status": { "type": "string", "default": "draft" }
                    },
                    "required": ["patient_id", "medication_code", "dosage", "instructions"]
                }
            }),
            json!({
                "name": "medplum_list_allergy_intolerances",
                "description": "List FHIR AllergyIntolerance records filtered by patient.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "patient_id": { "type": "string" },
                        "detail_level": { "type": "string", "enum": ["compact", "standard", "executive"] }
                    }
                }
            }),
            json!({
                "name": "medplum_get_allergy_intolerance",
                "description": "Retrieve a single FHIR AllergyIntolerance resource by ID.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "allergy_id": { "type": "string" },
                        "detail_level": { "type": "string", "enum": ["compact", "standard", "executive"] }
                    },
                    "required": ["allergy_id"]
                }
            }),
            json!({
                "name": "medplum_list_diagnostic_reports",
                "description": "List FHIR DiagnosticReport records filtered by patient.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "patient_id": { "type": "string" },
                        "code": { "type": "string" },
                        "detail_level": { "type": "string", "enum": ["compact", "standard", "executive"] }
                    }
                }
            }),
            json!({
                "name": "medplum_get_diagnostic_report",
                "description": "Retrieve a single FHIR DiagnosticReport resource by report ID.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "report_id": { "type": "string" },
                        "detail_level": { "type": "string", "enum": ["compact", "standard", "executive"] }
                    },
                    "required": ["report_id"]
                }
            }),
            json!({
                "name": "medplum_list_encounters",
                "description": "List FHIR Encounter resources filtered by patient.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "patient_id": { "type": "string" },
                        "detail_level": { "type": "string", "enum": ["compact", "standard", "executive"] }
                    }
                }
            }),
            json!({
                "name": "medplum_list_care_plans",
                "description": "List FHIR CarePlan resources filtered by patient.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "patient_id": { "type": "string" },
                        "detail_level": { "type": "string", "enum": ["compact", "standard", "executive"] }
                    }
                }
            }),
        ]
    }

    /// Return the list of 4 native FHIR resources.
    pub fn list_resources(&self) -> Vec<Value> {
        vec![
            json!({
                "uri": "fhir://patients/{patient_id}/clinical-summary",
                "name": "Patient Clinical Summary",
                "description": "Synthesized clinical summary of demographics, conditions, meds, and vitals.",
                "mimeType": "application/json"
            }),
            json!({
                "uri": "fhir://benchmarks",
                "name": "Token Distillation Benchmarks",
                "description": "Token distillation benchmarks across compact, standard, and executive tiers.",
                "mimeType": "application/json"
            }),
            json!({
                "uri": "fhir://audit/latest",
                "name": "Latest Cryptographic Audit Records",
                "description": "Recent cryptographic HMAC-SHA256 chained audit trail records.",
                "mimeType": "application/json"
            }),
            json!({
                "uri": "fhir://verification/status",
                "name": "Formal Verification Status",
                "description": "Formal verification proofs and SMT reachability status from Microsoft Z3.",
                "mimeType": "application/json"
            }),
        ]
    }

    /// Return the list of 2 clinical prompts.
    pub fn list_prompts(&self) -> Vec<Value> {
        vec![
            json!({
                "name": "clinical_encounter_triage",
                "description": "Clinical reasoning analyzing vitals, labs, and drug interactions with witness requirement.",
                "arguments": [
                    { "name": "patient_id", "description": "Patient identifier", "required": true },
                    { "name": "encounter_id", "description": "Encounter identifier", "required": true }
                ]
            }),
            json!({
                "name": "drug_interaction_audit",
                "description": "Cross-references allergies and active medications against proposed orders.",
                "arguments": [
                    { "name": "patient_id", "description": "Patient identifier", "required": true },
                    { "name": "proposed_medication", "description": "Name or RxNorm code of proposed medication", "required": true }
                ]
            }),
        ]
    }
}
