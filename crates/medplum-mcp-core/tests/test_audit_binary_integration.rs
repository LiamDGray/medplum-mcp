//! Integration tests for BinaryAuditHeader and AuditLogManager zero-copy binary auditing.

use medplum_mcp_core::audit::{ActionStatus, AuditEntry, AuditLogManager, GENESIS_PREV_SIGNATURE};
use medplum_mcp_core::zerocopy_audit::{
    compute_binary_header_signature, verify_binary_header, AUDIT_MAGIC,
};
use std::fs;
use tempfile::NamedTempFile;

#[test]
fn test_audit_entry_to_and_from_binary_header() {
    let entry = AuditEntry {
        sequence_id: 1,
        timestamp: "2026-10-05T12:00:00Z".to_string(),
        action_status: ActionStatus::Allowed,
        tool_name: "get_patient".to_string(),
        payload_digest: "a".repeat(64),
        prev_signature: GENESIS_PREV_SIGNATURE.to_string(),
        signature: "b".repeat(64),
        payload: None,
    };

    let bin_header = entry
        .to_binary_header()
        .expect("conversion to binary header should succeed");
    assert_eq!(bin_header.magic, AUDIT_MAGIC);
    assert_eq!(bin_header.sequence_id, 1);
    assert_eq!(bin_header.action_status, 0);

    let roundtrip = AuditEntry::from_binary_header(&bin_header, "get_patient");
    assert_eq!(roundtrip.sequence_id, entry.sequence_id);
    assert_eq!(roundtrip.action_status, entry.action_status);
    assert_eq!(roundtrip.tool_name, "get_patient");
    assert_eq!(roundtrip.payload_digest, entry.payload_digest);
    assert_eq!(roundtrip.prev_signature, entry.prev_signature);
    assert_eq!(roundtrip.signature, entry.signature);

    // Test signing and verification on a binary header copy
    let mut signed_header = bin_header;
    let secret = b"test-secret-key-32-bytes-long!!!";
    signed_header.signature = compute_binary_header_signature(&signed_header, secret);
    assert!(verify_binary_header(&signed_header, secret));
}

#[test]
fn test_audit_log_manager_dual_binary_frame_logging_and_verification() {
    let json_temp = NamedTempFile::new().unwrap();
    let bin_temp = NamedTempFile::new().unwrap();
    let secret_key = b"integration-test-secret-key-32b!";

    let mut manager = AuditLogManager::new(json_temp.path(), secret_key)
        .unwrap()
        .with_binary_log(bin_temp.path());

    // Log 3 events
    let payload1 = serde_json::json!({"action": "first"});
    let payload2 = serde_json::json!({"action": "second"});
    let payload3 = serde_json::json!({"action": "third"});

    manager
        .log_event("tool_one", ActionStatus::Allowed, Some(&payload1))
        .unwrap();
    manager
        .log_event("tool_two", ActionStatus::Blocked, Some(&payload2))
        .unwrap();
    manager
        .log_event("tool_three", ActionStatus::Error, Some(&payload3))
        .unwrap();

    // Verify binary file size: 3 headers * 120 bytes = 360 bytes
    let bin_meta = fs::metadata(bin_temp.path()).unwrap();
    assert_eq!(
        bin_meta.len(),
        360,
        "Binary audit log must contain exactly 3 * 120-byte frames"
    );

    // Verify binary audit log using zero-copy streaming
    let report = AuditLogManager::verify_binary_audit_log(bin_temp.path(), secret_key)
        .expect("binary audit log verification should succeed");
    assert_eq!(report.verified_count, 3);
    assert!(report.is_valid);
}
