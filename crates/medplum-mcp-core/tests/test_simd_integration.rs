//! Test suite for SIMD distillation integration and to_value conversion.

use medplum_mcp_core::simd_diet::distill_resource_simd;
use medplum_mcp_core::token_diet::{distill_raw_slice, DetailLevel};

#[test]
fn test_simd_distilled_patient_to_value() {
    let mut patient_raw = br#"{
        "resourceType": "Patient",
        "id": "pat-simd-777",
        "name": [{"family": "Curie", "given": ["Marie"]}],
        "gender": "female",
        "birthDate": "1867-11-07"
    }"#
    .to_vec();

    let distilled = distill_resource_simd(&mut patient_raw, DetailLevel::Compact)
        .expect("simd distillation should succeed");

    assert_eq!(distilled.resource_type(), "Patient");
    assert_eq!(distilled.resource_id(), "pat-simd-777");

    let val = distilled.to_value();
    assert_eq!(val["resourceType"], "Patient");
    assert_eq!(val["id"], "pat-simd-777");
    assert_eq!(val["name"], "Curie, Marie");
    assert_eq!(val["gender"], "female");
    assert_eq!(val["birthDate"], "1867-11-07");
}

#[test]
fn test_simd_distilled_observation_to_value() {
    let mut obs_raw = br#"{
        "resourceType": "Observation",
        "id": "obs-simd-888",
        "status": "final",
        "code": {
            "coding": [{"system": "http://loinc.org", "code": "29463-7", "display": "Body Weight"}],
            "text": "Body Weight"
        },
        "valueQuantity": {
            "value": 68.5,
            "unit": "kg"
        },
        "effectiveDateTime": "2026-10-05T09:00:00Z"
    }"#
    .to_vec();

    let distilled = distill_resource_simd(&mut obs_raw, DetailLevel::Standard)
        .expect("simd distillation should succeed");

    assert_eq!(distilled.resource_type(), "Observation");
    assert_eq!(distilled.resource_id(), "obs-simd-888");

    let val = distilled.to_value();
    assert_eq!(val["resourceType"], "Observation");
    assert_eq!(val["id"], "obs-simd-888");
    assert_eq!(val["code"], "29463-7");
    assert_eq!(val["value"], 68.5);
    assert_eq!(val["unit"], "kg");
}

#[test]
fn test_distill_raw_slice_convenience_api() {
    let mut raw_bytes = br#"{
        "resourceType": "Patient",
        "id": "pat-slice-01",
        "name": [{"text": "Ada Lovelace"}],
        "gender": "female"
    }"#
    .to_vec();

    let val = distill_raw_slice(&mut raw_bytes, DetailLevel::Compact)
        .expect("distill_raw_slice must succeed");

    assert_eq!(val["resourceType"], "Patient");
    assert_eq!(val["id"], "pat-slice-01");
    assert_eq!(val["name"], "Ada Lovelace");
    assert_eq!(val["gender"], "female");
}
