//! Test suite for rkyv zero-deserialization clinical archive integration in ClinicalSandbox.

use medplum_mcp_core::rkyv_clinical::access_archived_dataset;
use medplum_mcp_server::sandbox::ClinicalSandbox;

#[test]
fn test_sandbox_export_and_access_rkyv_archive() {
    let sandbox = ClinicalSandbox::new_st_jude();

    // 1. Export sandbox dataset to zero-deserialization rkyv binary bytes
    let archive_bytes = sandbox
        .export_rkyv_archive()
        .expect("exporting rkyv archive from sandbox must succeed");

    assert!(
        !archive_bytes.is_empty(),
        "rkyv archive buffer must not be empty"
    );

    // 2. Access archived dataset directly from byte slice with zero allocations
    let archived = access_archived_dataset(&archive_bytes)
        .expect("accessing archived clinical dataset must succeed");

    assert_eq!(
        archived.organization_id.as_str(),
        "st-jude-research-institute"
    );
    assert!(
        !archived.patients.is_empty(),
        "archived patients must not be empty"
    );
    assert!(
        !archived.observations.is_empty(),
        "archived observations must not be empty"
    );
    assert!(
        !archived.medication_requests.is_empty(),
        "archived medications must not be empty"
    );

    // 3. Verify zero-deserialization lookup helpers on the archive
    let patient_id = archived.patients[0].id.as_str();
    let pat = ClinicalSandbox::query_archived_patient(archived, patient_id);
    assert!(pat.is_some(), "should find patient in zero-copy archive");
    assert_eq!(pat.unwrap().id.as_str(), patient_id);

    let obs_list = ClinicalSandbox::query_archived_observations(archived, patient_id);
    assert!(
        !obs_list.is_empty(),
        "should find observations for patient in archive"
    );
    assert_eq!(obs_list[0].patient_id.as_str(), patient_id);
}
