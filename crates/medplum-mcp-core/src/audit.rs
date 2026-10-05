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
use zerocopy::{FromBytes, IntoBytes};

use crate::zerocopy_audit::{
    compute_binary_header_signature, BinaryAuditHeader, AUDIT_MAGIC, AUDIT_VERSION,
};

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

impl AuditEntry {
    /// Convert this AuditEntry into a zero-copy 120-byte C-ABI BinaryAuditHeader.
    pub fn to_binary_header(&self) -> Result<BinaryAuditHeader, AuditError> {
        let ts_epoch_ms = DateTime::parse_from_rfc3339(&self.timestamp)
            .map(|dt| dt.timestamp_millis().max(0) as u64)
            .unwrap_or(0);

        let action_status = match self.action_status {
            ActionStatus::Allowed => 0,
            ActionStatus::Blocked => 1,
            ActionStatus::Error => 2,
        };

        let mut payload_digest = [0u8; 32];
        let digest_bytes = hex::decode(&self.payload_digest)
            .map_err(|e| AuditError::InvalidKey(format!("invalid payload digest hex: {e}")))?;
        if digest_bytes.len() == 32 {
            payload_digest.copy_from_slice(&digest_bytes);
        }

        let mut prev_signature = [0u8; 32];
        let prev_sig_bytes = hex::decode(&self.prev_signature)
            .map_err(|e| AuditError::InvalidKey(format!("invalid prev_signature hex: {e}")))?;
        if prev_sig_bytes.len() == 32 {
            prev_signature.copy_from_slice(&prev_sig_bytes);
        }

        let mut signature = [0u8; 32];
        let sig_bytes = hex::decode(&self.signature)
            .map_err(|e| AuditError::InvalidKey(format!("invalid signature hex: {e}")))?;
        if sig_bytes.len() == 32 {
            signature.copy_from_slice(&sig_bytes);
        }

        Ok(BinaryAuditHeader {
            magic: AUDIT_MAGIC,
            version: AUDIT_VERSION,
            action_status,
            reserved: 0,
            sequence_id: self.sequence_id,
            timestamp_epoch_ms: ts_epoch_ms,
            payload_digest,
            prev_signature,
            signature,
        })
    }

    /// Construct an AuditEntry from a zero-copy 120-byte C-ABI BinaryAuditHeader.
    pub fn from_binary_header(header: &BinaryAuditHeader, tool_name: &str) -> Self {
        let action_status = match header.action_status {
            0 => ActionStatus::Allowed,
            1 => ActionStatus::Blocked,
            _ => ActionStatus::Error,
        };

        let naive_utc = DateTime::from_timestamp_millis(header.timestamp_epoch_ms as i64)
            .unwrap_or_else(Utc::now);
        let timestamp = naive_utc.to_rfc3339();

        Self {
            sequence_id: header.sequence_id,
            timestamp,
            action_status,
            tool_name: tool_name.to_string(),
            payload_digest: hex::encode(header.payload_digest),
            prev_signature: hex::encode(header.prev_signature),
            signature: hex::encode(header.signature),
            payload: None,
        }
    }
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
    log_path: Option<PathBuf>,
    binary_path: Option<PathBuf>,
    secret_key: Vec<u8>,
    last_sequence_id: u64,
    last_signature: String,
    last_binary_signature: [u8; 32],
}

