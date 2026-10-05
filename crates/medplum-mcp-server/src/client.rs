//! Resilient Medplum FHIR Client with In-Memory Sandbox and Context Distillation.
//!
//! Supports zero-latency demo mode, robust HTTP retries on 429/503, and transparent
//! 3-tier FHIR token distillation.

use std::time::Duration;

use medplum_mcp_core::safety::{assert_write_permitted, SafetyViolationError};
use medplum_mcp_core::secret::SecretString;
use medplum_mcp_core::token_diet::{distill_resource, DetailLevel};
use serde_json::{json, Value};

use crate::sandbox::ClinicalSandbox;

/// Errors returned by `MedplumClient`.
#[derive(thiserror::Error, Debug)]
pub enum ClientError {
    #[error("Safety invariant violation: {0}")]
    SafetyViolation(#[from] SafetyViolationError),

    #[error("Write operations are disabled. Configure allow_writes=true.")]
    WritesBlocked,

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Server error ({status}): {message}")]
    ServerError { status: u16, message: String },

    #[error("JSON serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// High-assurance Medplum FHIR Client.
#[derive(Clone, Debug)]
pub struct MedplumClient {
    base_url: String,
    access_token: Option<SecretString>,
    demo_mode: bool,
    allow_writes: bool,
    sandbox: Option<ClinicalSandbox>,
    http_client: reqwest::Client,
    max_retries: usize,
    backoff_ms: u64,
}

impl MedplumClient {
    /// Create client operating in zero-latency demo mode with the provided sandbox.
    pub fn new_demo(sandbox: ClinicalSandbox) -> Self {
        Self {
            base_url: "http://demo.local".to_string(),
            access_token: None,
            demo_mode: true,
            allow_writes: true,
            sandbox: Some(sandbox),
            http_client: reqwest::Client::new(),
            max_retries: 3,
            backoff_ms: 100,
        }
    }

    /// Create client operating in demo mode with a fresh St. Jude sandbox.
    pub fn new_demo_default() -> Self {
        Self::new_demo(ClinicalSandbox::new_st_jude())
    }

    /// Create client operating in live HTTP mode.
    pub fn new_live(base_url: impl Into<String>, access_token: Option<SecretString>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            access_token,
            demo_mode: false,
            allow_writes: false, // Default to read-only for safety
            sandbox: None,
            http_client: reqwest::Client::new(),
            max_retries: 3,
            backoff_ms: 100,
        }
    }

    /// Configure whether mutating write operations are permitted.
    pub fn with_allow_writes(mut self, allow: bool) -> Self {
        self.allow_writes = allow;
        self
    }

    /// Configure maximum retry attempts for 429/503 responses.
    pub fn with_max_retries(mut self, max_retries: usize) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Configure base backoff in milliseconds for retries.
    pub fn with_backoff_ms(mut self, backoff_ms: u64) -> Self {
        self.backoff_ms = backoff_ms;
        self
    }

    /// Access the underlying sandbox if in demo mode.
    pub fn sandbox(&self) -> Option<&ClinicalSandbox> {
        self.sandbox.as_ref()
    }

    fn apply_distillation(&self, val: Value, detail: Option<DetailLevel>) -> Value {
        match detail {
            Some(level) => distill_resource(&val, level),
            None => val,
        }
    }

    // -----------------------------------------------------------------------
    // Resilient HTTP Execution (Live mode)
    // -----------------------------------------------------------------------

    async fn execute_live_request(
        &self,
        method: reqwest::Method,
        path: &str,
        query: Option<&[(&str, &str)]>,
        body: Option<&Value>,
    ) -> Result<reqwest::Response, ClientError> {
        let url = format!("{}/{}", self.base_url, path.trim_start_matches('/'));

        let mut attempt = 0;
        loop {
            let mut req = self.http_client.request(method.clone(), &url);
            if let Some(token) = &self.access_token {
                req = req.header("Authorization", format!("Bearer {}", token.expose_secret()));
            }
            req = req.header("Accept", "application/fhir+json, application/json");
            req = req.header("Content-Type", "application/fhir+json; charset=utf-8");

            if let Some(q) = query {
                req = req.query(q);
            }
            if let Some(b) = body {
                req = req.json(b);
            }

            let resp = req.send().await?;
            let status = resp.status();

            if (status == reqwest::StatusCode::TOO_MANY_REQUESTS
                || status == reqwest::StatusCode::SERVICE_UNAVAILABLE)
                && attempt < self.max_retries
            {
                attempt += 1;
                let delay = Duration::from_millis(self.backoff_ms * (1 << attempt));
                tokio::time::sleep(delay).await;
                continue;
            }

            return Ok(resp);
        }
    }

    // -----------------------------------------------------------------------
    // Patient queries
    // -----------------------------------------------------------------------

    pub async fn get_patient(
        &self,
        id: &str,
        detail: Option<DetailLevel>,
    ) -> Result<Option<Value>, ClientError> {
        if self.demo_mode {
            let sb = self.sandbox.as_ref().expect("sandbox present in demo mode");
            let pat = sb
                .get_patient(id)
                .map(|p| self.apply_distillation(p, detail));
            return Ok(pat);
        }

        let resp = self
            .execute_live_request(
                reqwest::Method::GET,
                &format!("/fhir/R4/Patient/{}", id.trim_start_matches("Patient/")),
                None,
                None,
            )
            .await?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(ClientError::ServerError {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }

        let body: Value = resp.json().await?;
        Ok(Some(self.apply_distillation(body, detail)))
    }

    pub async fn search_patients(
        &self,
        name: Option<&str>,
        identifier: Option<&str>,
        dob: Option<&str>,
        detail: Option<DetailLevel>,
    ) -> Result<Value, ClientError> {
        if self.demo_mode {
            let sb = self.sandbox.as_ref().expect("sandbox present in demo mode");
            let pats = sb.search_patients(name, identifier, dob);
            let bundle = sb.to_bundle(pats);
            return Ok(self.apply_distillation(bundle, detail));
        }

        let mut query_params: Vec<(&str, &str)> = Vec::new();
        if let Some(n) = name {
            query_params.push(("name", n));
        }
        if let Some(i) = identifier {
            query_params.push(("identifier", i));
        }
        if let Some(d) = dob {
            query_params.push(("birthdate", d));
        }

        let resp = self
            .execute_live_request(
                reqwest::Method::GET,
                "/fhir/R4/Patient",
                Some(&query_params),
                None,
            )
            .await?;

        if !resp.status().is_success() {
            return Err(ClientError::ServerError {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }

        let body: Value = resp.json().await?;
        Ok(self.apply_distillation(body, detail))
    }

    // -----------------------------------------------------------------------
    // Observation methods
    // -----------------------------------------------------------------------

    pub async fn get_observation(
        &self,
        id: &str,
        detail: Option<DetailLevel>,
    ) -> Result<Option<Value>, ClientError> {
        let clean_id = id.trim_start_matches("Observation/");
        if self.demo_mode {
            let sb = self.sandbox.as_ref().expect("sandbox present in demo mode");
            let item = sb
                .get_observation(clean_id)
                .map(|v| self.apply_distillation(v, detail));
            return Ok(item);
        }

        let resp = self
            .execute_live_request(
                reqwest::Method::GET,
                &format!("/fhir/R4/Observation/{}", clean_id),
                None,
                None,
            )
            .await?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(ClientError::ServerError {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }

        let body: Value = resp.json().await?;
        Ok(Some(self.apply_distillation(body, detail)))
    }

    pub async fn list_observations(
        &self,
        patient_id: Option<&str>,
        category: Option<&str>,
        code: Option<&str>,
        detail: Option<DetailLevel>,
    ) -> Result<Value, ClientError> {
        if self.demo_mode {
            let sb = self.sandbox.as_ref().expect("sandbox present in demo mode");
            let items = sb.list_observations(patient_id, category, code);
            let bundle = sb.to_bundle(items);
            return Ok(self.apply_distillation(bundle, detail));
        }

        let mut query: Vec<(&str, &str)> = Vec::new();
        if let Some(pat) = patient_id {
            query.push(("patient", pat));
        }
        if let Some(cat) = category {
            query.push(("category", cat));
        }
        if let Some(c) = code {
            query.push(("code", c));
        }

        let resp = self
            .execute_live_request(
                reqwest::Method::GET,
                "/fhir/R4/Observation",
                Some(&query),
                None,
            )
            .await?;

        if !resp.status().is_success() {
            return Err(ClientError::ServerError {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }

        let body: Value = resp.json().await?;
        Ok(self.apply_distillation(body, detail))
    }

    pub async fn create_observation_draft(
        &self,
        patient_id: &str,
        code: &str,
        value: f64,
        unit: &str,
        display: &str,
    ) -> Result<Value, ClientError> {
        if !self.allow_writes {
            return Err(ClientError::WritesBlocked);
        }

        let draft = json!({
            "resourceType": "Observation",
            "status": "preliminary",
            "code": {
                "coding": [{
                    "system": "http://loinc.org",
                    "code": code,
                    "display": display
                }],
                "text": display
            },
            "subject": {
                "reference": format!("Patient/{}", patient_id.trim_start_matches("Patient/"))
            },
            "valueQuantity": {
                "value": value,
                "unit": unit
            }
        });

        self.create_observation(draft).await
    }

    pub async fn create_observation(&self, resource: Value) -> Result<Value, ClientError> {
        if !self.allow_writes {
            return Err(ClientError::WritesBlocked);
        }
        assert_write_permitted("Observation", &resource, true)?;

        if self.demo_mode {
            let sb = self.sandbox.as_ref().expect("sandbox present in demo mode");
            let saved = sb.create_observation_resource(resource)?;
            return Ok(saved);
        }

        let resp = self
            .execute_live_request(
                reqwest::Method::POST,
                "/fhir/R4/Observation",
                None,
                Some(&resource),
            )
            .await?;

        if !resp.status().is_success() {
            return Err(ClientError::ServerError {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }

        let body: Value = resp.json().await?;
        Ok(body)
    }

    // -----------------------------------------------------------------------
    // MedicationRequest methods
    // -----------------------------------------------------------------------

    pub async fn get_medication(
        &self,
        id: &str,
        detail: Option<DetailLevel>,
    ) -> Result<Option<Value>, ClientError> {
        let clean_id = id.trim_start_matches("MedicationRequest/");
        if self.demo_mode {
            let sb = self.sandbox.as_ref().expect("sandbox present in demo mode");
            let item = sb
                .get_medication(clean_id)
                .map(|v| self.apply_distillation(v, detail));
            return Ok(item);
        }

        let resp = self
            .execute_live_request(
                reqwest::Method::GET,
                &format!("/fhir/R4/MedicationRequest/{}", clean_id),
                None,
                None,
            )
            .await?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(ClientError::ServerError {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }

        let body: Value = resp.json().await?;
        Ok(Some(self.apply_distillation(body, detail)))
    }

    pub async fn list_medications(
        &self,
        patient_id: Option<&str>,
        detail: Option<DetailLevel>,
    ) -> Result<Value, ClientError> {
        if self.demo_mode {
            let sb = self.sandbox.as_ref().expect("sandbox present in demo mode");
            let items = sb.list_medications(patient_id);
            let bundle = sb.to_bundle(items);
            return Ok(self.apply_distillation(bundle, detail));
        }

        let mut query: Vec<(&str, &str)> = Vec::new();
        if let Some(pat) = patient_id {
            query.push(("patient", pat));
        }

        let resp = self
            .execute_live_request(
                reqwest::Method::GET,
                "/fhir/R4/MedicationRequest",
                Some(&query),
                None,
            )
            .await?;

        if !resp.status().is_success() {
            return Err(ClientError::ServerError {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }

        let body: Value = resp.json().await?;
        Ok(self.apply_distillation(body, detail))
    }

    pub async fn create_medication_draft(
        &self,
        patient_id: &str,
        med_code: &str,
        dosage: &str,
        instructions: &str,
        display: &str,
    ) -> Result<Value, ClientError> {
        if !self.allow_writes {
            return Err(ClientError::WritesBlocked);
        }

        let draft = json!({
            "resourceType": "MedicationRequest",
            "status": "draft",
            "intent": "order",
            "subject": {
                "reference": format!("Patient/{}", patient_id.trim_start_matches("Patient/"))
            },
            "medicationCodeableConcept": {
                "coding": [{
                    "system": "http://www.nlm.nih.gov/research/umls/rxnorm",
                    "code": med_code,
                    "display": display
                }],
                "text": display
            },
            "dosageInstruction": [{
                "text": format!("{}: {}", dosage, instructions)
            }]
        });

        self.create_medication_request(draft).await
    }

    pub async fn create_medication_request(&self, resource: Value) -> Result<Value, ClientError> {
        if !self.allow_writes {
            return Err(ClientError::WritesBlocked);
        }
        assert_write_permitted("MedicationRequest", &resource, true)?;

        if self.demo_mode {
            let sb = self.sandbox.as_ref().expect("sandbox present in demo mode");
            let saved = sb.create_medication_resource(resource)?;
            return Ok(saved);
        }

        let resp = self
            .execute_live_request(
                reqwest::Method::POST,
                "/fhir/R4/MedicationRequest",
                None,
                Some(&resource),
            )
            .await?;

        if !resp.status().is_success() {
            return Err(ClientError::ServerError {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }

        let body: Value = resp.json().await?;
        Ok(body)
    }

    // -----------------------------------------------------------------------
    // Condition queries
    // -----------------------------------------------------------------------

    pub async fn get_condition(
        &self,
        id: &str,
        detail: Option<DetailLevel>,
    ) -> Result<Option<Value>, ClientError> {
        let clean_id = id.trim_start_matches("Condition/");
        if self.demo_mode {
            let sb = self.sandbox.as_ref().expect("sandbox present in demo mode");
            let item = sb
                .get_condition(clean_id)
                .map(|v| self.apply_distillation(v, detail));
            return Ok(item);
        }

        let resp = self
            .execute_live_request(
                reqwest::Method::GET,
                &format!("/fhir/R4/Condition/{}", clean_id),
                None,
                None,
            )
            .await?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(ClientError::ServerError {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }

        let body: Value = resp.json().await?;
        Ok(Some(self.apply_distillation(body, detail)))
    }

    pub async fn list_conditions(
        &self,
        patient_id: Option<&str>,
        detail: Option<DetailLevel>,
    ) -> Result<Value, ClientError> {
        if self.demo_mode {
            let sb = self.sandbox.as_ref().expect("sandbox present in demo mode");
            let items = sb.list_conditions(patient_id);
            let bundle = sb.to_bundle(items);
            return Ok(self.apply_distillation(bundle, detail));
        }

        let mut query: Vec<(&str, &str)> = Vec::new();
        if let Some(pat) = patient_id {
            query.push(("patient", pat));
        }

        let resp = self
            .execute_live_request(
                reqwest::Method::GET,
                "/fhir/R4/Condition",
                Some(&query),
                None,
            )
            .await?;

        let body: Value = resp.json().await?;
        Ok(self.apply_distillation(body, detail))
    }

    // -----------------------------------------------------------------------
    // AllergyIntolerance queries
    // -----------------------------------------------------------------------

    pub async fn get_allergy(
        &self,
        id: &str,
        detail: Option<DetailLevel>,
    ) -> Result<Option<Value>, ClientError> {
        let clean_id = id.trim_start_matches("AllergyIntolerance/");
        if self.demo_mode {
            let sb = self.sandbox.as_ref().expect("sandbox present in demo mode");
            let item = sb
                .get_allergy(clean_id)
                .map(|v| self.apply_distillation(v, detail));
            return Ok(item);
        }

        let resp = self
            .execute_live_request(
                reqwest::Method::GET,
                &format!("/fhir/R4/AllergyIntolerance/{}", clean_id),
                None,
                None,
            )
            .await?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(ClientError::ServerError {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }

        let body: Value = resp.json().await?;
        Ok(Some(self.apply_distillation(body, detail)))
    }

    pub async fn list_allergies(
        &self,
        patient_id: Option<&str>,
        detail: Option<DetailLevel>,
    ) -> Result<Value, ClientError> {
        if self.demo_mode {
            let sb = self.sandbox.as_ref().expect("sandbox present in demo mode");
            let items = sb.list_allergies(patient_id);
            let bundle = sb.to_bundle(items);
            return Ok(self.apply_distillation(bundle, detail));
        }

        let mut query: Vec<(&str, &str)> = Vec::new();
        if let Some(pat) = patient_id {
            query.push(("patient", pat));
        }

        let resp = self
            .execute_live_request(
                reqwest::Method::GET,
                "/fhir/R4/AllergyIntolerance",
                Some(&query),
                None,
            )
            .await?;

        let body: Value = resp.json().await?;
        Ok(self.apply_distillation(body, detail))
    }

    // -----------------------------------------------------------------------
    // DiagnosticReport queries
    // -----------------------------------------------------------------------

    pub async fn get_diagnostic_report(
        &self,
        id: &str,
        detail: Option<DetailLevel>,
    ) -> Result<Option<Value>, ClientError> {
        let clean_id = id.trim_start_matches("DiagnosticReport/");
        if self.demo_mode {
            let sb = self.sandbox.as_ref().expect("sandbox present in demo mode");
            let item = sb
                .get_diagnostic_report(clean_id)
                .map(|v| self.apply_distillation(v, detail));
            return Ok(item);
        }

        let resp = self
            .execute_live_request(
                reqwest::Method::GET,
                &format!("/fhir/R4/DiagnosticReport/{}", clean_id),
                None,
                None,
            )
            .await?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(ClientError::ServerError {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }

        let body: Value = resp.json().await?;
        Ok(Some(self.apply_distillation(body, detail)))
    }

    pub async fn list_diagnostic_reports(
        &self,
        patient_id: Option<&str>,
        code: Option<&str>,
        detail: Option<DetailLevel>,
    ) -> Result<Value, ClientError> {
        if self.demo_mode {
            let sb = self.sandbox.as_ref().expect("sandbox present in demo mode");
            let items = sb.list_diagnostic_reports(patient_id, code);
            let bundle = sb.to_bundle(items);
            return Ok(self.apply_distillation(bundle, detail));
        }

        let mut query: Vec<(&str, &str)> = Vec::new();
        if let Some(pat) = patient_id {
            query.push(("patient", pat));
        }
        if let Some(c) = code {
            query.push(("code", c));
        }

        let resp = self
            .execute_live_request(
                reqwest::Method::GET,
                "/fhir/R4/DiagnosticReport",
                Some(&query),
                None,
            )
            .await?;

        let body: Value = resp.json().await?;
        Ok(self.apply_distillation(body, detail))
    }

    // -----------------------------------------------------------------------
    // Encounter queries
    // -----------------------------------------------------------------------

    pub async fn get_encounter(
        &self,
        id: &str,
        detail: Option<DetailLevel>,
    ) -> Result<Option<Value>, ClientError> {
        let clean_id = id.trim_start_matches("Encounter/");
        if self.demo_mode {
            let sb = self.sandbox.as_ref().expect("sandbox present in demo mode");
            let item = sb
                .get_encounter(clean_id)
                .map(|v| self.apply_distillation(v, detail));
            return Ok(item);
        }

        let resp = self
            .execute_live_request(
                reqwest::Method::GET,
                &format!("/fhir/R4/Encounter/{}", clean_id),
                None,
                None,
            )
            .await?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(ClientError::ServerError {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }

        let body: Value = resp.json().await?;
        Ok(Some(self.apply_distillation(body, detail)))
    }

    pub async fn list_encounters(
        &self,
        patient_id: Option<&str>,
        detail: Option<DetailLevel>,
    ) -> Result<Value, ClientError> {
        if self.demo_mode {
            let sb = self.sandbox.as_ref().expect("sandbox present in demo mode");
            let items = sb.list_encounters(patient_id);
            let bundle = sb.to_bundle(items);
            return Ok(self.apply_distillation(bundle, detail));
        }

        let mut query: Vec<(&str, &str)> = Vec::new();
        if let Some(pat) = patient_id {
            query.push(("patient", pat));
        }

        let resp = self
            .execute_live_request(
                reqwest::Method::GET,
                "/fhir/R4/Encounter",
                Some(&query),
                None,
            )
            .await?;

        let body: Value = resp.json().await?;
        Ok(self.apply_distillation(body, detail))
    }

    // -----------------------------------------------------------------------
    // CarePlan queries
    // -----------------------------------------------------------------------

    pub async fn get_care_plan(
        &self,
        id: &str,
        detail: Option<DetailLevel>,
    ) -> Result<Option<Value>, ClientError> {
        let clean_id = id.trim_start_matches("CarePlan/");
        if self.demo_mode {
            let sb = self.sandbox.as_ref().expect("sandbox present in demo mode");
            let item = sb
                .get_care_plan(clean_id)
                .map(|v| self.apply_distillation(v, detail));
            return Ok(item);
        }

        let resp = self
            .execute_live_request(
                reqwest::Method::GET,
                &format!("/fhir/R4/CarePlan/{}", clean_id),
                None,
                None,
            )
            .await?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(ClientError::ServerError {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }

        let body: Value = resp.json().await?;
        Ok(Some(self.apply_distillation(body, detail)))
    }

    pub async fn list_care_plans(
        &self,
        patient_id: Option<&str>,
        detail: Option<DetailLevel>,
    ) -> Result<Value, ClientError> {
        if self.demo_mode {
            let sb = self.sandbox.as_ref().expect("sandbox present in demo mode");
            let items = sb.list_care_plans(patient_id);
            let bundle = sb.to_bundle(items);
            return Ok(self.apply_distillation(bundle, detail));
        }

        let mut query: Vec<(&str, &str)> = Vec::new();
        if let Some(pat) = patient_id {
            query.push(("patient", pat));
        }

        let resp = self
            .execute_live_request(
                reqwest::Method::GET,
                "/fhir/R4/CarePlan",
                Some(&query),
                None,
            )
            .await?;

        let body: Value = resp.json().await?;
        Ok(self.apply_distillation(body, detail))
    }
}
