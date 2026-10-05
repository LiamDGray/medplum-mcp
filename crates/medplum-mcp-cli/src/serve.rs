//! Server execution logic for `serve` and `mock-server` subcommands.

use std::sync::{Arc, Mutex};

use medplum_mcp_core::audit::AuditLogManager;
use medplum_mcp_server::client::MedplumClient;
use medplum_mcp_server::mcp::McpServer;
use medplum_mcp_server::mock_server::create_mock_router;
use medplum_mcp_server::sandbox::ClinicalSandbox;
use tokio::net::TcpListener;
use tracing::info;

use crate::cli::{AuditFormat, MockServerArgs, ServeArgs, Transport};

/// Run the MCP server with stdio or SSE transport.
pub async fn run_serve(args: ServeArgs) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = if args.demo {
        info!("Initializing St. Jude Children's Research Hospital synthetic demo sandbox");
        MedplumClient::new_demo_default()
    } else {
        info!("Connecting to Medplum FHIR backend at {}", args.base_url);
        MedplumClient::new_live(&args.base_url, None)
    };

    let audit_manager = if let Some(audit_path) = args.audit_log {
        info!(
            "Enabling HIPAA HMAC-SHA256 audit flight recorder at {:?} (format: {:?})",
            audit_path, args.audit_format
        );
        let key = args.audit_key.as_bytes();
        let mgr = match args.audit_format {
            AuditFormat::Jsonl => AuditLogManager::new(audit_path, key)?,
            AuditFormat::Binary => {
                let bin_path = if audit_path.extension().is_some_and(|ext| ext == "jsonl") {
                    audit_path.with_extension("bin")
                } else {
                    audit_path
                };
                AuditLogManager::new_binary(bin_path, key)?
            }
            AuditFormat::Dual => {
                let is_bin = audit_path.extension().is_some_and(|ext| ext == "bin");
                let (jsonl_path, bin_path) = if is_bin {
                    (audit_path.with_extension("jsonl"), audit_path)
                } else {
                    (audit_path.clone(), audit_path.with_extension("bin"))
                };
                AuditLogManager::new_dual(jsonl_path, bin_path, key)?
            }
        };
        Some(Arc::new(Mutex::new(mgr)))
    } else {
        None
    };

    let server = McpServer::new(client, audit_manager, args.allow_writes);

    match args.transport {
        Transport::Stdio => {
            info!("Starting Medplum MCP server on stdio transport");
            server
                .run_stdio(tokio::io::stdin(), tokio::io::stdout())
                .await?;
        }
        Transport::Sse => {
            let bind_addr = format!("{}:{}", args.host, args.port);
            info!("Starting Medplum MCP SSE server on http://{}", bind_addr);
            eprintln!("Medplum MCP SSE server listening on http://{}", bind_addr);
            let listener = TcpListener::bind(&bind_addr).await?;
            let app = server.into_router();
            axum::serve(listener, app).await?;
        }
    }

    Ok(())
}

/// Run the OpenAPI 3.0 mock HTTP server.
pub async fn run_mock_server(
    args: MockServerArgs,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let bind_addr = format!("{}:{}", args.host, args.port);
    info!(
        "Starting OpenAPI 3.0 FHIR R4 mock server on http://{}",
        bind_addr
    );
    eprintln!(
        "OpenAPI 3.0 FHIR R4 mock server listening on http://{}",
        bind_addr
    );

    let sandbox = ClinicalSandbox::default();
    let app = create_mock_router(sandbox);
    let listener = TcpListener::bind(&bind_addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
