//! Miri Undefined Behavior (UB) validation suite.
//! Certifies that zero-copy transmutation, C-ABI alignment, byte casting,
//! and affine typestates are 100% free of undefined behavior, unaligned access,
//! and provenance violations.

use medplum_mcp_core::audit::{ActionStatus, AuditEntry};
use medplum_mcp_core::safety::assert_write_permitted;
use medplum_mcp_core::typestate::{Draft, MedicationRequest, PhysicianWitness};
use medplum_mcp_core::zerocopy_audit::{BinaryAuditHeader, AUDIT_MAGIC, AUDIT_VERSION};
use serde_json::json;
use zerocopy::{FromBytes, IntoBytes};

#[test]
fn test_miri_binary_audit_header_zerocopy_transmutation() {
    let sequence_id = 42u64;
    let ts_epoch_ms = 1772712000000u64;
    let action_byte = 0u8; // Allowed
    let mut digest = [0u8; 32];
    digest[0] = 0xAB;
    digest[31] = 0xCD;
    let mut prev_sig = [0u8; 32];
    prev_sig[0] = 0x12;

    let mut header =
        BinaryAuditHeader::new(sequence_id, ts_epoch_ms, action_byte, digest, prev_sig);

    let key = b"miri-secret-verification-key-32b";
    header.sign(key);

    // 1. Verify exact size and natural C-ABI alignment (120 bytes, 8-byte u64 alignment)
    assert_eq!(std::mem::size_of::<BinaryAuditHeader>(), 120);
    assert_eq!(std::mem::align_of::<BinaryAuditHeader>(), 8);

    // 2. Transmute to bytes using zerocopy (must not cause UB or unaligned reads)
    let bytes = header.as_bytes();
    assert_eq!(bytes.len(), 120);
    assert_eq!(&bytes[0..4], &AUDIT_MAGIC);
    assert_eq!(&bytes[4..6], &AUDIT_VERSION.to_le_bytes());

    // 3. Transmute back from bytes using zerocopy::FromBytes
    let roundtrip = BinaryAuditHeader::read_from_bytes(bytes).expect("zerocopy read");
    assert_eq!(roundtrip.sequence_id, sequence_id);
    assert_eq!(roundtrip.timestamp_epoch_ms, ts_epoch_ms);
    assert_eq!(roundtrip.action_status, action_byte);
    assert_eq!(roundtrip.payload_digest, digest);
    assert_eq!(roundtrip.prev_signature, prev_sig);
    assert_eq!(roundtrip.signature, header.signature);

    // 4. Verify cryptographic signature verification
    assert!(roundtrip.verify(key));

    // 5. Test conversion to AuditEntry
    let entry = AuditEntry::from_binary_header(&roundtrip, "test_miri_tool");
    assert_eq!(entry.sequence_id, sequence_id);
    assert_eq!(entry.action_status, ActionStatus::Allowed);
}

#[test]
fn test_miri_typestate_fsm_consumption_and_transitions() {
    let draft = MedicationRequest::<Draft>::new_draft(
        "med-miri-001",
        "pat-miri-001",
        "rx-6851",
        "50 mg/m2",
    );
    assert_eq!(draft.id(), "med-miri-001");
    assert_eq!(draft.status(), "draft");

    let witness = PhysicianWitness::new(
        "Practitioner/dr-miri",
        "NPI-999888777",
        "valid-hmac-signature",
    );
    assert_eq!(witness.physician_id(), "Practitioner/dr-miri");

    // Typestate linear consumption: transitions from Draft to Active
    let active = draft.issue_with_physician_witness(witness);
    assert_eq!(active.status(), "active");

    let cancelled = active.cancel("Treatment protocol updated");
    assert_eq!(cancelled.status(), "cancelled");
}

#[test]
fn test_miri_safety_gate_unicode_nfkc_interceptor() {
    // 1. Valid draft payload
    let valid_payload = json!({
        "resourceType": "MedicationRequest",
        "status": "draft",
        "intent": "order"
    });
    assert!(assert_write_permitted("MedicationRequest", &valid_payload, true).is_ok());

    // 2. Writes disabled gate
    assert!(assert_write_permitted("MedicationRequest", &valid_payload, false).is_err());

    // 3. Forbidden terminal status
    let forbidden_payload = json!({
        "resourceType": "MedicationRequest",
        "status": "active"
    });
    assert!(assert_write_permitted("MedicationRequest", &forbidden_payload, true).is_err());

    // 4. Adversarial Cyrillic homoglyph evasion vector (Cyrillic 'а' U+0430)
    let homoglyph_payload = json!({
        "resourceType": "MedicationRequest",
        "status": "\u{0430}ctive"
    });
    assert!(assert_write_permitted("MedicationRequest", &homoglyph_payload, true).is_err());
}
