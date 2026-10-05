//! Integration Tests for HTTP Mock Server Edge-Cases, Invariant Enforcements,
//! Rate Limiting Headers, and Client Retry with Exponential Backoff.
//!
//! Validates:
//! 1. HTTP 400 on malformed or invalid JSON payloads.
//! 2. HTTP 404 on unknown REST endpoints and non-existent resources.
//! 3. HTTP 422 Unprocessable Entity when creating drafts with forbidden clinical statuses.
//! 4. Consistent rate-limiting headers (X-Rate-Limit-Limit, X-Rate-Limit-Remaining).
//! 5. Resilient client retry behavior with exponential backoff on 429 / 503.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use medplum_mcp_server::client::{ClientError, MedplumClient};
use medplum_mcp_server::mock_server::start_mock_server;
use medplum_mcp_server::sandbox::ClinicalSandbox;
use serde_json::{json, Value};
use tokio::net::TcpListener;

// ===========================================================================
// 1. HTTP 400 on Invalid / Malformed JSON Payload
// ===========================================================================

#[tokio::test]
async fn test_mock_server_http_400_on_malformed_json_syntax() {
    let sandbox = ClinicalSandbox::new_st_jude();
    let server = start_mock_server(0, sandbox).await.expect("mock server");
    let http = reqwest::Client::new();

    let resp = http
        .post(format!("{}/fhir/R4/Observation", server.base_url()))
        .header("Content-Type", "application/json")
        .body("{this is definitely not valid json}")
        .send()
        .await
        .expect("response");

    // Axum returns 400 Bad Request or 422 Unprocessable Entity on invalid JSON syntax
    assert!(
        resp.status() == StatusCode::BAD_REQUEST
            || resp.status() == StatusCode::UNPROCESSABLE_ENTITY,
        "Expected 400 or 422 for malformed JSON syntax, got {}",
        resp.status()
    );
}

#[tokio::test]
async fn test_mock_server_http_400_on_invalid_payload_type() {
    let sandbox = ClinicalSandbox::new_st_jude();
    let server = start_mock_server(0, sandbox).await.expect("mock server");
    let http = reqwest::Client::new();

    // Sending a JSON array instead of an object to resource creation
    let resp = http
        .post(format!("{}/fhir/R4/Observation", server.base_url()))
        .header("Content-Type", "application/json")
        .json(&json!(["an", "array", "not", "an", "object"]))
        .send()
        .await
        .expect("response");

    assert!(
        resp.status() == StatusCode::BAD_REQUEST
            || resp.status() == StatusCode::UNPROCESSABLE_ENTITY,
        "Expected client error for invalid payload type, got {}",
        resp.status()
    );
}

// ===========================================================================
// 2. HTTP 404 on Unknown Endpoint & Missing Resource
// ===========================================================================

