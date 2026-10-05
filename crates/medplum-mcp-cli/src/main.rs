//! Medplum MCP Rust CLI binary entrypoint (`medplum-mcp-rs`).

use clap::Parser;
use medplum_mcp_cli::bench::run_micro_benchmarks;
use medplum_mcp_cli::cli::{Cli, Commands, OutputFormat};
use medplum_mcp_cli::config::{generate_client_config, install_client_config};
use medplum_mcp_cli::serve::{run_mock_server, run_serve};
use medplum_mcp_cli::verify::run_verification;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Direct all tracing logs to stderr to preserve stdout for MCP JSON-RPC
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,medplum_mcp=info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Serve(args) => {
            run_serve(args).await?;
        }
        Commands::MockServer(args) => {
            run_mock_server(args).await?;
        }
        Commands::Verify(args) => {
            let report_res = run_verification(&args);
            match report_res {
                Ok(report) => {
                    match args.output {
                        OutputFormat::Terminal => report.render_terminal(),
                        OutputFormat::Json => println!("{}", report.to_json()),
                        OutputFormat::Markdown => println!("{}", report.to_markdown()),
                    }
                    if args.strict && !report.success {
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    eprintln!("Verification error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::Config(args) => {
            if args.install {
                match install_client_config(&args, None) {
                    Ok(path) => {
                        eprintln!(
                            "Successfully configured and installed {:?} configuration to {}",
                            args.client,
                            path.display()
                        );
                    }
                    Err(e) => {
                        eprintln!("Failed to install configuration: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                match generate_client_config(&args) {
                    Ok(config_str) => {
                        print!("{}", config_str);
                    }
                    Err(e) => {
                        eprintln!("Failed to generate configuration: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
        Commands::Bench(args) => match run_micro_benchmarks(&args) {
            Ok(report) => {
                if args.json {
                    println!("{}", report.to_json());
                } else {
                    report.render_terminal();
                }
            }
            Err(e) => {
                eprintln!("Benchmark failed: {}", e);
                std::process::exit(1);
            }
        },
        Commands::Soak(args) => {
            medplum_mcp_cli::soak::run_soak_test(&args)?;
        }
        Commands::Tui(args) => {
            medplum_mcp_cli::tui::run_tui_app(&args)?;
        }
        Commands::Openapi(args) => {
            let sandbox = medplum_mcp_server::sandbox::ClinicalSandbox::new_st_jude();
            let client = medplum_mcp_server::client::MedplumClient::new_demo(sandbox);
            let server = medplum_mcp_server::mcp::McpServer::new(client, None, false);
            let spec = server.export_openapi_spec(args.overlay);
            let json_str = serde_json::to_string_pretty(&spec)?;
            if let Some(path) = &args.output {
                std::fs::write(path, &json_str)?;
                eprintln!(
                    "Successfully exported OpenAPI 3.1 specification to {}",
                    path.display()
                );
            } else {
                println!("{}", json_str);
            }
        }
    }

    Ok(())
}
