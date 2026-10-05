use std::fs;
use std::str::FromStr;
use tempfile::tempdir;

use medplum_mcp_core::audit::{
    verify_audit_log, ActionStatus, AuditError, AuditLogManager, GENESIS_PREV_SIGNATURE,
};
use medplum_mcp_core::benchmarks::{
    run_fhir_benchmarks, SYNTHETIC_ALLERGY_INTOLERANCE, SYNTHETIC_BUNDLE, SYNTHETIC_CARE_PLAN,
    SYNTHETIC_CONDITION, SYNTHETIC_DIAGNOSTIC_REPORT, SYNTHETIC_ENCOUNTER,
    SYNTHETIC_MEDICATION_REQUEST, SYNTHETIC_OBSERVATION, SYNTHETIC_PATIENT,
};
use medplum_mcp_core::token_diet::{distill_resource, DetailLevel};

#[test]
fn test_detail_level_from_str_case_insensitive() {
    assert_eq!(
        DetailLevel::from_str("compact").unwrap(),
        DetailLevel::Compact
    );
    assert_eq!(
        DetailLevel::from_str("COMPACT").unwrap(),
        DetailLevel::Compact
    );
    assert_eq!(
        DetailLevel::from_str("Compact").unwrap(),
        DetailLevel::Compact
    );

    assert_eq!(
        DetailLevel::from_str("standard").unwrap(),
        DetailLevel::Standard
    );
    assert_eq!(
        DetailLevel::from_str("STANDARD").unwrap(),
        DetailLevel::Standard
    );
    assert_eq!(
        DetailLevel::from_str("Standard").unwrap(),
        DetailLevel::Standard
    );

    assert_eq!(
        DetailLevel::from_str("executive").unwrap(),
        DetailLevel::Executive
    );
    assert_eq!(
        DetailLevel::from_str("EXECUTIVE").unwrap(),
        DetailLevel::Executive
    );
    assert_eq!(
        DetailLevel::from_str("Executive").unwrap(),
        DetailLevel::Executive
    );

    assert!(DetailLevel::from_str("invalid").is_err());
    assert!(DetailLevel::from_str("").is_err());
}

#[test]
fn test_distill_patient() {
    let raw = &*SYNTHETIC_PATIENT;

    // Compact
    let compact = distill_resource(raw, DetailLevel::Compact);
    assert_eq!(compact["resourceType"], "Patient");
    assert_eq!(compact["id"], "pat-synthetic-001");
    assert_eq!(compact["name"], "Jane A. Smith");
    assert_eq!(compact["identifier"], "MRN-12345");
    assert_eq!(compact["gender"], "female");
    assert_eq!(compact["birthDate"], "1980-01-15");
    assert!(compact.get("text").is_none());
    assert!(compact.get("meta").is_none());
    assert!(compact.get("extension").is_none());
    assert!(compact.get("telecom").is_none());

    // Standard
    let standard = distill_resource(raw, DetailLevel::Standard);
    assert_eq!(standard["active"], true);
    assert!(standard["telecom"].is_array());
    assert_eq!(standard["telecom"][0]["value"], "555-0100");

    // Executive
    let exec = distill_resource(raw, DetailLevel::Executive);
    assert_eq!(exec["active"], true);
    assert!(exec.get("telecom").is_none());
}

#[test]
fn test_distill_observation() {
    let raw = &*SYNTHETIC_OBSERVATION;

    // Compact
    let compact = distill_resource(raw, DetailLevel::Compact);
    assert_eq!(compact["resourceType"], "Observation");
    assert_eq!(compact["code"], "Fasting Blood Glucose");
    assert_eq!(compact["value"], 140);
    assert_eq!(compact["unit"], "mg/dL");
    assert_eq!(compact["status"], "final");
    assert!(compact.get("referenceRange").is_none());
    assert!(compact.get("interpretation").is_none());

    // Standard
    let standard = distill_resource(raw, DetailLevel::Standard);
    assert_eq!(standard["referenceRange"], "70 - 99 mg/dL");
    assert_eq!(standard["interpretation"], "High");
    assert_eq!(
        standard["performer"],
        "Dr. Gregory House, Clinical Pathologist"
    );

    // Executive
    let exec = distill_resource(raw, DetailLevel::Executive);
    assert_eq!(exec["value"], "140 mg/dL");
    assert_eq!(exec["is_critical"], true);
}

