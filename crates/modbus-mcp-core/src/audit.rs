//! Machine Event Audit Flight Recording with Zero-Copy Binary Headers.
//!
//! Provides fixed-layout 128-byte C-ABI binary frames for industrial flight recording.
//! Supports zero-copy transmutation between raw byte buffers and structured headers,
//! hardware-accelerated SHA-256 digests, and tamper-evident cryptographic HMAC-SHA256 chaining.

use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

use crate::registers::ModbusError;

type HmacSha256 = Hmac<Sha256>;

/// Magic bytes identifying Industrial IoT Modbus Audit Frame ("IIOT").
pub const AUDIT_MAGIC: [u8; 4] = *b"IIOT";

/// Binary audit frame version.
pub const AUDIT_VERSION: u16 = 1;

/// Event categories for machine audit logging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MachineEventKind {
    ReadRegisters = 1,
    WriteAttempt = 2,
    WriteExecuted = 3,
    InterlockTripped = 4,
    SafetyFault = 5,
}

impl MachineEventKind {
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            1 => Some(Self::ReadRegisters),
            2 => Some(Self::WriteAttempt),
            3 => Some(Self::WriteExecuted),
            4 => Some(Self::InterlockTripped),
            5 => Some(Self::SafetyFault),
            _ => None,
        }
    }
}

/// Fixed-layout 128-byte C-ABI binary audit header.
///
/// Memory layout (zero uninitialized padding bytes):
/// - magic: 4 bytes (offset 0..4)
/// - version: 2 bytes (offset 4..6)
/// - event_kind: 1 byte (offset 6..7)
/// - status: 1 byte (offset 7..8)
/// - unit_id: 1 byte (offset 8..9)
/// - reserved: 3 bytes (offset 9..12)
/// - target_address: 2 bytes (offset 12..14)
/// - raw_value: 2 bytes (offset 14..16)
/// - sequence_id: 8 bytes (offset 16..24)
/// - timestamp_epoch_ms: 8 bytes (offset 24..32)
/// - payload_digest: 32 bytes (offset 32..64)
/// - prev_signature: 32 bytes (offset 64..96)
/// - signature: 32 bytes (offset 96..128)
///
/// Total: exactly 128 bytes, 8-byte aligned.
#[derive(FromBytes, IntoBytes, Immutable, KnownLayout, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct BinaryAuditHeader {
    pub magic: [u8; 4],
    pub version: u16,
    pub event_kind: u8,
    pub status: u8,
    pub unit_id: u8,
    pub reserved: [u8; 3],
    pub target_address: u16,
    pub raw_value: u16,
    pub sequence_id: u64,
    pub timestamp_epoch_ms: u64,
    pub payload_digest: [u8; 32],
    pub prev_signature: [u8; 32],
    pub signature: [u8; 32],
}

impl BinaryAuditHeader {
    /// Constructs a new binary audit header with zero allocations.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        event_kind: MachineEventKind,
        status: u8,
        unit_id: u8,
        target_address: u16,
        raw_value: u16,
        sequence_id: u64,
        timestamp_epoch_ms: u64,
        payload_digest: [u8; 32],
        prev_signature: [u8; 32],
    ) -> Self {
        Self {
            magic: AUDIT_MAGIC,
            version: AUDIT_VERSION,
            event_kind: event_kind.to_u8(),
            status,
            unit_id,
            reserved: [0u8; 3],
            target_address,
            raw_value,
            sequence_id,
            timestamp_epoch_ms,
            payload_digest,
            prev_signature,
            signature: [0u8; 32],
        }
    }

    /// Safely transmutes a raw byte buffer into a structured `BinaryAuditHeader` zero-copy.
    pub fn from_bytes_zero_copy(bytes: &[u8]) -> Result<Self, ModbusError> {
        let (header, _): (Self, &[u8]) = Self::read_from_prefix(bytes).map_err(|e| {
            ModbusError::AuditVerificationFailed(format!("Zero-copy transmutation failed: {e:?}"))
        })?;

        if header.magic != AUDIT_MAGIC {
            return Err(ModbusError::AuditVerificationFailed(
                "Invalid audit magic header bytes".to_string(),
            ));
        }

        Ok(header)
    }

    /// Computes and signs this header in-place with HMAC-SHA256 across all header fields (offsets 0..96).
    pub fn sign(&mut self, secret: &[u8]) {
        let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC supports any key size");
        let all_bytes = self.as_bytes();
        // Sign everything before the signature field (first 96 bytes)
        mac.update(&all_bytes[0..96]);
        let sig = mac.finalize().into_bytes();
        self.signature.copy_from_slice(&sig);
    }

    /// Verifies the cryptographic HMAC-SHA256 signature in-place.
    pub fn verify_signature(&self, secret: &[u8]) -> bool {
        let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC supports any key size");
        let all_bytes = self.as_bytes();
        mac.update(&all_bytes[0..96]);
        mac.verify_slice(&self.signature).is_ok()
    }

    /// Verifies that a given payload slice matches the recorded `payload_digest`.
    pub fn verify_payload(&self, payload: &[u8]) -> bool {
        compute_payload_digest(payload) == self.payload_digest
    }
}

