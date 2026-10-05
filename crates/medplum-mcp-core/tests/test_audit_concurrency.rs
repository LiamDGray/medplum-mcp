//! Concurrency, Data Race & Multi-Thread Contention Test Suite for medplum-mcp-core.
//!
//! Asserts:
//! 1. Heavy multi-threaded contention on AuditLogManager (50 threads x 20 entries = 1,000 entries)
//!    preserves strict sequence monotonicity and unbroken HMAC-SHA256 hash chaining.
//! 2. Concurrent token distillation across 40 threads produces deterministic, bit-identical outputs
//!    with zero thread interference.
//! 3. Concurrent zero-deserialization rkyv pointer access across threads without data races.
//! 4. Concurrent binary audit frame transmutation and verification.

use std::sync::{Arc, Mutex};
use std::thread;
use tempfile::NamedTempFile;
use zerocopy::{FromBytes, IntoBytes};

use medplum_mcp_core::audit::{verify_audit_log, ActionStatus, AuditLogManager};
use medplum_mcp_core::benchmarks::SYNTHETIC_BUNDLE;
use medplum_mcp_core::rkyv_clinical::{
    access_archived_dataset, archive_clinical_dataset, ClinicalDataset, PatientRecord,
};
use medplum_mcp_core::token_diet::{distill_resource, DetailLevel};
use medplum_mcp_core::zerocopy_audit::{verify_binary_header, BinaryAuditHeader};

#[test]
fn test_concurrent_audit_log_manager_heavy_contention() {
    let temp_file = NamedTempFile::new().unwrap();
    let log_path = temp_file.path().to_path_buf();
    let secret_key = b"concurrent-stress-test-hmac-key";

    let manager = Arc::new(Mutex::new(
        AuditLogManager::new(&log_path, secret_key).unwrap(),
    ));

    const NUM_THREADS: usize = 50;
    const ENTRIES_PER_THREAD: usize = 20;
    const TOTAL_ENTRIES: usize = NUM_THREADS * ENTRIES_PER_THREAD;

    let mut handles = Vec::with_capacity(NUM_THREADS);

    for t_idx in 0..NUM_THREADS {
        let mgr = Arc::clone(&manager);
        let handle = thread::spawn(move || {
            for e_idx in 0..ENTRIES_PER_THREAD {
                let tool_name = format!("concurrent_tool_t{}_e{}", t_idx, e_idx);
                let payload = serde_json::json!({
                    "thread_id": t_idx,
                    "entry_id": e_idx,
                    "data": "clinical_payload_audit"
                });

                let mut guard = mgr.lock().unwrap();
                let status = if e_idx % 3 == 0 {
                    ActionStatus::Blocked
                } else {
                    ActionStatus::Allowed
                };
                guard
                    .log_event(&tool_name, status, Some(&payload))
                    .expect("log_event must succeed under lock");
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // 1. Verify exact total count
    let guard = manager.lock().unwrap();
    let entries = guard.get_entries().unwrap();
    assert_eq!(entries.len(), TOTAL_ENTRIES);

    // 2. Verify sequence IDs are strictly monotonic 1..=TOTAL_ENTRIES
    for (idx, entry) in entries.iter().enumerate() {
        assert_eq!(
            entry.sequence_id,
            (idx + 1) as u64,
            "Sequence ID must be contiguous and monotonic"
        );
    }
    drop(guard);

    // 3. Cryptographically verify the entire HMAC-SHA256 blockchain
    let report = verify_audit_log(&log_path, secret_key).expect("verify_audit_log must not error");
    assert!(
        report.is_valid,
        "Tamper check failed on concurrent audit log"
    );
    assert_eq!(report.verified_count, TOTAL_ENTRIES);
}

#[test]
fn test_concurrent_token_diet_purity_and_thread_safety() {
    let raw_bundle: serde_json::Value = SYNTHETIC_BUNDLE.clone();
    let shared_bundle = Arc::new(raw_bundle);

    // Precompute single-threaded ground truth for all 3 tiers
    let truth_compact = distill_resource(&shared_bundle, DetailLevel::Compact);
    let truth_standard = distill_resource(&shared_bundle, DetailLevel::Standard);
    let truth_executive = distill_resource(&shared_bundle, DetailLevel::Executive);

    const NUM_THREADS: usize = 40;
    const ITERATIONS_PER_THREAD: usize = 25;

    let mut handles = Vec::with_capacity(NUM_THREADS);

    for _ in 0..NUM_THREADS {
        let bundle = Arc::clone(&shared_bundle);
        let c_exp = truth_compact.clone();
        let s_exp = truth_standard.clone();
        let e_exp = truth_executive.clone();

        let handle = thread::spawn(move || {
            for _ in 0..ITERATIONS_PER_THREAD {
                let compact = distill_resource(&bundle, DetailLevel::Compact);
                assert_eq!(
                    compact, c_exp,
                    "Compact distillation must be deterministic and thread-safe"
                );

                let standard = distill_resource(&bundle, DetailLevel::Standard);
                assert_eq!(
                    standard, s_exp,
                    "Standard distillation must be deterministic and thread-safe"
                );

                let executive = distill_resource(&bundle, DetailLevel::Executive);
                assert_eq!(
                    executive, e_exp,
                    "Executive distillation must be deterministic and thread-safe"
                );
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_concurrent_rkyv_dataset_reads() {
    // Build synthetic clinical dataset
    let mut dataset = ClinicalDataset {
        organization_id: "org-st-jude-research".to_string(),
        patients: Vec::new(),
        observations: Vec::new(),
        medication_requests: Vec::new(),
    };

    for i in 0..100 {
        dataset.patients.push(PatientRecord {
            id: format!("pat-stress-{}", i),
            name: format!("Patient Stress {}", i),
            gender: "female".to_string(),
            birth_date: "2015-05-15".to_string(),
        });
    }

    let archived_bytes = Arc::new(archive_clinical_dataset(&dataset).unwrap());

    const NUM_THREADS: usize = 30;
    const READS_PER_THREAD: usize = 100;

    let mut handles = Vec::with_capacity(NUM_THREADS);

    for _ in 0..NUM_THREADS {
        let bytes = Arc::clone(&archived_bytes);
        let handle = thread::spawn(move || {
            for i in 0..READS_PER_THREAD {
                let target_idx = i % 100;
                let expected_id = format!("pat-stress-{}", target_idx);

                // Zero-deserialization direct access from raw pointer
                let archived = access_archived_dataset(&bytes).unwrap();
                let pat = &archived.patients[target_idx];
                assert_eq!(pat.id.as_str(), expected_id.as_str());
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_concurrent_zerocopy_binary_frame_transmutation() {
    let mut header = BinaryAuditHeader::new(
        1001,
        1700000000000,
        0, // Allowed
        [0xAA; 32],
        [0xBB; 32],
    );
    header.sign(b"secret-key-binary-stress-test");

    let bytes = Arc::new(header.as_bytes().to_vec());

    const NUM_THREADS: usize = 40;
    const TRANSMUTES_PER_THREAD: usize = 500;

    let mut handles = Vec::with_capacity(NUM_THREADS);

    for _ in 0..NUM_THREADS {
        let raw = Arc::clone(&bytes);
        let expected = header;

        let handle = thread::spawn(move || {
            for _ in 0..TRANSMUTES_PER_THREAD {
                // Zero-cost pointer transmutation
                let parsed = BinaryAuditHeader::ref_from_bytes(&raw).unwrap();
                assert_eq!(*parsed, expected);
                assert!(verify_binary_header(
                    parsed,
                    b"secret-key-binary-stress-test"
                ));
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}