#[test]
fn test_distill_condition() {
    let raw = &*SYNTHETIC_CONDITION;

    // Compact
    let compact = distill_resource(raw, DetailLevel::Compact);
    assert_eq!(compact["resourceType"], "Condition");
    assert_eq!(compact["code"], "Type 2 Diabetes Mellitus");
    assert_eq!(compact["clinicalStatus"], "active");

    // Standard
    let standard = distill_resource(raw, DetailLevel::Standard);
    assert_eq!(standard["verificationStatus"], "confirmed");
    assert_eq!(standard["onsetDateTime"], "2020-01-15");

    // Executive
    let exec = distill_resource(raw, DetailLevel::Executive);
    assert_eq!(exec["is_active"], true);
}

#[test]
fn test_distill_medication_request() {
    let raw = &*SYNTHETIC_MEDICATION_REQUEST;

    // Compact
    let compact = distill_resource(raw, DetailLevel::Compact);
    assert_eq!(compact["resourceType"], "MedicationRequest");
    assert_eq!(compact["medication"], "Metformin 500mg tablet");
    assert_eq!(compact["status"], "active");

    // Standard
    let standard = distill_resource(raw, DetailLevel::Standard);
    assert_eq!(standard["intent"], "order");
    assert_eq!(
        standard["dosageInstruction"],
        "Take 1 tablet daily with meals"
    );

    // Executive
    let exec = distill_resource(raw, DetailLevel::Executive);
    assert_eq!(exec["intent"], "order");
    assert!(exec.get("dosageInstruction").is_none());
}

#[test]
fn test_distill_allergy_intolerance() {
    let raw = &*SYNTHETIC_ALLERGY_INTOLERANCE;

    // Compact
    let compact = distill_resource(raw, DetailLevel::Compact);
    assert_eq!(compact["resourceType"], "AllergyIntolerance");
    assert_eq!(compact["criticality"], "high");
    assert_eq!(compact["code"], "Penicillin allergy");

    // Standard
    let standard = distill_resource(raw, DetailLevel::Standard);
    assert!(standard["reaction"].is_array());
    assert_eq!(standard["reaction"][0]["manifestation"], "Anaphylaxis");

    // Executive
    let exec = distill_resource(raw, DetailLevel::Executive);
    assert_eq!(exec["is_critical"], true);
}

#[test]
fn test_distill_diagnostic_report() {
    let raw = &*SYNTHETIC_DIAGNOSTIC_REPORT;

    // Compact
    let compact = distill_resource(raw, DetailLevel::Compact);
    assert_eq!(compact["resourceType"], "DiagnosticReport");
    assert_eq!(compact["code"], "Comprehensive Metabolic Panel");

    // Standard
    let standard = distill_resource(raw, DetailLevel::Standard);
    assert_eq!(standard["category"], "Laboratory");
    assert!(standard["conclusion"]
        .as_str()
        .unwrap()
        .contains("Elevated fasting glucose"));

    // Executive
    let exec = distill_resource(raw, DetailLevel::Executive);
    assert_eq!(exec["category"], "Laboratory");
}

#[test]
fn test_distill_encounter() {
    let raw = &*SYNTHETIC_ENCOUNTER;

    // Compact
    let compact = distill_resource(raw, DetailLevel::Compact);
    assert_eq!(compact["resourceType"], "Encounter");
    assert_eq!(compact["class"], "AMB");
    assert_eq!(compact["type"], "Routine checkup");

    // Standard
    let standard = distill_resource(raw, DetailLevel::Standard);
    assert_eq!(
        standard["reasonCode"],
        "Routine general medical examination"
    );

    // Executive
    let exec = distill_resource(raw, DetailLevel::Executive);
    assert_eq!(exec["reason"], "Routine general medical examination");
}

#[test]
fn test_distill_care_plan() {
    let raw = &*SYNTHETIC_CARE_PLAN;

    // Compact
    let compact = distill_resource(raw, DetailLevel::Compact);
    assert_eq!(compact["resourceType"], "CarePlan");
    assert_eq!(compact["title"], "Diabetes Management Plan");

    // Standard
    let standard = distill_resource(raw, DetailLevel::Standard);
    assert!(standard["activities"].is_array());
    assert_eq!(standard["activities"].as_array().unwrap().len(), 3);

    // Executive
    let exec = distill_resource(raw, DetailLevel::Executive);
    assert_eq!(exec["activity_count"], 3);
}

