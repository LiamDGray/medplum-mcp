//! Microsecond empirical benchmark for Recommendation 3:
//! Rkyv zero-deserialization clinical archive lookup vs standard Serde Value cloning.

use std::time::Instant;

use medplum_mcp_core::rkyv_clinical::access_archived_dataset;
use medplum_mcp_server::sandbox::ClinicalSandbox;

#[test]
fn test_benchmark_recommendation_3_rkyv_vs_serde_lookup() {
    let sandbox = ClinicalSandbox::new_st_jude();
    let archive_bytes = sandbox
        .export_rkyv_archive()
        .expect("exporting rkyv archive from sandbox must succeed");

    let archived = access_archived_dataset(&archive_bytes)
        .expect("accessing archived clinical dataset must succeed");

    let patient_id = "pat-sj-001";
    let iters = 50_000;

    // 1. Serde DOM Value lookup (sandbox.get_patient clones serde_json::Value)
    let start_serde = Instant::now();
    for _ in 0..iters {
        let pat = sandbox.get_patient(patient_id).unwrap();
        assert_eq!(pat["id"], patient_id);
    }
    let serde_duration = start_serde.elapsed();

    // 2. Rkyv zero-deserialization byte-offset lookup
    let start_rkyv = Instant::now();
    for _ in 0..iters {
        let pat = ClinicalSandbox::query_archived_patient(archived, patient_id).unwrap();
        assert_eq!(pat.id.as_str(), patient_id);
    }
    let rkyv_duration = start_rkyv.elapsed();

    // 3. Observations query comparison
    let start_serde_obs = Instant::now();
    for _ in 0..iters {
        let obs = sandbox.list_observations(Some(patient_id), None, None);
        assert!(!obs.is_empty());
    }
    let serde_obs_duration = start_serde_obs.elapsed();

    let start_rkyv_obs = Instant::now();
    for _ in 0..iters {
        let obs = ClinicalSandbox::query_archived_observations(archived, patient_id);
        assert!(!obs.is_empty());
    }
    let rkyv_obs_duration = start_rkyv_obs.elapsed();

    let serde_pat_nanos = serde_duration.as_nanos() as f64 / iters as f64;
    let rkyv_pat_nanos = rkyv_duration.as_nanos() as f64 / iters as f64;
    let serde_obs_nanos = serde_obs_duration.as_nanos() as f64 / iters as f64;
    let rkyv_obs_nanos = rkyv_obs_duration.as_nanos() as f64 / iters as f64;

    eprintln!("\n=== RECOMMENDATION 3 BENCHMARK: Rkyv Zero-Copy vs Serde Value Lookup ===");
    eprintln!(
        "Patient Lookup: Serde DOM Value = {:.1} ns/op | Rkyv Zero-Copy = {:.1} ns/op | Speedup = {:.2}x",
        serde_pat_nanos,
        rkyv_pat_nanos,
        serde_pat_nanos / rkyv_pat_nanos
    );
    eprintln!(
        "Observation List: Serde DOM Value = {:.1} ns/op | Rkyv Zero-Copy = {:.1} ns/op | Speedup = {:.2}x",
        serde_obs_nanos,
        rkyv_obs_nanos,
        serde_obs_nanos / rkyv_obs_nanos
    );

    assert!(rkyv_pat_nanos > 0.0);
    assert!(serde_pat_nanos > 0.0);
}