#[tokio::test]
async fn test_mock_server_http_404_on_unknown_endpoint() {
    let sandbox = ClinicalSandbox::new_st_jude();
    let server = start_mock_server(0, sandbox).await.expect("mock server");
    let http = reqwest::Client::new();

    let resp = http
        .get(format!("{}/fhir/R4/NonExistentResource", server.base_url()))
        .send()
        .await
        .expect("response");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_mock_server_http_404_on_root_or_unknown_path() {
    let sandbox = ClinicalSandbox::new_st_jude();
    let server = start_mock_server(0, sandbox).await.expect("mock server");
    let http = reqwest::Client::new();

    let resp = http
        .get(format!("{}/some/random/api/path", server.base_url()))
        .send()
        .await
        .expect("response");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_mock_server_http_404_on_missing_patient_id() {
    let sandbox = ClinicalSandbox::new_st_jude();
    let server = start_mock_server(0, sandbox).await.expect("mock server");
    let http = reqwest::Client::new();

    let resp = http
        .get(format!(
            "{}/fhir/R4/Patient/pat-does-not-exist-999",
            server.base_url()
        ))
        .send()
        .await
        .expect("response");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let outcome: Value = resp.json().await.unwrap();
    assert_eq!(outcome["resourceType"], "OperationOutcome");
    assert_eq!(outcome["issue"][0]["code"], "not-found");
}

// ===========================================================================
// 3. HTTP 422 Unprocessable Entity on Forbidden Clinical Status
// ===========================================================================

#[tokio::test]
async fn test_mock_server_http_422_on_observation_forbidden_status_final() {
    let sandbox = ClinicalSandbox::new_st_jude();
    let server = start_mock_server(0, sandbox).await.expect("mock server");
    let http = reqwest::Client::new();

    let forbidden_obs = json!({
        "resourceType": "Observation",
        "status": "final", // Forbidden terminal state!
        "code": {
            "coding": [{
                "system": "http://loinc.org",
                "code": "26499-4",
                "display": "Absolute Neutrophil Count"
            }]
        },
        "subject": { "reference": "Patient/pat-sj-001" },
        "valueQuantity": { "value": 1500.0, "unit": "/uL" }
    });

    let resp = http
        .post(format!("{}/fhir/R4/Observation", server.base_url()))
        .json(&forbidden_obs)
        .send()
        .await
        .expect("response");

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let outcome: Value = resp.json().await.unwrap();
    assert_eq!(outcome["resourceType"], "OperationOutcome");
    assert_eq!(outcome["issue"][0]["code"], "invariant-violation");
}

#[tokio::test]
async fn test_mock_server_http_422_on_observation_forbidden_status_amended() {
    let sandbox = ClinicalSandbox::new_st_jude();
    let server = start_mock_server(0, sandbox).await.expect("mock server");
    let http = reqwest::Client::new();

    let forbidden_obs = json!({
        "resourceType": "Observation",
        "status": "amended",
        "code": { "coding": [{ "code": "26499-4" }] },
        "subject": { "reference": "Patient/pat-sj-001" }
    });

    let resp = http
        .post(format!("{}/fhir/R4/Observation", server.base_url()))
        .json(&forbidden_obs)
        .send()
        .await
        .expect("response");

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_mock_server_http_422_on_medication_forbidden_status_active() {
    let sandbox = ClinicalSandbox::new_st_jude();
    let server = start_mock_server(0, sandbox).await.expect("mock server");
    let http = reqwest::Client::new();

    let forbidden_med = json!({
        "resourceType": "MedicationRequest",
        "status": "active", // Forbidden active state!
        "intent": "order",
        "subject": { "reference": "Patient/pat-sj-001" },
        "medicationCodeableConcept": { "text": "Mercaptopurine" }
    });

    let resp = http
        .post(format!("{}/fhir/R4/MedicationRequest", server.base_url()))
        .json(&forbidden_med)
        .send()
        .await
        .expect("response");

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let outcome: Value = resp.json().await.unwrap();
    assert_eq!(outcome["issue"][0]["code"], "invariant-violation");
}

#[tokio::test]
async fn test_mock_server_http_422_on_medication_forbidden_status_completed() {
    let sandbox = ClinicalSandbox::new_st_jude();
    let server = start_mock_server(0, sandbox).await.expect("mock server");
    let http = reqwest::Client::new();

    let forbidden_med = json!({
        "resourceType": "MedicationRequest",
        "status": "completed",
        "intent": "order",
        "subject": { "reference": "Patient/pat-sj-001" },
        "medicationCodeableConcept": { "text": "Mercaptopurine" }
    });

    let resp = http
        .post(format!("{}/fhir/R4/MedicationRequest", server.base_url()))
        .json(&forbidden_med)
        .send()
        .await
        .expect("response");

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

// ===========================================================================
// 4. Rate Limit Headers Verification Across Endpoints
// ===========================================================================

#[tokio::test]
async fn test_mock_server_rate_limit_headers_present_on_all_endpoints() {
    let sandbox = ClinicalSandbox::new_st_jude();
    let server = start_mock_server(0, sandbox).await.expect("mock server");
    let http = reqwest::Client::new();
    let base = server.base_url();

    let endpoints = [
        format!("{}/fhir/R4/Patient", base),
        format!("{}/fhir/R4/Patient/pat-sj-001", base),
        format!("{}/fhir/R4/Observation", base),
        format!("{}/fhir/R4/MedicationRequest", base),
        format!("{}/fhir/R4/Condition", base),
        format!("{}/fhir/R4/AllergyIntolerance", base),
        format!("{}/fhir/R4/DiagnosticReport", base),
        format!("{}/fhir/R4/Encounter", base),
        format!("{}/fhir/R4/CarePlan", base),
    ];

    for ep in endpoints {
        let resp = http.get(&ep).send().await.expect("response");
        assert_eq!(resp.status(), StatusCode::OK, "Failed GET {}", ep);

        let headers = resp.headers();
        let limit = headers
            .get("X-Rate-Limit-Limit")
            .and_then(|v| v.to_str().ok())
            .expect("X-Rate-Limit-Limit present");
        assert_eq!(limit, "100");

        let remaining = headers
            .get("X-Rate-Limit-Remaining")
            .and_then(|v| v.to_str().ok())
            .expect("X-Rate-Limit-Remaining present");
        assert_eq!(remaining, "99");

        let content_type = headers
            .get("Content-Type")
            .and_then(|v| v.to_str().ok())
            .expect("Content-Type present");
        assert!(content_type.contains("application/fhir+json"));
    }
}

#[tokio::test]
async fn test_mock_server_rate_limit_headers_present_on_error_responses() {
    let sandbox = ClinicalSandbox::new_st_jude();
    let server = start_mock_server(0, sandbox).await.expect("mock server");
    let http = reqwest::Client::new();

    // 404 response
    let resp_404 = http
        .get(format!("{}/fhir/R4/Patient/not-found", server.base_url()))
        .send()
        .await
        .unwrap();
    assert_eq!(resp_404.status(), StatusCode::NOT_FOUND);
    assert_eq!(resp_404.headers().get("X-Rate-Limit-Limit").unwrap(), "100");
    assert_eq!(
        resp_404.headers().get("X-Rate-Limit-Remaining").unwrap(),
        "99"
    );

    // 422 response
    let resp_422 = http
        .post(format!("{}/fhir/R4/Observation", server.base_url()))
        .json(&json!({"status": "final"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp_422.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(resp_422.headers().get("X-Rate-Limit-Limit").unwrap(), "100");
    assert_eq!(
        resp_422.headers().get("X-Rate-Limit-Remaining").unwrap(),
        "99"
    );
}

// ===========================================================================
// 5. Client Retry Behavior with Exponential Backoff on 429 / 503
// ===========================================================================

#[tokio::test]
async fn test_client_retries_and_succeeds_on_temporary_429() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_clone = attempts.clone();

    // Spawn transient 429 responder: returns 429 twice, then 200 OK
    let app = Router::new().route(
        "/fhir/R4/Patient/{id}",
        get(move |Path(id): Path<String>| {
            let att = attempts_clone.fetch_add(1, Ordering::SeqCst);
            async move {
                if att < 2 {
                    (StatusCode::TOO_MANY_REQUESTS, "Too Many Requests").into_response()
                } else {
                    let pat = json!({ "resourceType": "Patient", "id": id });
                    (StatusCode::OK, Json(pat)).into_response()
                }
            }
        }),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = MedplumClient::new_live(format!("http://{}", addr), None)
        .with_max_retries(3)
        .with_backoff_ms(2);

    let pat = client
        .get_patient("pat-sj-001", None)
        .await
        .expect("get_patient should succeed after retries")
        .expect("patient found");

    assert_eq!(pat["id"], "pat-sj-001");
    assert_eq!(attempts.load(Ordering::SeqCst), 3); // 2 failed + 1 success
}

#[tokio::test]
async fn test_client_retries_and_succeeds_on_temporary_503() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_clone = attempts.clone();

    // Spawn transient 503 responder: returns 503 once, then 200 OK
    let app = Router::new().route(
        "/fhir/R4/Patient/{id}",
        get(move |Path(id): Path<String>| {
            let att = attempts_clone.fetch_add(1, Ordering::SeqCst);
            async move {
                if att < 1 {
                    (StatusCode::SERVICE_UNAVAILABLE, "Service Unavailable").into_response()
                } else {
                    let pat = json!({ "resourceType": "Patient", "id": id });
                    (StatusCode::OK, Json(pat)).into_response()
                }
            }
        }),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = MedplumClient::new_live(format!("http://{}", addr), None)
        .with_max_retries(3)
        .with_backoff_ms(2);

    let pat = client
        .get_patient("pat-sj-002", None)
        .await
        .expect("get_patient should succeed after retrying 503")
        .expect("patient found");

    assert_eq!(pat["id"], "pat-sj-002");
    assert_eq!(attempts.load(Ordering::SeqCst), 2); // 1 failed + 1 success
}

#[tokio::test]
async fn test_client_exhausts_retries_on_persistent_503() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_clone = attempts.clone();

    // Always returns 503
    let app = Router::new().route(
        "/fhir/R4/Patient/{id}",
        get(move |Path(_id): Path<String>| {
            attempts_clone.fetch_add(1, Ordering::SeqCst);
            async move { (StatusCode::SERVICE_UNAVAILABLE, "Persistent Outage").into_response() }
        }),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let max_retries = 2;
    let client = MedplumClient::new_live(format!("http://{}", addr), None)
        .with_max_retries(max_retries)
        .with_backoff_ms(2);

    let err = client.get_patient("pat-001", None).await;
    assert!(err.is_err());
    match err.unwrap_err() {
        ClientError::ServerError { status, .. } => assert_eq!(status, 503),
        other => panic!("Expected ServerError 503, got {:?}", other),
    }

    // 1 initial attempt + 2 retries = 3 total attempts
    assert_eq!(attempts.load(Ordering::SeqCst), max_retries + 1);
}

#[tokio::test]
async fn test_client_exhausts_retries_on_persistent_429() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_clone = attempts.clone();

    let app = Router::new().route(
        "/fhir/R4/Patient/{id}",
        get(move |Path(_id): Path<String>| {
            attempts_clone.fetch_add(1, Ordering::SeqCst);
            async move { (StatusCode::TOO_MANY_REQUESTS, "Persistent Rate Limit").into_response() }
        }),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let max_retries = 2;
    let client = MedplumClient::new_live(format!("http://{}", addr), None)
        .with_max_retries(max_retries)
        .with_backoff_ms(2);

    let err = client.get_patient("pat-001", None).await;
    assert!(err.is_err());
    assert_eq!(attempts.load(Ordering::SeqCst), max_retries + 1);
}

#[tokio::test]
async fn test_client_does_not_retry_on_non_transient_client_error() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_clone = attempts.clone();

    // 401 Unauthorized should NOT be retried
    let app = Router::new().route(
        "/fhir/R4/Patient/{id}",
        get(move |Path(_id): Path<String>| {
            attempts_clone.fetch_add(1, Ordering::SeqCst);
            async move { (StatusCode::UNAUTHORIZED, "Unauthorized").into_response() }
        }),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = MedplumClient::new_live(format!("http://{}", addr), None)
        .with_max_retries(3)
        .with_backoff_ms(2);

    let err = client.get_patient("pat-001", None).await;
    assert!(err.is_err());
    // Only exactly 1 attempt should have been made
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
}
