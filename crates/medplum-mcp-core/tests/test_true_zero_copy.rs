use medplum_mcp_core::rkyv_clinical::{
    access_archived_dataset, archive_clinical_dataset, ClinicalDataset, MedicationRecord,
    ObservationRecord, PatientRecord,
};
use medplum_mcp_core::simd_diet::{distill_resource_simd, SimdDistilledResource};
use medplum_mcp_core::token_diet::DetailLevel;
use medplum_mcp_core::zerocopy_audit::{
    compute_binary_header_signature, verify_binary_header, BinaryAuditHeader, AUDIT_MAGIC,
};
use zerocopy::{FromBytes, IntoBytes};

#[test]
fn test_zerocopy_binary_audit_header_layout_and_transmutation() {
    assert_eq!(
        std::mem::size_of::<BinaryAuditHeader>(),
        120,
        "BinaryAuditHeader must be exactly 120 bytes for standard C-ABI layout"
    );

    let mut header = BinaryAuditHeader::new(
        1,
        1_700_000_000_000,
        0, // Allowed
        [0xAA; 32],
        [0x00; 32],
    );

    assert_eq!(header.magic, AUDIT_MAGIC);
    assert_eq!(header.version, 1);
    assert_eq!(header.sequence_id, 1);
    assert_eq!(header.action_status, 0);

    let secret = b"super-secret-hipaa-audit-key-32b";
    let sig = compute_binary_header_signature(&header, secret);
    header.signature = sig;

    assert!(verify_binary_header(&header, secret));

    // Zero-copy serialization: IntoBytes
    let bytes = header.as_bytes();
    assert_eq!(bytes.len(), 120);

    // Zero-copy deserialization: FromBytes transmutation from slice
    let parsed = BinaryAuditHeader::read_from_bytes(bytes).expect("safe transmutation from slice");
    assert_eq!(parsed, header);

    // Tampering detection
    let mut tampered_bytes = bytes.to_vec();
    tampered_bytes[6] = 1; // Mutate action_status in byte array
    let tampered_header =
        BinaryAuditHeader::read_from_bytes(&tampered_bytes).expect("safe transmutation");
    assert!(!verify_binary_header(&tampered_header, secret));
}

#[test]
fn test_rkyv_zero_deserialization_clinical_dataset() {
    let dataset = ClinicalDataset {
        organization_id: "org-st-jude-childrens".to_string(),
        patients: vec![
            PatientRecord {
                id: "pat-101".to_string(),
                name: "John Doe".to_string(),
                gender: "male".to_string(),
                birth_date: "1985-04-12".to_string(),
            },
            PatientRecord {
                id: "pat-102".to_string(),
                name: "Jane Smith".to_string(),
                gender: "female".to_string(),
                birth_date: "1990-08-23".to_string(),
            },
        ],
        observations: vec![ObservationRecord {
            id: "obs-201".to_string(),
            patient_id: "pat-101".to_string(),
            code: "8867-4".to_string(),
            code_display: "Heart rate".to_string(),
            value: 72.0,
            unit: "beats/minute".to_string(),
        }],
        medication_requests: vec![MedicationRecord {
            id: "med-301".to_string(),
            patient_id: "pat-102".to_string(),
            medication_name: "Amoxicillin 500mg".to_string(),
            status: "draft".to_string(),
            dosage: "500 mg oral tid".to_string(),
        }],
    };

    let archive_bytes =
        archive_clinical_dataset(&dataset).expect("archiving clinical dataset must succeed");

    assert!(!archive_bytes.is_empty());

    // Access without deserializing into Rust heap objects
    let archived = access_archived_dataset(&archive_bytes)
        .expect("zero-deserialization validation and access must succeed");

    assert_eq!(archived.organization_id.as_str(), "org-st-jude-childrens");
    assert_eq!(archived.patients.len(), 2);
    assert_eq!(archived.patients[0].id.as_str(), "pat-101");
    assert_eq!(archived.patients[0].name.as_str(), "John Doe");
    assert_eq!(archived.patients[1].name.as_str(), "Jane Smith");

    assert_eq!(archived.observations.len(), 1);
    assert_eq!(archived.observations[0].code.as_str(), "8867-4");
    assert_eq!(archived.observations[0].value, 72.0);

    assert_eq!(archived.medication_requests.len(), 1);
    assert_eq!(archived.medication_requests[0].status.as_str(), "draft");
    assert_eq!(
        archived.medication_requests[0].medication_name.as_str(),
        "Amoxicillin 500mg"
    );
}

