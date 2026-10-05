//! Cryptographic machine event flight recorder verifier.
//!
//! Validates fixed-layout 128-byte C-ABI binary audit headers, verifying:
//! 1. Strictly sequential sequence counters (1..N).
//! 2. Zero-gap HMAC-SHA256 prev_signature hash chains.
//! 3. Cryptographic signature authenticity guarding against register alterations or tampering.

use std::fs;
use std::path::Path;

use modbus_mcp_core::audit::{BinaryAuditHeader, AUDIT_MAGIC};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::AuditVerifyArgs;

#[derive(Error, Debug)]
pub enum VerifierError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Verification error: {0}")]
    Verification(String),
}

/// Verification report detailing the outcome of audit flight log inspection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditVerificationReport {
    pub is_valid: bool,
    pub verified_count: usize,
    pub last_sequence_id: u64,
    pub error: Option<String>,
}

/// Verifies raw binary audit log bytes.
pub fn verify_audit_log_bytes(
    bytes: &[u8],
    secret: &[u8],
) -> Result<AuditVerificationReport, VerifierError> {
    if bytes.is_empty() {
        return Ok(AuditVerificationReport {
            is_valid: true,
            verified_count: 0,
            last_sequence_id: 0,
            error: None,
        });
    }

    if !bytes.len().is_multiple_of(128) {
        return Ok(AuditVerificationReport {
            is_valid: false,
            verified_count: 0,
            last_sequence_id: 0,
            error: Some(format!(
                "Invalid binary audit log length: {} bytes is not a multiple of 128",
                bytes.len()
            )),
        });
    }

    let mut expected_seq = 1u64;
    let mut expected_prev_sig = [0u8; 32];
    let (chunks, _) = bytes.as_chunks::<128>();
    let frame_count = chunks.len();

    for (i, chunk) in chunks.iter().enumerate() {
        let frame = match BinaryAuditHeader::from_bytes_zero_copy(chunk) {
            Ok(f) => f,
            Err(e) => {
                return Ok(AuditVerificationReport {
                    is_valid: false,
                    verified_count: i,
                    last_sequence_id: expected_seq.saturating_sub(1),
                    error: Some(format!("Frame {i} zero-copy parsing failed: {e}")),
                });
            }
        };

        if frame.magic != AUDIT_MAGIC {
            return Ok(AuditVerificationReport {
                is_valid: false,
                verified_count: i,
                last_sequence_id: expected_seq.saturating_sub(1),
                error: Some(format!("Frame {i} has invalid magic bytes")),
            });
        }

        if frame.sequence_id != expected_seq {
            return Ok(AuditVerificationReport {
                is_valid: false,
                verified_count: i,
                last_sequence_id: frame.sequence_id,
                error: Some(format!(
                    "Sequence gap detected at frame {i}: expected {expected_seq}, found {}",
                    frame.sequence_id
                )),
            });
        }

        if frame.prev_signature != expected_prev_sig {
            return Ok(AuditVerificationReport {
                is_valid: false,
                verified_count: i,
                last_sequence_id: frame.sequence_id,
                error: Some(format!(
                    "Cryptographic chain broken at frame {i}: prev_signature mismatch"
                )),
            });
        }

        if !frame.verify_signature(secret) {
            return Ok(AuditVerificationReport {
                is_valid: false,
                verified_count: i,
                last_sequence_id: frame.sequence_id,
                error: Some(format!(
                    "Tampered signature or altered register value detected at frame {i} (sequence: {})",
                    frame.sequence_id
                )),
            });
        }

        expected_prev_sig = frame.signature;
        expected_seq = expected_seq.saturating_add(1);
    }

    Ok(AuditVerificationReport {
        is_valid: true,
        verified_count: frame_count,
        last_sequence_id: expected_seq.saturating_sub(1),
        error: None,
    })
}

/// Verifies a binary audit log file on disk.
pub fn verify_audit_log_file(
    path: &Path,
    secret: &[u8],
) -> Result<AuditVerificationReport, VerifierError> {
    let data = fs::read(path)?;
    verify_audit_log_bytes(&data, secret)
}

/// Headless entrypoint for `audit-verify` CLI subcommand.
pub fn run_audit_verify(args: &AuditVerifyArgs) -> Result<AuditVerificationReport, VerifierError> {
    verify_audit_log_file(&args.log_path, args.key.as_bytes())
}
