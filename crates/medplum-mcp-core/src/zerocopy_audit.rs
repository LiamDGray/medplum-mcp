//! Zero-Copy Binary Audit Frame Engine
//!
//! Provides fixed-layout C-ABI binary frames for HIPAA 45 CFR § 164.312 audit logging.
//! Enables safe transmutation between raw byte buffers and structured audit headers
//! with zero heap allocations, zero copying, and hardware-accelerated HMAC-SHA256 validation.

use hmac::{Hmac, Mac};
use sha2::Sha256;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

type HmacSha256 = Hmac<Sha256>;

/// Magic bytes identifying Medplum MCP Audit Frame ("MPLM").
pub const AUDIT_MAGIC: [u8; 4] = *b"MPLM";

/// Default binary audit frame version.
pub const AUDIT_VERSION: u16 = 1;

/// Fixed-layout C-ABI binary header for cryptographic flight recorder.
///
/// Layout:
/// - magic: 4 bytes (offset 0)
/// - version: 2 bytes (offset 4)
/// - action_status: 1 byte (offset 6: 0=Allowed, 1=Blocked, 2=Error)
/// - reserved: 1 byte (offset 7: zero padding for alignment)
/// - sequence_id: 8 bytes (offset 8)
/// - timestamp_epoch_ms: 8 bytes (offset 16)
/// - payload_digest: 32 bytes (offset 24)
/// - prev_signature: 32 bytes (offset 56)
/// - signature: 32 bytes (offset 88)
///
/// Total: exactly 120 bytes with zero uninitialized padding.
#[derive(FromBytes, IntoBytes, Immutable, KnownLayout, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct BinaryAuditHeader {
    pub magic: [u8; 4],
    pub version: u16,
    pub action_status: u8,
    pub reserved: u8,
    pub sequence_id: u64,
    pub timestamp_epoch_ms: u64,
    pub payload_digest: [u8; 32],
    pub prev_signature: [u8; 32],
    pub signature: [u8; 32],
}

impl BinaryAuditHeader {
    /// Construct a new binary audit header with zero allocations.
    pub fn new(
        sequence_id: u64,
        timestamp_epoch_ms: u64,
        action_status: u8,
        payload_digest: [u8; 32],
        prev_signature: [u8; 32],
    ) -> Self {
        Self {
            magic: AUDIT_MAGIC,
            version: AUDIT_VERSION,
            action_status,
            reserved: 0,
            sequence_id,
            timestamp_epoch_ms,
            payload_digest,
            prev_signature,
            signature: [0u8; 32],
        }
    }

    /// Sign this header in-place with the provided secret key using HMAC-SHA256.
    pub fn sign(&mut self, secret: &[u8]) {
        self.signature = compute_binary_header_signature(self, secret);
    }

    /// Verify HMAC-SHA256 signature in-place.
    pub fn verify(&self, secret: &[u8]) -> bool {
        verify_binary_header(self, secret)
    }
}

/// Compute HMAC-SHA256 signature for the first 88 bytes of the binary header.
pub fn compute_binary_header_signature(header: &BinaryAuditHeader, secret: &[u8]) -> [u8; 32] {
    let mut mac = match HmacSha256::new_from_slice(secret) {
        Ok(m) => m,
        Err(_) => return [0u8; 32],
    };
    // Sign first 88 bytes (magic through prev_signature, omitting the signature field itself)
    mac.update(&header.as_bytes()[..88]);
    let result = mac.finalize().into_bytes();
    let mut sig = [0u8; 32];
    sig.copy_from_slice(&result[..32]);
    sig
}

/// Verify HMAC-SHA256 signature of a binary header in constant time.
pub fn verify_binary_header(header: &BinaryAuditHeader, secret: &[u8]) -> bool {
    if header.magic != AUDIT_MAGIC || header.version != AUDIT_VERSION {
        return false;
    }
    let mut mac = match HmacSha256::new_from_slice(secret) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(&header.as_bytes()[..88]);
    mac.verify_slice(&header.signature).is_ok()
}