#[test]
fn test_distill_bundle_recursive() {
    let raw = &*SYNTHETIC_BUNDLE;

    let compact = distill_resource(raw, DetailLevel::Compact);
    assert_eq!(compact["resourceType"], "Bundle");
    assert_eq!(compact["total"], 8);
    let entries = compact["entry"].as_array().unwrap();
    assert_eq!(entries.len(), 8);
    assert_eq!(entries[0]["resource"]["resourceType"], "Patient");
    assert_eq!(entries[1]["resource"]["resourceType"], "Observation");

    let standard = distill_resource(raw, DetailLevel::Standard);
    assert_eq!(standard["resourceType"], "Bundle");
    assert_eq!(standard["total"], 8);
    assert!(standard["entry"][0].get("fullUrl").is_some());
}

#[test]
fn test_bundle_reduction_exceeds_85_percent() {
    let raw = &*SYNTHETIC_BUNDLE;
    let raw_bytes = serde_json::to_string(raw).unwrap().len() as f64;

    for level in [
        DetailLevel::Compact,
        DetailLevel::Standard,
        DetailLevel::Executive,
    ] {
        let distilled = distill_resource(raw, level);
        let distilled_bytes = serde_json::to_string(&distilled).unwrap().len() as f64;
        let reduction = (1.0 - (distilled_bytes / raw_bytes)) * 100.0;
        assert!(
            reduction > 85.0,
            "Reduction for {:?} was {:.2}%, expected >85%",
            level,
            reduction
        );
    }
}

#[test]
fn test_audit_action_status_and_genesis() {
    assert_eq!(ActionStatus::Allowed.as_str(), "ALLOWED");
    assert_eq!(ActionStatus::Blocked.as_str(), "BLOCKED");
    assert_eq!(ActionStatus::Error.as_str(), "ERROR");
    assert_eq!(
        GENESIS_PREV_SIGNATURE,
        "0000000000000000000000000000000000000000000000000000000000000000"
    );
}

#[test]
fn test_audit_log_manager_append_and_verify() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");
    let secret_key = b"super-secret-hipaa-audit-key-2026";

    let mut manager = AuditLogManager::new(&log_path, secret_key).unwrap();

    let e1 = manager
        .log_event(
            "search_patients",
            ActionStatus::Allowed,
            Some(&serde_json::json!({"query": "Smith"})),
        )
        .unwrap();

    assert_eq!(e1.sequence_id, 1);
    assert_eq!(e1.prev_signature, GENESIS_PREV_SIGNATURE);
    assert_eq!(e1.action_status, ActionStatus::Allowed);

    let e2 = manager
        .log_event(
            "update_status",
            ActionStatus::Blocked,
            Some(&serde_json::json!({"status": "active"})),
        )
        .unwrap();

    assert_eq!(e2.sequence_id, 2);
    assert_eq!(e2.prev_signature, e1.signature);

    let e3 = manager
        .log_event("query_server", ActionStatus::Error, None)
        .unwrap();

    assert_eq!(e3.sequence_id, 3);
    assert_eq!(e3.prev_signature, e2.signature);

    // Verify log
    let report = verify_audit_log(&log_path, secret_key).unwrap();
    assert_eq!(report.verified_count, 3);
    assert!(report.is_valid);
}

#[test]
fn test_audit_verification_tampered_payload() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");
    let secret_key = b"secret-key-123";

    let mut manager = AuditLogManager::new(&log_path, secret_key).unwrap();
    manager
        .log_event(
            "test_tool",
            ActionStatus::Allowed,
            Some(&serde_json::json!({"field": "original"})),
        )
        .unwrap();

    // Tamper the payload in file
    let content = fs::read_to_string(&log_path).unwrap();
    let tampered = content.replace("original", "tampered");
    fs::write(&log_path, tampered).unwrap();

    let err = verify_audit_log(&log_path, secret_key).unwrap_err();
    match err {
        AuditError::TamperedPayload { sequence_id, .. } => {
            assert_eq!(sequence_id, 1);
        }
        other => panic!("Expected TamperedPayload, got {:?}", other),
    }
}

