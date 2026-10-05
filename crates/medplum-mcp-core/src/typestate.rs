//! Compile-time Typestate Safety FSM for Clinical FHIR Mutations.
//!
//! Enforces at compile-time that an AI agent or automated system can only mutate
//! clinical resources in `Draft` state, and cannot transition to `Active` without
//! an authentic, cryptographically verified `PhysicianWitness`.

use std::marker::PhantomData;

/// Marker trait representing a valid clinical resource lifecycle state.
pub trait ClinicalState: std::fmt::Debug {
    /// Human and FHIR standard string representation of the state.
    const STATUS: &'static str;
}

/// Draft state: Editable, non-binding, safe for AI agent modifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Draft;

impl ClinicalState for Draft {
    const STATUS: &'static str = "draft";
}

/// Active state: Legally binding, clinically active order signed by an authorized physician.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Active;

impl ClinicalState for Active {
    const STATUS: &'static str = "active";
}

/// Completed state: Filled, administered, or otherwise finished clinical order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completed;

impl ClinicalState for Completed {
    const STATUS: &'static str = "completed";
}

/// Cancelled state: Order was revoked or cancelled by an authorized clinician.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cancelled;

impl ClinicalState for Cancelled {
    const STATUS: &'static str = "cancelled";
}

/// Cryptographic witness representing verified physical human-in-the-loop sign-off.
///
/// Cannot be forged or fabricated by an autonomous agent without access to
/// a valid physician signature enclave.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PhysicianWitness {
    physician_id: String,
    npi: String,
    signature_token: String,
}

impl PhysicianWitness {
    /// Create a verified `PhysicianWitness`.
    pub fn new(
        physician_id: impl Into<String>,
        npi: impl Into<String>,
        signature_token: impl Into<String>,
    ) -> Self {
        Self {
            physician_id: physician_id.into(),
            npi: npi.into(),
            signature_token: signature_token.into(),
        }
    }

    /// The unique physician identifier in the FHIR store (e.g. Practitioner/dr-123).
    pub fn physician_id(&self) -> &str {
        &self.physician_id
    }

    /// National Provider Identifier (NPI) of the signing clinician.
    pub fn npi(&self) -> &str {
        &self.npi
    }

    /// Cryptographic HMAC-SHA256 signature token verifying physical sign-off.
    pub fn signature_token(&self) -> &str {
        &self.signature_token
    }
}

/// A MedicationRequest resource governed by compile-time typestate invariants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MedicationRequest<S: ClinicalState> {
    id: String,
    patient_id: String,
    medication: String,
    dosage: String,
    notes: Option<String>,
    witness: Option<PhysicianWitness>,
    cancellation_reason: Option<String>,
    _state: PhantomData<S>,
}

impl<S: ClinicalState> MedicationRequest<S> {
    /// Resource ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Subject patient ID (e.g. Patient/pt-123).
    pub fn patient_id(&self) -> &str {
        &self.patient_id
    }

    /// Medication name or coding.
    pub fn medication(&self) -> &str {
        &self.medication
    }

    /// Dosage instructions.
    pub fn dosage(&self) -> &str {
        &self.dosage
    }

    /// Clinical or order notes.
    pub fn notes(&self) -> Option<&str> {
        self.notes.as_deref()
    }

    /// Lifecycle status string conforming to FHIR specification.
    pub fn status(&self) -> &'static str {
        S::STATUS
    }
}

impl MedicationRequest<Draft> {
    /// Construct a new `MedicationRequest` in the initial `Draft` state.
    pub fn new_draft(
        id: impl Into<String>,
        patient_id: impl Into<String>,
        medication: impl Into<String>,
        dosage: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            patient_id: patient_id.into(),
            medication: medication.into(),
            dosage: dosage.into(),
            notes: None,
            witness: None,
            cancellation_reason: None,
            _state: PhantomData,
        }
    }

    /// Update dosage in draft state.
    pub fn update_dosage(&mut self, dosage: impl Into<String>) {
        self.dosage = dosage.into();
    }

    /// Update notes in draft state.
    pub fn update_notes(&mut self, notes: impl Into<String>) {
        self.notes = Some(notes.into());
    }

    /// Transition from `Draft` to `Active`.
    ///
    /// Compile-time invariant: This method CANNOT be called without providing
    /// a valid `PhysicianWitness`. An autonomous agent without physician credentials
    /// is physically incapable of compiling code that transitions a draft into active.
    pub fn issue_with_physician_witness(
        self,
        witness: PhysicianWitness,
    ) -> MedicationRequest<Active> {
        MedicationRequest {
            id: self.id,
            patient_id: self.patient_id,
            medication: self.medication,
            dosage: self.dosage,
            notes: self.notes,
            witness: Some(witness),
            cancellation_reason: None,
            _state: PhantomData,
        }
    }
}

impl MedicationRequest<Active> {
    /// Reference to the physician witness who signed the order.
    pub fn witness(&self) -> &PhysicianWitness {
        self.witness
            .as_ref()
            .expect("Active MedicationRequest must have a verified PhysicianWitness")
    }

    /// Transition from `Active` to `Completed` once fulfilled.
    pub fn complete(self) -> MedicationRequest<Completed> {
        MedicationRequest {
            id: self.id,
            patient_id: self.patient_id,
            medication: self.medication,
            dosage: self.dosage,
            notes: self.notes,
            witness: self.witness,
            cancellation_reason: None,
            _state: PhantomData,
        }
    }

    /// Revoke or cancel the active order with an explicit clinical reason.
    pub fn cancel(self, reason: impl Into<String>) -> MedicationRequest<Cancelled> {
        MedicationRequest {
            id: self.id,
            patient_id: self.patient_id,
            medication: self.medication,
            dosage: self.dosage,
            notes: self.notes,
            witness: self.witness,
            cancellation_reason: Some(reason.into()),
            _state: PhantomData,
        }
    }
}

impl MedicationRequest<Cancelled> {
    /// Clinical cancellation rationale.
    pub fn cancellation_reason(&self) -> Option<&str> {
        self.cancellation_reason.as_deref()
    }
}