impl AuditLogManager {
    /// Create a standard JSONL append-only audit log manager.
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
            log_path: Some(path),
            binary_path: None,
            secret_key: secret_key.to_vec(),
            last_sequence_id,
            last_signature,
            last_binary_signature: [0u8; 32],
        })
    }

    /// Create a pure zero-copy binary (.bin) audit log manager.
    pub fn new_binary(
        binary_path: impl AsRef<Path>,
        secret_key: &[u8],
    ) -> Result<Self, AuditError> {
        let path = binary_path.as_ref().to_path_buf();
        let mut last_sequence_id = 0;
        let last_signature = GENESIS_PREV_SIGNATURE.to_string();
        let mut last_binary_signature = [0u8; 32];

        if path.exists() {
            if let Ok(meta) = std::fs::metadata(&path) {
                let len = meta.len();
                if len >= 120 && len % 120 == 0 {
                    if let Ok(mut f) = File::open(&path) {
                        use std::io::{Read, Seek, SeekFrom};
                        if f.seek(SeekFrom::End(-120)).is_ok() {
                            let mut buf = [0u8; 120];
                            if f.read_exact(&mut buf).is_ok() {
                                if let Ok(hdr) = BinaryAuditHeader::read_from_bytes(&buf) {
                                    last_sequence_id = hdr.sequence_id;
                                    last_binary_signature = hdr.signature;
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(Self {
            log_path: None,
            binary_path: Some(path),
            secret_key: secret_key.to_vec(),
            last_sequence_id,
            last_signature,
            last_binary_signature,
        })
    }

    /// Create a dual-mode audit log manager writing both JSONL and zero-copy binary logs simultaneously.
    pub fn new_dual(
        jsonl_path: impl AsRef<Path>,
        binary_path: impl AsRef<Path>,
        secret_key: &[u8],
    ) -> Result<Self, AuditError> {
        let mgr = Self::new(jsonl_path, secret_key)?;
        Ok(mgr.with_binary_log(binary_path))
    }

    /// Primary JSONL log path (if configured).
    pub fn log_path(&self) -> Option<&Path> {
        self.log_path.as_deref()
    }

    /// Secondary or standalone binary log path (if configured).
    pub fn binary_path(&self) -> Option<&Path> {
        self.binary_path.as_deref()
    }

    /// Configure a secondary zero-copy binary audit log (.bin) written concurrently with .jsonl.
    pub fn with_binary_log(mut self, binary_path: impl AsRef<Path>) -> Self {
        let path = binary_path.as_ref().to_path_buf();
        if path.exists() {
            if let Ok(meta) = std::fs::metadata(&path) {
                let len = meta.len();
                if len >= 120 && len % 120 == 0 {
                    if let Ok(mut f) = File::open(&path) {
                        use std::io::{Read, Seek, SeekFrom};
                        if f.seek(SeekFrom::End(-120)).is_ok() {
                            let mut buf = [0u8; 120];
                            if f.read_exact(&mut buf).is_ok() {
                                if let Ok(hdr) = BinaryAuditHeader::read_from_bytes(&buf) {
                                    self.last_binary_signature = hdr.signature;
                                }
                            }
                        }
                    }
                }
            }
        }
        self.binary_path = Some(path);
        self
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
            payload_digest: digest.clone(),
            prev_signature: prev_sig,
            signature: signature.clone(),
            payload: payload.cloned(),
        };

        if let Some(log_path) = &self.log_path {
            if let Some(parent) = log_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_path)?;

            let serialized = serde_json::to_string(&entry)?;
            writeln!(file, "{serialized}")?;
            file.flush()?;
        }

        if let Some(bin_path) = &self.binary_path {
            if let Some(parent) = bin_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let ts_epoch_ms = DateTime::parse_from_rfc3339(timestamp)
                .map(|dt| dt.timestamp_millis().max(0) as u64)
                .unwrap_or(0);
            let action_byte = match action_status {
                ActionStatus::Allowed => 0,
                ActionStatus::Blocked => 1,
                ActionStatus::Error => 2,
            };
            let mut digest_bytes = [0u8; 32];
            if let Ok(decoded) = hex::decode(&digest) {
                if decoded.len() == 32 {
                    digest_bytes.copy_from_slice(&decoded);
                }
            }
            let mut bin_header = BinaryAuditHeader::new(
                sequence_id,
                ts_epoch_ms,
                action_byte,
                digest_bytes,
                self.last_binary_signature,
            );
            bin_header.sign(&self.secret_key);
            self.last_binary_signature = bin_header.signature;

            let mut bin_file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(bin_path)?;
            bin_file.write_all(bin_header.as_bytes())?;
            bin_file.flush()?;
        }

        self.last_sequence_id = sequence_id;
        self.last_signature = signature;

        Ok(entry)
    }

    /// Cryptographically verify the tamper-evidence and continuity of a binary audit log.
    pub fn verify_binary_audit_log(
        path: impl AsRef<Path>,
        secret_key: &[u8],
    ) -> Result<AuditVerificationReport, AuditError> {
        verify_binary_audit_log(path, secret_key)
    }

    pub fn get_entries(&self) -> Result<Vec<AuditEntry>, AuditError> {
        if let Some(log_path) = &self.log_path {
            if !log_path.exists() {
                return Ok(Vec::new());
            }

            let file = File::open(log_path)?;
            let reader = BufReader::new(file);
            let mut entries = Vec::new();

            for line_res in reader.lines() {
                let line = line_res?;
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    entries.push(serde_json::from_str::<AuditEntry>(trimmed)?);
                }
            }

            return Ok(entries);
        }

        if let Some(bin_path) = &self.binary_path {
            if !bin_path.exists() {
                return Ok(Vec::new());
            }

            let mut file = File::open(bin_path)?;
            use std::io::Read;
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)?;
            let mut entries = Vec::new();

            for chunk in buf.as_chunks::<120>().0 {
                if let Ok(hdr) = BinaryAuditHeader::read_from_bytes(chunk) {
                    entries.push(AuditEntry::from_binary_header(&hdr, "audit_event"));
                }
            }

            return Ok(entries);
        }

        Ok(Vec::new())
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

/// Cryptographically verify the tamper-evidence and sequential continuity of a binary audit log.
pub fn verify_binary_audit_log(
    path: impl AsRef<Path>,
    secret_key: &[u8],
) -> Result<AuditVerificationReport, AuditError> {
    let p = path.as_ref();
    if !p.exists() {
        return Err(AuditError::FileNotFound(p.display().to_string()));
    }

    let file = File::open(p)?;
    let mut reader = BufReader::new(file);
    let mut verified_count = 0;
    let mut expected_seq = 1u64;
    let mut expected_prev = [0u8; 32];

    use std::io::Read;
    let mut buffer = [0u8; 120];

    loop {
        match reader.read_exact(&mut buffer) {
            Ok(()) => {
                let header = BinaryAuditHeader::read_from_bytes(&buffer).map_err(|e| {
                    AuditError::Serialization(serde::de::Error::custom(e.to_string()))
                })?;

                if header.magic != AUDIT_MAGIC || header.version != AUDIT_VERSION {
                    return Err(AuditError::MalformedJson {
                        line: verified_count + 1,
                        error: "Invalid binary header magic or version".to_string(),
                    });
                }

                if header.sequence_id != expected_seq {
                    return Err(AuditError::BrokenSequence {
                        line: verified_count + 1,
                        expected: expected_seq,
                        found: header.sequence_id,
                    });
                }

                if header.prev_signature != expected_prev {
                    return Err(AuditError::BrokenChain {
                        line: verified_count + 1,
                        sequence_id: header.sequence_id,
                        expected_prev: hex::encode(expected_prev),
                        found_prev: hex::encode(header.prev_signature),
                    });
                }

                let computed_sig = compute_binary_header_signature(&header, secret_key);
                if computed_sig != header.signature {
                    return Err(AuditError::ForgedSignature {
                        line: verified_count + 1,
                        sequence_id: header.sequence_id,
                    });
                }

                expected_prev = header.signature;
                expected_seq += 1;
                verified_count += 1;
            }
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(AuditError::Io(e)),
        }
    }

    Ok(AuditVerificationReport {
        verified_count,
        is_valid: true,
    })
}