#[test]
fn test_audit_verification_forged_signature() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");
    let secret_key = b"secret-key-123";

    let mut manager = AuditLogManager::new(&log_path, secret_key).unwrap();
    let e1 = manager
        .log_event("tool1", ActionStatus::Allowed, None)
        .unwrap();

    // Replace signature with dummy
    let content = fs::read_to_string(&log_path).unwrap();
    let tampered = content.replace(
        &e1.signature,
        "deadbeefcafebabe0000111122223333444455556666777788889999aaaabbbb",
    );
    fs::write(&log_path, tampered).unwrap();

    let err = verify_audit_log(&log_path, secret_key).unwrap_err();
    match err {
        AuditError::ForgedSignature { sequence_id, .. } => {
            assert_eq!(sequence_id, 1);
        }
        other => panic!("Expected ForgedSignature, got {:?}", other),
    }
}

#[test]
fn test_audit_verification_altered_timestamp() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");
    let secret_key = b"secret-key-123";

    let mut manager = AuditLogManager::new(&log_path, secret_key).unwrap();
    manager
        .log_event_with_timestamp("tool1", ActionStatus::Allowed, None, "2026-10-04T12:00:00Z")
        .unwrap();
    manager
        .log_event_with_timestamp(
            "tool2",
            ActionStatus::Allowed,
            None,
            "2026-10-04T11:00:00Z", // Earlier than first
        )
        .unwrap();

    let err = verify_audit_log(&log_path, secret_key).unwrap_err();
    match err {
        AuditError::AlteredTimestamp { sequence_id, .. } => {
            assert_eq!(sequence_id, 2);
        }
        other => panic!("Expected AlteredTimestamp, got {:?}", other),
    }
}

#[test]
fn test_audit_verification_broken_sequence_id() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");
    let secret_key = b"secret-key-123";

    let mut manager = AuditLogManager::new(&log_path, secret_key).unwrap();
    manager
        .log_event("tool1", ActionStatus::Allowed, None)
        .unwrap();

    // Tamper sequence_id to 42
    let content = fs::read_to_string(&log_path).unwrap();
    let tampered = content.replace("\"sequence_id\":1", "\"sequence_id\":42");
    fs::write(&log_path, tampered).unwrap();

    let err = verify_audit_log(&log_path, secret_key).unwrap_err();
    match err {
        AuditError::BrokenSequence {
            expected, found, ..
        } => {
            assert_eq!(expected, 1);
            assert_eq!(found, 42);
        }
        other => panic!("Expected BrokenSequence, got {:?}", other),
    }
}

#[test]
fn test_audit_verification_modified_genesis_signature() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");
    let secret_key = b"secret-key-123";

    let mut manager = AuditLogManager::new(&log_path, secret_key).unwrap();
    manager
        .log_event("tool1", ActionStatus::Allowed, None)
        .unwrap();

    let content = fs::read_to_string(&log_path).unwrap();
    let tampered = content.replace(
        GENESIS_PREV_SIGNATURE,
        "1111111111111111111111111111111111111111111111111111111111111111",
    );
    fs::write(&log_path, tampered).unwrap();

    let err = verify_audit_log(&log_path, secret_key).unwrap_err();
    match err {
        AuditError::InvalidGenesisSignature { .. } => {}
        other => panic!("Expected InvalidGenesisSignature, got {:?}", other),
    }
}

#[test]
fn test_benchmarks_aggregate_reduction() {
    let report = run_fhir_benchmarks().unwrap();
    assert!(
        report.aggregate_compact_reduction_pct > 85.0,
        "Compact reduction: {:.2}%",
        report.aggregate_compact_reduction_pct
    );
    assert!(
        report.aggregate_standard_reduction_pct > 80.0,
        "Standard reduction: {:.2}%",
        report.aggregate_standard_reduction_pct
    );
    assert!(
        report.aggregate_executive_reduction_pct > 85.0,
        "Executive reduction: {:.2}%",
        report.aggregate_executive_reduction_pct
    );
    assert!(
        report.overall_reduction_pct >= 85.0,
        "Overall reduction: {:.2}%",
        report.overall_reduction_pct
    );
}
