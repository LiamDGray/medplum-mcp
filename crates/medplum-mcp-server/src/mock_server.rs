//! In-Process OpenAPI FHIR R4 Mock HTTP Server.
//!
//! Provides standard FHIR R4 REST endpoints, OAuth2 token issuance,
//! rate limit headers, and safety invariant validation via Axum.

use std::collections::HashMap;
use std::net::SocketAddr;

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use medplum_mcp_core::safety::SafetyViolationError;
use serde_json::{json, Value};
use tokio::net::TcpListener;

use crate::sandbox::ClinicalSandbox;

/// Errors that may occur when starting or running the mock server.
#[derive(thiserror::Error, Debug)]
pub enum ServerError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Handle to a running in-process FHIR mock server.
pub struct MockServerHandle {
    addr: SocketAddr,
    task: tokio::task::JoinHandle<()>,
}

impl MockServerHandle {
    /// Bound socket address.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Bound port number.
    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    /// Base URL string (e.g. `http://127.0.0.1:12345`).
    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Gracefully abort the server task.
    pub fn shutdown(self) {
        self.task.abort();
    }
}

impl Drop for MockServerHandle {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn standard_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("X-Rate-Limit-Limit", HeaderValue::from_static("100"));
    headers.insert("X-Rate-Limit-Remaining", HeaderValue::from_static("99"));
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/fhir+json; charset=utf-8"),
    );
    headers
}

fn operation_outcome(status: StatusCode, code: &str, diagnostics: &str) -> Response {
    let outcome = json!({
        "resourceType": "OperationOutcome",
        "issue": [{
            "severity": "error",
            "code": code,
            "diagnostics": diagnostics
        }]
    });
    (status, standard_headers(), Json(outcome)).into_response()
}

// ---------------------------------------------------------------------------
// Route Handlers
// ---------------------------------------------------------------------------

async fn oauth_token() -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );

    let body = json!({
        "access_token": "mock-token",
        "token_type": "Bearer",
        "expires_in": 7200,
        "scope": "user/*.*"
    });

    (StatusCode::OK, headers, Json(body))
}

async fn search_patients(
    State(sandbox): State<ClinicalSandbox>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let name = params.get("name").map(|s| s.as_str());
    let identifier = params.get("identifier").map(|s| s.as_str());
    let dob = params
        .get("birthdate")
        .or_else(|| params.get("dob"))
        .map(|s| s.as_str());

    let patients = sandbox.search_patients(name, identifier, dob);
    let bundle = sandbox.to_bundle(patients);

    (StatusCode::OK, standard_headers(), Json(bundle))
}

async fn get_patient(
    State(sandbox): State<ClinicalSandbox>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match sandbox.get_patient(&id) {
        Some(pat) => (StatusCode::OK, standard_headers(), Json(pat)).into_response(),
        None => operation_outcome(
            StatusCode::NOT_FOUND,
            "not-found",
            &format!("Patient/{} not found", id),
        ),
    }
}

async fn list_observations(
    State(sandbox): State<ClinicalSandbox>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let patient = params
        .get("patient")
        .or_else(|| params.get("subject"))
        .map(|s| s.as_str());
    let category = params.get("category").map(|s| s.as_str());
    let code = params.get("code").map(|s| s.as_str());

    let items = sandbox.list_observations(patient, category, code);
    let bundle = sandbox.to_bundle(items);

    (StatusCode::OK, standard_headers(), Json(bundle))
}

async fn create_observation(
    State(sandbox): State<ClinicalSandbox>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    match sandbox.create_observation_resource(payload) {
        Ok(created) => {
            let mut headers = standard_headers();
            if let Some(id) = created.get("id").and_then(Value::as_str) {
                if let Ok(loc) = HeaderValue::from_str(&format!("/fhir/R4/Observation/{}", id)) {
                    headers.insert(header::LOCATION, loc);
                }
            }
            (StatusCode::CREATED, headers, Json(created)).into_response()
        }
        Err(SafetyViolationError::ForbiddenTerminalStatus { raw_status, .. }) => operation_outcome(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invariant-violation",
            &format!("Forbidden clinical terminal status: '{}'", raw_status),
        ),
        Err(err) => operation_outcome(StatusCode::BAD_REQUEST, "invalid-request", &err.to_string()),
    }
}

async fn list_medications(
    State(sandbox): State<ClinicalSandbox>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let patient = params
        .get("patient")
        .or_else(|| params.get("subject"))
        .map(|s| s.as_str());

    let items = sandbox.list_medications(patient);
    let bundle = sandbox.to_bundle(items);

    (StatusCode::OK, standard_headers(), Json(bundle))
}

