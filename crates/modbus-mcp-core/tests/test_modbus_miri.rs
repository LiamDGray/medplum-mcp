//! Miri and Memory Safety Verification for `modbus-mcp-core`.
//!
//! Verifies:
//! 1. `BinaryAuditHeader` 128-byte fixed C-ABI layout, zero padding bytes, and 8-byte alignment.
//! 2. Exact field offsets matching industrial flight recording specifications.
//! 3. Zero-copy transmutation safety, pointer provenance, absence of undefined behavior.

use modbus_mcp_core::audit::{BinaryAuditHeader, MachineEventKind, AUDIT_MAGIC, AUDIT_VERSION};
use zerocopy::IntoBytes;

#[test]
fn test_binary_audit_header_layout_and_offsets() {
    use std::mem::{align_of, offset_of, size_of};

    // 1. Strict size and alignment invariants
    assert_eq!(
        size_of::<BinaryAuditHeader>(),
        128,
        "BinaryAuditHeader must be exactly 128 bytes"
    );
    assert_eq!(
        align_of::<BinaryAuditHeader>(),
        8,
        "BinaryAuditHeader must have 8-byte alignment due to u64 fields"
    );

    // 2. Strict field offset assertions (Zero uninitialized padding holes)
    assert_eq!(offset_of!(BinaryAuditHeader, magic), 0);
    assert_eq!(offset_of!(BinaryAuditHeader, version), 4);
    assert_eq!(offset_of!(BinaryAuditHeader, event_kind), 6);
    assert_eq!(offset_of!(BinaryAuditHeader, status), 7);
    assert_eq!(offset_of!(BinaryAuditHeader, unit_id), 8);
    assert_eq!(offset_of!(BinaryAuditHeader, reserved), 9);
    assert_eq!(offset_of!(BinaryAuditHeader, target_address), 12);
    assert_eq!(offset_of!(BinaryAuditHeader, raw_value), 14);
    assert_eq!(offset_of!(BinaryAuditHeader, sequence_id), 16);
    assert_eq!(offset_of!(BinaryAuditHeader, timestamp_epoch_ms), 24);
    assert_eq!(offset_of!(BinaryAuditHeader, payload_digest), 32);
    assert_eq!(offset_of!(BinaryAuditHeader, prev_signature), 64);
    assert_eq!(offset_of!(BinaryAuditHeader, signature), 96);
}

#[test]
fn test_zerocopy_transmutation_provenance_and_roundtrip() {
    let mut header = BinaryAuditHeader::new(
        MachineEventKind::WriteExecuted,
        0x01,
        0x05,
        0x1020,
        0x03E8,
        42,
        1700000000000,
        [0xAA; 32],
        [0xBB; 32],
    );
    header.signature = [0xCC; 32];

    // Transmute to bytes zero-copy
    let bytes: &[u8] = header.as_bytes();
    assert_eq!(bytes.len(), 128);

    // Pointer provenance validation: ensure address of byte slice matches struct address
    let struct_ptr = &header as *const BinaryAuditHeader as *const u8;
    let slice_ptr = bytes.as_ptr();
    assert_eq!(
        struct_ptr, slice_ptr,
        "Zero-copy slice pointer must match struct address"
    );

    // Reconstitute from raw bytes zero-copy
    let reconstructed = BinaryAuditHeader::from_bytes_zero_copy(bytes)
        .expect("zero-copy transmutation must succeed");
    assert_eq!(reconstructed, header);

    // Inspect individual transmuted byte fields at known offsets
    assert_eq!(&bytes[0..4], &AUDIT_MAGIC);
    assert_eq!(bytes[4..6], AUDIT_VERSION.to_ne_bytes());
    assert_eq!(bytes[6], MachineEventKind::WriteExecuted.to_u8());
    assert_eq!(bytes[7], 0x01);
    assert_eq!(bytes[8], 0x05);
    assert_eq!(&bytes[9..12], &[0, 0, 0]);
    assert_eq!(bytes[12..14], 0x1020u16.to_ne_bytes());
    assert_eq!(bytes[14..16], 0x03E8u16.to_ne_bytes());
    assert_eq!(bytes[16..24], 42u64.to_ne_bytes());
    assert_eq!(bytes[24..32], 1700000000000u64.to_ne_bytes());
    assert_eq!(&bytes[32..64], &[0xAA; 32]);
    assert_eq!(&bytes[64..96], &[0xBB; 32]);
    assert_eq!(&bytes[96..128], &[0xCC; 32]);
}

#[test]
fn test_zerocopy_unaligned_and_truncated_slice_safety() {
    // 1. Buffer too short: must return error without out-of-bounds read or panic
    let short_buf = vec![0u8; 127];
    let err = BinaryAuditHeader::from_bytes_zero_copy(&short_buf);
    assert!(err.is_err(), "Buffers < 128 bytes must be rejected");

    // 2. Buffer with invalid magic must be rejected safely
    let mut invalid_magic_buf = vec![0u8; 128];
    invalid_magic_buf[0..4].copy_from_slice(b"NOPE");
    let err = BinaryAuditHeader::from_bytes_zero_copy(&invalid_magic_buf);
    assert!(err.is_err(), "Invalid magic must be safely rejected");

    // 3. Unaligned buffer safety: offset into a byte array
    let mut unaligned_backing = vec![0u8; 256];
    // Write valid header at an odd byte offset (e.g. offset 3)
    let odd_offset = 3;
    let valid_header = BinaryAuditHeader::new(
        MachineEventKind::ReadRegisters,
        0,
        1,
        50,
        100,
        1,
        5000,
        [0x11; 32],
        [0x22; 32],
    );
    unaligned_backing[odd_offset..odd_offset + 128].copy_from_slice(valid_header.as_bytes());

    // Transmute from unaligned offset
    let unaligned_slice = &unaligned_backing[odd_offset..odd_offset + 128];
    let extracted = BinaryAuditHeader::from_bytes_zero_copy(unaligned_slice)
        .expect("Transmuting from unaligned byte slice must succeed without UB");
    assert_eq!(extracted.magic, AUDIT_MAGIC);
    assert_eq!(extracted.sequence_id, 1);
    assert_eq!(extracted.target_address, 50);
}
