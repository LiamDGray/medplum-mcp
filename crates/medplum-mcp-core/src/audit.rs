//! HIPAA 45 CFR § 164.312 Cryptographic Audit Flight Recorder.
//!
//! Implements tamper-evident HMAC-SHA256 chained audit ledger for HL7 FHIR tool invocations,
//! safety intercepts, and error outcomes under federal healthcare security standards.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

/// 64-character hex zero string representing genesis signature anchor.
pub const GENESIS_PREV_SIGNATURE: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

/// Audit action outcome status pursuant to HIPAA § 164.312(b).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ActionStatus {
    Allowed,
    Blocked,
    Error,
}

impl ActionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ActionStatus::Allowed => "ALLOWED",
            ActionStatus::Blocked => "BLOCKED",
            ActionStatus::Error => "ERROR",
        }
    }
}

impl std::fmt::Display for ActionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for ActionStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_uppercase().as_str() {
            "ALLOWED" => Ok(ActionStatus::Allowed),
            "BLOCKED" => Ok(ActionStatus::Blocked),
            "ERROR" => Ok(ActionStatus::Error),
            _ => Err(format!("Unknown ActionStatus: {s}")),
        }
    }
}

/// Tamper-evident audit log entry with cryptographic HMAC-SHA256 chaining.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditEntry {
    pub sequence_id: u64,
    pub timestamp: String,
    pub action_status: ActionStatus,
    pub tool_name: String,
    pub payload_digest: String,
    pub prev_signature: String,
    pub signature: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditVerificationReport {
    pub verified_count: usize,
    pub is_valid: bool,
}

#[derive(Debug, Error)]
pub enum AuditError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Invalid HMAC key: {0}")]
    InvalidKey(String),

    #[error("Audit log file not found: {0}")]
    FileNotFound(String),

    #[error("Malformed JSON on line {line}: {error}")]
    MalformedJson { line: usize, error: String },

    #[error("Missing required audit fields on line {line}")]
    MissingFields { line: usize },

    #[error("Sequence gap at line {line}: expected sequence_id {expected}, got {found}")]
    BrokenSequence {
        line: usize,
        expected: u64,
        found: u64,
    },

    #[error("Genesis signature invalid: expected {expected}, got {found}")]
    InvalidGenesisSignature { expected: String, found: String },

    #[error(
        "Chain break at line {line}: expected prev_signature {expected_prev}, got {found_prev}"
    )]
    BrokenChain {
        line: usize,
        sequence_id: u64,
        expected_prev: String,
        found_prev: String,
    },

    #[error("Timestamp non-monotonic at line {line}: previous {previous}, current {current}")]
    AlteredTimestamp {
        line: usize,
        sequence_id: u64,
        previous: String,
        current: String,
    },

    #[error("Payload digest mismatch at line {line} (sequence_id {sequence_id})")]
    TamperedPayload {
        line: usize,
        sequence_id: u64,
        expected: String,
        found: String,
    },

    #[error("HMAC signature mismatch at line {line} (sequence_id {sequence_id})")]
    ForgedSignature { line: usize, sequence_id: u64 },
}

/// Compute deterministic SHA-256 digest of payload.
pub fn compute_payload_digest(payload: Option<&Value>) -> String {
    let bytes = match payload {
        None | Some(Value::Null) => b"{}".to_vec(),
        Some(Value::String(s)) => s.as_bytes().to_vec(),
        Some(v) => serde_json::to_vec(v).unwrap_or_else(|_| b"{}".to_vec()),
    };
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    hex::encode(hasher.finalize())
}

/// Calculate cryptographic HMAC-SHA256 signature for canonical entry data.
pub fn compute_entry_signature(
    secret_key: &[u8],
    sequence_id: u64,
    timestamp: &str,
    status: &str,
    tool: &str,
    payload_digest: &str,
    prev_sig: &str,
) -> Result<String, AuditError> {
    let mut mac = HmacSha256::new_from_slice(secret_key)
        .map_err(|e| AuditError::InvalidKey(e.to_string()))?;
    let message = format!("{sequence_id}:{timestamp}:{status}:{tool}:{payload_digest}:{prev_sig}");
    mac.update(message.as_bytes());
    Ok(hex::encode(mac.finalize().into_bytes()))
}

/// Thread-safe, append-only cryptographic flight recorder for HIPAA audit trails.
pub struct AuditLogManager {
    log_path: PathBuf,
    secret_key: Vec<u8>,
    last_sequence_id: u64,
    last_signature: String,
}

impl AuditLogManager {
    pub fn new(log_path: impl AsRef<Path>, secret_key: &[u8]) -> Result<Self, AuditError> {
        let path = log_path.as_ref().to_path_buf();
        let mut last_sequence_id = 0;
        let mut last_signature = GENESIS_PREV_SIGNATURE.to_string();

        if path.exists() {
            let file = File::open(&path)?;
            let reader = BufReader::new(file);
            for line_res in reader.lines() {
                let line = line_res?;
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if let Ok(entry) = serde_json::from_str::<AuditEntry>(trimmed) {
                    last_sequence_id = entry.sequence_id;
                    last_signature = entry.signature;
                }
            }
        }

        Ok(Self {
            log_path: path,
            secret_key: secret_key.to_vec(),
            last_sequence_id,
            last_signature,
        })
    }