/// Computes a standard SHA-256 payload digest.
pub fn compute_payload_digest(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut digest = [0u8; 32];
    digest.copy_from_slice(&result);
    digest
}

/// Cryptographic machine event flight recorder.
pub struct MachineFlightRecorder {
    secret: Vec<u8>,
    sequence_id: u64,
    last_signature: [u8; 32],
    frames: Vec<BinaryAuditHeader>,
}

impl MachineFlightRecorder {
    /// Creates a new flight recorder initialized with a secret key.
    pub fn new(secret: &[u8]) -> Self {
        Self {
            secret: secret.to_vec(),
            sequence_id: 0,
            last_signature: [0u8; 32],
            frames: Vec::new(),
        }
    }

    /// Current sequence counter.
    pub fn sequence_id(&self) -> u64 {
        self.sequence_id
    }

    /// Records and cryptographically chains a machine event.
    pub fn record_event(
        &mut self,
        event_kind: MachineEventKind,
        status: u8,
        unit_id: u8,
        target_address: u16,
        raw_value: u16,
        payload_data: &[u8],
    ) -> BinaryAuditHeader {
        self.sequence_id = self.sequence_id.saturating_add(1);
        let timestamp_ms = Utc::now().timestamp_millis().max(0) as u64;
        let payload_digest = compute_payload_digest(payload_data);

        let mut header = BinaryAuditHeader::new(
            event_kind,
            status,
            unit_id,
            target_address,
            raw_value,
            self.sequence_id,
            timestamp_ms,
            payload_digest,
            self.last_signature,
        );

        header.sign(&self.secret);
        self.last_signature = header.signature;
        self.frames.push(header);
        header
    }

    /// Serializes a frame header to raw binary bytes zero-copy.
    pub fn export_frame_bytes(&self, header: &BinaryAuditHeader) -> Vec<u8> {
        header.as_bytes().to_vec()
    }

    /// Access the slice of recorded binary audit headers.
    pub fn frames(&self) -> &[BinaryAuditHeader] {
        &self.frames
    }

    /// Verifies the entire audit log sequence and cryptographic signature chain.
    pub fn verify_chain(&self) -> Result<(), ModbusError> {
        let mut expected_seq = 1u64;
        let mut expected_prev_sig = [0u8; 32];

        for (i, frame) in self.frames.iter().enumerate() {
            if frame.magic != AUDIT_MAGIC {
                return Err(ModbusError::AuditVerificationFailed(format!(
                    "Frame {i} has invalid magic"
                )));
            }

            if frame.sequence_id != expected_seq {
                return Err(ModbusError::AuditVerificationFailed(format!(
                    "Sequence gap detected at frame {i}: expected {expected_seq}, found {}",
                    frame.sequence_id
                )));
            }

            if frame.prev_signature != expected_prev_sig {
                return Err(ModbusError::AuditVerificationFailed(format!(
                    "Cryptographic chain broken at frame {i}: prev_signature mismatch"
                )));
            }

            if !frame.verify_signature(&self.secret) {
                return Err(ModbusError::AuditVerificationFailed(format!(
                    "Tampered signature detected at frame {i}"
                )));
            }

            expected_prev_sig = frame.signature;
            expected_seq = expected_seq.saturating_add(1);
        }

        Ok(())
    }

    /// Helper for testing: corrupts a byte in a recorded frame to verify tamper detection.
    pub fn corrupt_frame_for_test(&mut self, index: usize) {
        if let Some(frame) = self.frames.get_mut(index) {
            frame.raw_value = frame.raw_value.wrapping_add(1);
        }
    }

    /// Helper for testing: flips a specific bit in a frame's signature.
    pub fn flip_signature_bit_for_test(&mut self, frame_idx: usize, byte_idx: usize, bit_idx: u8) {
        if let Some(frame) = self.frames.get_mut(frame_idx) {
            if byte_idx < 32 {
                frame.signature[byte_idx] ^= 1 << (bit_idx % 8);
            }
        }
    }

    /// Helper for testing: flips a specific bit in a frame's previous signature.
    pub fn flip_prev_signature_bit_for_test(
        &mut self,
        frame_idx: usize,
        byte_idx: usize,
        bit_idx: u8,
    ) {
        if let Some(frame) = self.frames.get_mut(frame_idx) {
            if byte_idx < 32 {
                frame.prev_signature[byte_idx] ^= 1 << (bit_idx % 8);
            }
        }
    }
}