async fn create_medication_request(
    State(sandbox): State<ClinicalSandbox>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    match sandbox.create_medication_resource(payload) {
        Ok(created) => {
            let mut headers = standard_headers();
            if let Some(id) = created.get("id").and_then(Value::as_str) {
                if let Ok(loc) =
                    HeaderValue::from_str(&format!("/fhir/R4/MedicationRequest/{}", id))
                {
                    headers.insert(header::LOCATION, loc);
                }
            }
            (StatusCode::CREATED, headers, Json(created)).into_response()
        }
        Err(SafetyViolationError::ForbiddenTerminalStatus { raw_status, .. }) => operation_outcome(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invariant-violation",
            &format!("Forbidden clinical terminal status: '{}'", raw_status),
        ),
        Err(err) => operation_outcome(StatusCode::BAD_REQUEST, "invalid-request", &err.to_string()),
    }
}

async fn list_conditions(
    State(sandbox): State<ClinicalSandbox>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let patient = params
        .get("patient")
        .or_else(|| params.get("subject"))
        .map(|s| s.as_str());

    let items = sandbox.list_conditions(patient);
    let bundle = sandbox.to_bundle(items);

    (StatusCode::OK, standard_headers(), Json(bundle))
}

async fn list_allergies(
    State(sandbox): State<ClinicalSandbox>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let patient = params
        .get("patient")
        .or_else(|| params.get("subject"))
        .map(|s| s.as_str());

    let items = sandbox.list_allergies(patient);
    let bundle = sandbox.to_bundle(items);

    (StatusCode::OK, standard_headers(), Json(bundle))
}

async fn list_diagnostic_reports(
    State(sandbox): State<ClinicalSandbox>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let patient = params
        .get("patient")
        .or_else(|| params.get("subject"))
        .map(|s| s.as_str());
    let code = params.get("code").map(|s| s.as_str());

    let items = sandbox.list_diagnostic_reports(patient, code);
    let bundle = sandbox.to_bundle(items);

    (StatusCode::OK, standard_headers(), Json(bundle))
}

async fn list_encounters(
    State(sandbox): State<ClinicalSandbox>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let patient = params
        .get("patient")
        .or_else(|| params.get("subject"))
        .map(|s| s.as_str());

    let items = sandbox.list_encounters(patient);
    let bundle = sandbox.to_bundle(items);

    (StatusCode::OK, standard_headers(), Json(bundle))
}

async fn list_care_plans(
    State(sandbox): State<ClinicalSandbox>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let patient = params
        .get("patient")
        .or_else(|| params.get("subject"))
        .map(|s| s.as_str());

    let items = sandbox.list_care_plans(patient);
    let bundle = sandbox.to_bundle(items);

    (StatusCode::OK, standard_headers(), Json(bundle))
}

// ---------------------------------------------------------------------------
// Server Setup
// ---------------------------------------------------------------------------

/// Create the Axum application router configured with all FHIR R4 mock routes.
pub fn create_mock_router(sandbox: ClinicalSandbox) -> Router {
    Router::new()
        .route("/oauth2/token", post(oauth_token))
        .route("/fhir/R4/Patient", get(search_patients))
        .route("/fhir/R4/Patient/{id}", get(get_patient))
        .route(
            "/fhir/R4/Observation",
            get(list_observations).post(create_observation),
        )
        .route(
            "/fhir/R4/MedicationRequest",
            get(list_medications).post(create_medication_request),
        )
        .route("/fhir/R4/Condition", get(list_conditions))
        .route("/fhir/R4/AllergyIntolerance", get(list_allergies))
        .route("/fhir/R4/DiagnosticReport", get(list_diagnostic_reports))
        .route("/fhir/R4/Encounter", get(list_encounters))
        .route("/fhir/R4/CarePlan", get(list_care_plans))
        .with_state(sandbox)
}

/// Spin up an in-process Axum HTTP mock server on `127.0.0.1:{port}`.
pub async fn start_mock_server(
    port: u16,
    sandbox: ClinicalSandbox,
) -> Result<MockServerHandle, ServerError> {
    let bind_addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&bind_addr).await?;
    let local_addr = listener.local_addr()?;

    let app = create_mock_router(sandbox);

    let task = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("Mock server error: {:?}", e);
        }
    });

    Ok(MockServerHandle {
        addr: local_addr,
        task,
    })
}