    pub fn log_event(
        &mut self,
        tool_name: &str,
        action_status: ActionStatus,
        payload: Option<&Value>,
    ) -> Result<AuditEntry, AuditError> {
        let timestamp = Utc::now().to_rfc3339();
        self.log_event_with_timestamp(tool_name, action_status, payload, &timestamp)
    }

    pub fn log_event_with_timestamp(
        &mut self,
        tool_name: &str,
        action_status: ActionStatus,
        payload: Option<&Value>,
        timestamp: &str,
    ) -> Result<AuditEntry, AuditError> {
        let sequence_id = self.last_sequence_id + 1;
        let prev_sig = self.last_signature.clone();
        let digest = compute_payload_digest(payload);

        let signature = compute_entry_signature(
            &self.secret_key,
            sequence_id,
            timestamp,
            action_status.as_str(),
            tool_name,
            &digest,
            &prev_sig,
        )?;

        let entry = AuditEntry {
            sequence_id,
            timestamp: timestamp.to_string(),
            action_status,
            tool_name: tool_name.to_string(),
            payload_digest: digest,
            prev_signature: prev_sig,
            signature: signature.clone(),
            payload: payload.cloned(),
        };

        if let Some(parent) = self.log_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)?;

        let serialized = serde_json::to_string(&entry)?;
        writeln!(file, "{serialized}")?;
        file.flush()?;

        self.last_sequence_id = sequence_id;
        self.last_signature = signature;

        Ok(entry)
    }

    pub fn get_entries(&self) -> Result<Vec<AuditEntry>, AuditError> {
        if !self.log_path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&self.log_path)?;
        let reader = BufReader::new(file);
        let mut entries = Vec::new();

        for line_res in reader.lines() {
            let line = line_res?;
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                entries.push(serde_json::from_str::<AuditEntry>(trimmed)?);
            }
        }

        Ok(entries)
    }
}

/// Cryptographically verify the tamper-evidence and sequential continuity of an audit log.
pub fn verify_audit_log(
    path: impl AsRef<Path>,
    secret_key: &[u8],
) -> Result<AuditVerificationReport, AuditError> {
    let p = path.as_ref();
    if !p.exists() {
        return Err(AuditError::FileNotFound(p.display().to_string()));
    }

    let file = File::open(p)?;
    let reader = BufReader::new(file);

    let mut expected_sequence_id = 1;
    let mut expected_prev_signature = GENESIS_PREV_SIGNATURE.to_string();
    let mut prev_timestamp: Option<DateTime<Utc>> = None;
    let mut verified_count = 0;

    for (idx, line_res) in reader.lines().enumerate() {
        let line_num = idx + 1;
        let line = line_res?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let entry: AuditEntry =
            serde_json::from_str(trimmed).map_err(|e| AuditError::MalformedJson {
                line: line_num,
                error: e.to_string(),
            })?;

        // 1. Sequence ID check
        if entry.sequence_id != expected_sequence_id {
            return Err(AuditError::BrokenSequence {
                line: line_num,
                expected: expected_sequence_id,
                found: entry.sequence_id,
            });
        }

        // 2. Prev signature check
        if entry.sequence_id == 1 {
            if entry.prev_signature != GENESIS_PREV_SIGNATURE {
                return Err(AuditError::InvalidGenesisSignature {
                    expected: GENESIS_PREV_SIGNATURE.to_string(),
                    found: entry.prev_signature,
                });
            }
        } else if entry.prev_signature != expected_prev_signature {
            return Err(AuditError::BrokenChain {
                line: line_num,
                sequence_id: entry.sequence_id,
                expected_prev: expected_prev_signature,
                found_prev: entry.prev_signature,
            });
        }

        // 3. Monotonic timestamp check
        let cur_dt = DateTime::parse_from_rfc3339(&entry.timestamp)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| AuditError::MalformedJson {
                line: line_num,
                error: format!("Invalid timestamp {}: {e}", entry.timestamp),
            })?;

        if let Some(prev_dt) = prev_timestamp {
            if cur_dt < prev_dt {
                return Err(AuditError::AlteredTimestamp {
                    line: line_num,
                    sequence_id: entry.sequence_id,
                    previous: prev_dt.to_rfc3339(),
                    current: entry.timestamp.clone(),
                });
            }
        }
        prev_timestamp = Some(cur_dt);

        // 4. Payload digest check (if payload is present)
        if let Some(ref pl) = entry.payload {
            let computed_digest = compute_payload_digest(Some(pl));
            if computed_digest != entry.payload_digest {
                return Err(AuditError::TamperedPayload {
                    line: line_num,
                    sequence_id: entry.sequence_id,
                    expected: computed_digest,
                    found: entry.payload_digest,
                });
            }
        }

        // 5. Signature verification
        let expected_sig = compute_entry_signature(
            secret_key,
            entry.sequence_id,
            &entry.timestamp,
            entry.action_status.as_str(),
            &entry.tool_name,
            &entry.payload_digest,
            &entry.prev_signature,
        )?;

        if expected_sig != entry.signature {
            return Err(AuditError::ForgedSignature {
                line: line_num,
                sequence_id: entry.sequence_id,
            });
        }

        expected_sequence_id = entry.sequence_id + 1;
        expected_prev_signature = entry.signature;
        verified_count += 1;
    }

    Ok(AuditVerificationReport {
        verified_count,
        is_valid: true,
    })
}
