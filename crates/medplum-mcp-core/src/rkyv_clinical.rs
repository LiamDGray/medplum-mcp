//! Zero-Deserialization Clinical Cache using rkyv
//!
//! Provides zero-copy archiving and zero-deserialization field access for HL7 FHIR clinical datasets.
//! Clinical datasets (Patients, Observations, MedicationRequests) can be accessed directly
//! from raw byte memory without heap allocations or struct reconstructions.

use rkyv::{Archive, Deserialize, Serialize};

/// Patient record with rkyv zero-copy serialization support.
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub struct PatientRecord {
    pub id: String,
    pub name: String,
    pub gender: String,
    pub birth_date: String,
}

/// Type alias matching ArchivedPatient for clinical cache access.
pub type ArchivedPatient = ArchivedPatientRecord;

/// Observation record with rkyv zero-copy serialization support.
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub struct ObservationRecord {
    pub id: String,
    pub patient_id: String,
    pub code: String,
    pub code_display: String,
    pub value: f64,
    pub unit: String,
}

/// Type alias matching ArchivedObservation for clinical cache access.
pub type ArchivedObservation = ArchivedObservationRecord;

/// MedicationRequest record with rkyv zero-copy serialization support.
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub struct MedicationRecord {
    pub id: String,
    pub patient_id: String,
    pub medication_name: String,
    pub status: String,
    pub dosage: String,
}

/// Type alias matching ArchivedMedicationRequest for clinical cache access.
pub type ArchivedMedicationRequest = ArchivedMedicationRecord;

/// Full clinical dataset archive containing patient, observation, and medication collections.
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub struct ClinicalDataset {
    pub organization_id: String,
    pub patients: Vec<PatientRecord>,
    pub observations: Vec<ObservationRecord>,
    pub medication_requests: Vec<MedicationRecord>,
}

/// Archive a clinical dataset into zero-deserialization rkyv binary bytes.
pub fn archive_clinical_dataset(data: &ClinicalDataset) -> Result<Vec<u8>, rkyv::rancor::Error> {
    let aligned_vec = rkyv::to_bytes::<rkyv::rancor::Error>(data)?;
    Ok(aligned_vec.to_vec())
}

/// Access archived clinical dataset directly from raw byte slice without heap deserialization.
pub fn access_archived_dataset(
    bytes: &[u8],
) -> Result<&ArchivedClinicalDataset, rkyv::rancor::Error> {
    rkyv::access::<ArchivedClinicalDataset, rkyv::rancor::Error>(bytes)
}
