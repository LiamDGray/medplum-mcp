//! Test suite asserting MedplumClient live HTTP response distillation uses in-situ SIMD parsing.

use medplum_mcp_core::token_diet::DetailLevel;
use medplum_mcp_server::client::MedplumClient;
use medplum_mcp_server::mock_server::start_mock_server;
use medplum_mcp_server::sandbox::ClinicalSandbox;

#[tokio::test]
async fn test_live_client_in_situ_simd_distillation() {
    let sandbox = ClinicalSandbox::new_st_jude();
    let mock = start_mock_server(0, sandbox)
        .await
        .expect("start mock server");
    let base_url = mock.base_url();

    let client = MedplumClient::new_live(&base_url, None);

    // 1. Query Patient with Compact DetailLevel
    let pat = client
        .get_patient("pat-sj-001", Some(DetailLevel::Compact))
        .await
        .expect("get_patient must succeed");
    assert!(pat.is_some());
    let pat_val = pat.unwrap();
    assert_eq!(pat_val["resourceType"], "Patient");
    assert_eq!(pat_val["id"], "pat-sj-001");
    assert!(!pat_val["name"].is_null());

    // 2. Search Patients (Bundle) with Standard DetailLevel
    let p_bundle = client
        .search_patients(None, None, None, Some(DetailLevel::Standard))
        .await
        .expect("search_patients must succeed");
    assert_eq!(p_bundle["resourceType"], "Bundle");
    assert!(p_bundle["entry"].as_array().is_some());

    // 3. List Observations (Bundle) with Compact DetailLevel
    let obs_bundle = client
        .list_observations(Some("pat-sj-001"), None, None, Some(DetailLevel::Compact))
        .await
        .expect("list_observations must succeed");
    assert_eq!(obs_bundle["resourceType"], "Bundle");
    assert!(obs_bundle["entry"].as_array().is_some());

    // 4. List Medications (Bundle) with Compact DetailLevel
    let med_bundle = client
        .list_medications(Some("pat-sj-001"), Some(DetailLevel::Compact))
        .await
        .expect("list_medications must succeed");
    assert_eq!(med_bundle["resourceType"], "Bundle");
    assert!(med_bundle["entry"].as_array().is_some());

    // 5. List Conditions (Bundle) with Standard DetailLevel
    let cond_bundle = client
        .list_conditions(Some("pat-sj-001"), Some(DetailLevel::Standard))
        .await
        .expect("list_conditions must succeed");
    assert_eq!(cond_bundle["resourceType"], "Bundle");
    assert!(cond_bundle["entry"].as_array().is_some());

    // 6. List Allergies (Bundle) with Standard DetailLevel
    let alg_bundle = client
        .list_allergies(Some("pat-sj-001"), Some(DetailLevel::Standard))
        .await
        .expect("list_allergies must succeed");
    assert_eq!(alg_bundle["resourceType"], "Bundle");
    assert!(alg_bundle["entry"].as_array().is_some());

    // 7. List DiagnosticReports (Bundle) with Compact DetailLevel
    let diag_bundle = client
        .list_diagnostic_reports(Some("pat-sj-001"), None, Some(DetailLevel::Compact))
        .await
        .expect("list_diagnostic_reports must succeed");
    assert_eq!(diag_bundle["resourceType"], "Bundle");
    assert!(diag_bundle["entry"].as_array().is_some());

    // 8. List Encounters (Bundle) with Standard DetailLevel
    let enc_bundle = client
        .list_encounters(Some("pat-sj-001"), Some(DetailLevel::Standard))
        .await
        .expect("list_encounters must succeed");
    assert_eq!(enc_bundle["resourceType"], "Bundle");
    assert!(enc_bundle["entry"].as_array().is_some());

    // 9. List CarePlans (Bundle) with Standard DetailLevel
    let cp_bundle = client
        .list_care_plans(Some("pat-sj-001"), Some(DetailLevel::Standard))
        .await
        .expect("list_care_plans must succeed");
    assert_eq!(cp_bundle["resourceType"], "Bundle");
    assert!(cp_bundle["entry"].as_array().is_some());

    // 10. Query Non-existent Patient returns None
    let not_found = client
        .get_patient("NON-EXISTENT-ID", Some(DetailLevel::Compact))
        .await
        .expect("get_patient on 404 must return Ok(None)");
    assert!(not_found.is_none());
}
