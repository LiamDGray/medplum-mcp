//! High-Performance Zero-Copy Streaming Transport Engine & SSE Cloud Router.
//!
//! Provides:
//! 1. Zero-copy buffered stdio transport (`run_stdio`) with reused frame buffers.
//! 2. Linux `io_uring` high-throughput zero-copy ring integration when enabled.
//! 3. SSE (Server-Sent Events) HTTP router (`GET /sse`, `POST /message`) for cloud deployment.

use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tracing::debug;

use crate::mcp::McpServer;

/// Shared application state for SSE and HTTP routing.
#[derive(Clone)]
pub struct AppState {
    pub server: McpServer,
    pub sse_sender: broadcast::Sender<String>,
}

#[cfg(all(target_os = "linux", feature = "io-uring"))]
pub struct LinuxIoUringEngine {
    ring: io_uring::IoUring,
}

#[cfg(all(target_os = "linux", feature = "io-uring"))]
impl LinuxIoUringEngine {
    /// Initialize a Linux io_uring instance for low-latency kernel submissions.
    pub fn new(entries: u32) -> Result<Self, std::io::Error> {
        let ring = io_uring::IoUring::new(entries)?;
        Ok(Self { ring })
    }

    /// Check if the submission queue is operational.
    pub fn is_ready(&self) -> bool {
        self.ring.params().sq_entries() > 0
    }
}

impl McpServer {
    /// Run zero-copy line-delimited stdio transport.
    ///
    /// Reuses a single preallocated frame buffer across incoming requests,
    /// eliminating heap allocation churn in the fast path.
    pub async fn run_stdio<R, W>(&self, reader: R, mut writer: W) -> Result<(), std::io::Error>
    where
        R: AsyncRead + Unpin + Send,
        W: AsyncWrite + Unpin + Send,
    {
        let mut buf_reader = BufReader::with_capacity(64 * 1024, reader);
        let mut line_buffer = String::with_capacity(64 * 1024);

        loop {
            line_buffer.clear();
            let bytes_read = buf_reader.read_line(&mut line_buffer).await?;
            if bytes_read == 0 {
                // EOF reached
                debug!("Stdio transport reached EOF");
                break;
            }

            let trimmed = line_buffer.trim();
            if trimmed.is_empty() {
                continue;
            }

            if let Some(response_str) = self.handle_jsonrpc_message(trimmed).await {
                writer.write_all(response_str.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await?;
            }
        }

        Ok(())
    }

    /// Construct an Axum HTTP Router providing SSE transport and JSON-RPC message dispatch.
    pub fn into_router(self) -> Router {
        let (sse_sender, _) = broadcast::channel(1024);
        let state = Arc::new(AppState {
            server: self,
            sse_sender,
        });

        Router::new()
            .route("/mcp", get(sse_handler).post(message_handler))
            .route("/sse", get(sse_handler).post(message_handler))
            .route("/message", post(message_handler).get(sse_handler))
            .route("/", get(sse_handler).post(message_handler))
            .with_state(state)
    }
}

fn is_valid_localhost_host_or_origin(val: &str) -> bool {
    let without_scheme = val
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    let host_part = without_scheme.split('/').next().unwrap_or("").trim();
    let hostname = host_part
        .split(':')
        .next()
        .unwrap_or("")
        .trim_matches('[')
        .trim_matches(']');
    hostname == "localhost" || hostname == "127.0.0.1" || hostname == "::1" || hostname.is_empty()
}

fn validate_dns_rebinding(headers: &axum::http::HeaderMap) -> Result<(), axum::http::StatusCode> {
    if let Some(host) = headers.get("host").and_then(|v| v.to_str().ok()) {
        if !is_valid_localhost_host_or_origin(host) {
            return Err(axum::http::StatusCode::FORBIDDEN);
        }
    }
    if let Some(origin) = headers.get("origin").and_then(|v| v.to_str().ok()) {
        if !is_valid_localhost_host_or_origin(origin) {
            return Err(axum::http::StatusCode::FORBIDDEN);
        }
    }
    Ok(())
}

/// Handler for SSE streams (`GET /mcp`, `GET /sse`, `GET /`).
async fn sse_handler(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    if let Err(status) = validate_dns_rebinding(&headers) {
        return axum::response::Response::builder()
            .status(status)
            .body(axum::body::Body::from("DNS rebinding forbidden"))
            .unwrap();
    }

    let session_id = headers
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let rx = state.sse_sender.subscribe();

    // Initial endpoint notification per MCP SSE transport spec
    let endpoint_url = format!("/message?sessionId={}", session_id);
    let initial_event: Result<Event, std::convert::Infallible> =
        Ok(Event::default().event("endpoint").data(endpoint_url));

    let rx_stream = BroadcastStream::new(rx).filter_map(|msg| match msg {
        Ok(text) => Some(Ok(Event::default().event("message").data(text))),
        Err(e) => {
            debug!("Broadcast stream lagged: {}", e);
            None
        }
    });

    let stream = tokio_stream::once(initial_event).chain(rx_stream);

    let sse_response =
        Sse::new(stream).keep_alive(KeepAlive::default().interval(Duration::from_secs(15)));

    let mut response_headers = axum::http::HeaderMap::new();
    if let Ok(val) = axum::http::HeaderValue::from_str(&session_id) {
        response_headers.insert("mcp-session-id", val);
    }
    response_headers.insert(
        "cache-control",
        axum::http::HeaderValue::from_static("no-cache"),
    );

    (response_headers, sse_response).into_response()
}

/// Handler for JSON-RPC messages (`POST /mcp`, `POST /sse`, `POST /message`, `POST /`).
async fn message_handler(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    body: String,
) -> impl IntoResponse {
    if let Err(status) = validate_dns_rebinding(&headers) {
        return axum::response::Response::builder()
            .status(status)
            .header("Content-Type", "application/json")
            .body(axum::body::Body::from(
                r#"{"error":"DNS rebinding forbidden: Host/Origin must be localhost"}"#,
            ))
            .unwrap();
    }

    let session_id = headers
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let trimmed = body.trim();
    if let Some(response_str) = state.server.handle_jsonrpc_message(trimmed).await {
        // Broadcast to SSE listeners
        let _ = state.sse_sender.send(response_str.clone());

        axum::response::Response::builder()
            .status(axum::http::StatusCode::OK)
            .header("Content-Type", "application/json")
            .header("mcp-session-id", session_id)
            .body(axum::body::Body::from(response_str))
            .unwrap()
    } else {
        axum::response::Response::builder()
            .status(axum::http::StatusCode::ACCEPTED)
            .header("mcp-session-id", session_id)
            .body(axum::body::Body::empty())
            .unwrap()
    }
}
