//! Medplum MCP Core
//!
//! Core primitives, typestate invariants, zero-leak credential enclaves,
//! non-bypassable clinical safety gates, 3-tier FHIR context distillation,
//! and HIPAA cryptographic audit flight recorder.

pub mod audit;
pub mod benchmarks;
pub mod rkyv_clinical;
pub mod safety;
pub mod secret;
pub mod simd_diet;
pub mod token_diet;
pub mod typestate;
pub mod zerocopy_audit;

// Re-exports for convenient downstream access
pub use audit::{
    compute_entry_signature, compute_payload_digest, verify_audit_log, ActionStatus, AuditEntry,
    AuditError, AuditLogManager, AuditVerificationReport, GENESIS_PREV_SIGNATURE,
};
pub use benchmarks::{run_fhir_benchmarks, AggregateBenchmarkReport, BenchmarkResult};
pub use rkyv_clinical::{
    access_archived_dataset, archive_clinical_dataset, ArchivedClinicalDataset,
    ArchivedMedicationRequest, ArchivedObservation, ArchivedPatient, ClinicalDataset,
    MedicationRecord, ObservationRecord, PatientRecord,
};
pub use safety::{
    assert_write_permitted, normalize_status, SafetyViolationError, FORBIDDEN_CLINICAL_STATUSES,
};
pub use secret::SecretString;
pub use simd_diet::{distill_resource_simd, SimdDistilledResource};
pub use token_diet::{distill_resource, DetailLevel, TokenDietError};
pub use typestate::{
    Active, Cancelled, ClinicalState, Completed, Draft, MedicationRequest, PhysicianWitness,
};
pub use zerocopy_audit::{
    compute_binary_header_signature, verify_binary_header, BinaryAuditHeader, AUDIT_MAGIC,
    AUDIT_VERSION,
};
