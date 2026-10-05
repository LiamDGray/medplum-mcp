//! Medplum MCP Server
//!
//! Production-grade FastMCP server, in-memory clinical sandbox,
//! resilient FHIR R4 client, OpenAPI mock server, and zero-copy streaming engine.

pub mod client;
pub mod mcp;
pub mod mock_server;
pub mod network_zero_copy;
pub mod sandbox;
pub mod splice_transport;
pub mod streaming;

// Re-exports for convenience
pub use client::{ClientError, MedplumClient};
pub use mcp::{JsonRpcError, JsonRpcRequest, JsonRpcResponse, McpError, McpServer};
pub use mock_server::{create_mock_router, start_mock_server, MockServerHandle, ServerError};
pub use network_zero_copy::{
    determine_zerocopy_strategy, enable_ktls_if_supported, enable_so_zerocopy_if_supported,
    send_clinical_payload, sendfile_to_socket, ZeroCopyStrategy, DYNAMIC_ZEROCOPY_THRESHOLD,
};
pub use sandbox::ClinicalSandbox;
pub use splice_transport::{SplicePipe, SpliceTransport};
