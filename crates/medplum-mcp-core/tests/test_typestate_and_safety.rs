use medplum_mcp_core::safety::{
    assert_write_permitted, normalize_status, SafetyViolationError, FORBIDDEN_CLINICAL_STATUSES,
};
use medplum_mcp_core::secret::SecretString;
use medplum_mcp_core::typestate::{Active, Draft, MedicationRequest, PhysicianWitness};
use serde_json::json;

#[test]
fn test_typestate_draft_can_update_notes_and_dosage() {
    let mut draft: MedicationRequest<Draft> = MedicationRequest::new_draft(
        "med-001",
        "patient-123",
        "Amoxicillin 500mg",
        "500mg PO TID",
    );
    assert_eq!(draft.status(), "draft");
    assert_eq!(draft.dosage(), "500mg PO TID");

    draft.update_dosage("250mg PO TID");
    draft.update_notes("Adjusted dosage based on renal function");

    assert_eq!(draft.dosage(), "250mg PO TID");
    assert_eq!(
        draft.notes(),
        Some("Adjusted dosage based on renal function")
    );
    assert_eq!(draft.status(), "draft");
}

#[test]
fn test_typestate_transition_requires_physician_witness() {
    let draft = MedicationRequest::new_draft(
        "med-002",
        "patient-456",
        "Metformin 1000mg",
        "1000mg PO BID",
    );

    let witness =
        PhysicianWitness::new("dr-smith", "1234567890", "hmac-sha256-signature-token-xyz");

    let active: MedicationRequest<Active> = draft.issue_with_physician_witness(witness);
    assert_eq!(active.status(), "active");
    assert_eq!(active.witness().physician_id(), "dr-smith");
    assert_eq!(active.witness().npi(), "1234567890");

    let completed = active.complete();
    assert_eq!(completed.status(), "completed");
}

#[test]
fn test_assert_write_permitted_disallowed_writes() {
    let payload = json!({
        "resourceType": "MedicationRequest",
        "status": "draft"
    });

    let result = assert_write_permitted("MedicationRequest", &payload, false);
    assert!(result.is_err());
    match result.unwrap_err() {
        SafetyViolationError::PermissionDenied { resource_type } => {
            assert_eq!(resource_type, "MedicationRequest");
        }
        err => panic!("Expected PermissionDenied, got {:?}", err),
    }
}

#[test]
fn test_assert_write_permitted_allows_draft() {
    let payload = json!({
        "resourceType": "MedicationRequest",
        "status": "draft",
        "intent": "order"
    });

    let result = assert_write_permitted("MedicationRequest", &payload, true);
    assert!(result.is_ok());
}

#[test]
fn test_assert_write_permitted_blocks_all_forbidden_statuses() {
    for forbidden in FORBIDDEN_CLINICAL_STATUSES {
        let payload = json!({
            "resourceType": "Observation",
            "status": forbidden
        });

        let result = assert_write_permitted("Observation", &payload, true);
        assert!(
            result.is_err(),
            "Expected status '{}' to be blocked",
            forbidden
        );
        match result.unwrap_err() {
            SafetyViolationError::ForbiddenTerminalStatus {
                resource_type,
                raw_status,
                normalized_status,
            } => {
                assert_eq!(resource_type, "Observation");
                assert_eq!(raw_status, *forbidden);
                assert_eq!(normalized_status, *forbidden);
            }
            err => panic!("Expected ForbiddenTerminalStatus, got {:?}", err),
        }
    }
}

#[test]
fn test_assert_write_permitted_catches_nested_status() {
    // Nested in sub-object
    let nested_obj = json!({
        "resourceType": "CarePlan",
        "status": "draft",
        "activity": [
            {
                "detail": {
                    "status": "completed"
                }
            }
        ]
    });
    let result = assert_write_permitted("CarePlan", &nested_obj, true);
    assert!(result.is_err());
    match result.unwrap_err() {
        SafetyViolationError::ForbiddenTerminalStatus {
            raw_status,
            normalized_status,
            ..
        } => {
            assert_eq!(raw_status, "completed");
            assert_eq!(normalized_status, "completed");
        }
        err => panic!("Expected ForbiddenTerminalStatus, got {:?}", err),
    }

    // Nested clinicalStatus coding
    let nested_coding = json!({
        "resourceType": "Condition",
        "clinicalStatus": {
            "coding": [
                {
                    "system": "http://terminology.hl7.org/CodeSystem/condition-clinical",
                    "code": "resolved"
                }
            ]
        }
    });
    let result2 = assert_write_permitted("Condition", &nested_coding, true);
    assert!(result2.is_err());
    match result2.unwrap_err() {
        SafetyViolationError::ForbiddenTerminalStatus {
            raw_status,
            normalized_status,
            ..
        } => {
            assert_eq!(raw_status, "resolved");
            assert_eq!(normalized_status, "resolved");
        }
        err => panic!("Expected ForbiddenTerminalStatus, got {:?}", err),
    }
}

#[test]
fn test_homoglyph_and_unicode_nfkc_normalization() {
    // Cyrillic 'а' (U+0430) in 'active'
    let cyrillic_active = "\u{0430}ctive";
    assert_eq!(normalize_status(cyrillic_active), "active");

    // Full-width Latin 'ａ' (U+FF41) in 'ａctive'
    let fullwidth_active = "\u{ff41}ctive";
    assert_eq!(normalize_status(fullwidth_active), "active");

    // Cyrillic mixed 'соmрlеtеd' (с, о, р, е are Cyrillic)
    let cyrillic_completed = "\u{0441}\u{043e}\u{043c}\u{0440}l\u{0435}t\u{0435}d";
    assert_eq!(normalize_status(cyrillic_completed), "completed");

    // Zero-width space injected into 'active'
    let zw_active = "act\u{200b}ive";
    assert_eq!(normalize_status(zw_active), "active");

    // Attempting to bypass safety gate with homoglyphs
    let malicious_payload = json!({
        "resourceType": "MedicationRequest",
        "status": cyrillic_active
    });
    let result = assert_write_permitted("MedicationRequest", &malicious_payload, true);
    assert!(result.is_err());
    match result.unwrap_err() {
        SafetyViolationError::ForbiddenTerminalStatus {
            raw_status,
            normalized_status,
            ..
        } => {
            assert_eq!(raw_status, cyrillic_active);
            assert_eq!(normalized_status, "active");
        }
        err => panic!("Expected ForbiddenTerminalStatus, got {:?}", err),
    }
}

#[test]
fn test_secret_string_zero_leak() {
    let secret_val = "super-secret-oauth-token-12345";
    let secret = SecretString::new(secret_val);

    // Display must format as ***
    assert_eq!(format!("{}", secret), "***");

    // Debug must format as ***
    assert_eq!(format!("{:?}", secret), "SecretString(\"***\")");

    // expose_secret() exposes the inner string
    assert_eq!(secret.expose_secret(), secret_val);

    // Equality test
    let secret2 = SecretString::new(secret_val);
    let secret3 = SecretString::new("other-token");
    assert_eq!(secret, secret2);
    assert_ne!(secret, secret3);
}