#[test]
fn test_simd_json_in_situ_borrowed_distillation() {
    let mut patient_raw = br#"{
        "resourceType": "Patient",
        "id": "pat-simd-001",
        "name": [{"family": "Williams", "given": ["Serena"]}],
        "gender": "female",
        "birthDate": "1981-09-26",
        "telecom": [{"system": "phone", "value": "555-0199"}]
    }"#
    .to_vec();

    let distilled = distill_resource_simd(&mut patient_raw, DetailLevel::Compact)
        .expect("SIMD borrowed distillation must succeed");

    match distilled {
        SimdDistilledResource::Patient(pat) => {
            assert_eq!(pat.id, "pat-simd-001");
            assert_eq!(pat.name, "Williams, Serena");
            assert_eq!(pat.gender, Some("female"));
            assert_eq!(pat.birth_date, Some("1981-09-26"));
        }
        _ => panic!("Expected SimdDistilledResource::Patient"),
    }
}

#[test]
fn test_benchmark_zerocopy_vs_heap_serde() {
    use std::time::Instant;

    // 1. Zerocopy transmutation vs Serde JSON
    let header = BinaryAuditHeader::new(42, 1_700_000_000_000, 0, [0xAA; 32], [0xBB; 32]);
    let raw_bytes = header.as_bytes();

    let iters = 10_000;

    let start = Instant::now();
    for _ in 0..iters {
        let parsed = BinaryAuditHeader::read_from_bytes(raw_bytes).unwrap();
        std::hint::black_box(parsed);
    }
    let zerocopy_duration = start.elapsed();

    let json_str = serde_json::to_string(&serde_json::json!({
        "sequence_id": 42,
        "timestamp": 1_700_000_000_000u64,
        "status": 0,
        "payload_digest": "aa".repeat(32),
        "prev_signature": "bb".repeat(32),
    }))
    .unwrap();

    let start = Instant::now();
    for _ in 0..iters {
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        std::hint::black_box(parsed);
    }
    let serde_duration = start.elapsed();

    println!(
        "\n[BENCHMARK] Zerocopy Audit Frame: {:?} vs Serde JSON: {:?} (Speedup: {:.1}x)",
        zerocopy_duration,
        serde_duration,
        serde_duration.as_nanos() as f64 / zerocopy_duration.as_nanos().max(1) as f64
    );
    assert!(zerocopy_duration < serde_duration);

    // 2. Rkyv zero-deserialization access vs Serde JSON
    let dataset = ClinicalDataset {
        organization_id: "org-bench".to_string(),
        patients: vec![PatientRecord {
            id: "pat-1".to_string(),
            name: "Alice".to_string(),
            gender: "female".to_string(),
            birth_date: "1990-01-01".to_string(),
        }],
        observations: vec![ObservationRecord {
            id: "obs-1".to_string(),
            patient_id: "pat-1".to_string(),
            code: "8867-4".to_string(),
            code_display: "HR".to_string(),
            value: 70.0,
            unit: "bpm".to_string(),
        }],
        medication_requests: vec![],
    };
    let rkyv_bytes = archive_clinical_dataset(&dataset).unwrap();

    let start = Instant::now();
    for _ in 0..iters {
        let accessed = access_archived_dataset(&rkyv_bytes).unwrap();
        std::hint::black_box(accessed.patients[0].name.as_str());
    }
    let rkyv_duration = start.elapsed();

    let json_dataset = serde_json::json!({
        "organization_id": "org-bench",
        "patients": [{"id": "pat-1", "name": "Alice", "gender": "female", "birth_date": "1990-01-01"}],
        "observations": [{"id": "obs-1", "patient_id": "pat-1", "code": "8867-4", "code_display": "HR", "value": 70.0, "unit": "bpm"}],
        "medication_requests": []
    }).to_string();

    let start = Instant::now();
    for _ in 0..iters {
        let parsed: serde_json::Value = serde_json::from_str(&json_dataset).unwrap();
        std::hint::black_box(&parsed["patients"][0]["name"]);
    }
    let serde_clinical_duration = start.elapsed();

    println!(
        "[BENCHMARK] Rkyv Zero-Deserialization: {:?} vs Serde JSON: {:?} (Speedup: {:.1}x)",
        rkyv_duration,
        serde_clinical_duration,
        serde_clinical_duration.as_nanos() as f64 / rkyv_duration.as_nanos().max(1) as f64
    );
    assert!(rkyv_duration < serde_clinical_duration);
}
