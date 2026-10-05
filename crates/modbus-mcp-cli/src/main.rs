//! Industrial IoT Modbus Bridge MCP Server CLI executable.

use clap::Parser;
use colored::Colorize;
use modbus_mcp_cli::config::generate_client_config;
use modbus_mcp_cli::verifier::run_audit_verify;
use modbus_mcp_cli::{run_serve, run_simulate, Cli, Commands};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Serve(args) => {
            run_serve(args).await?;
        }
        Commands::Simulate(args) => {
            let report = match run_simulate(&args) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("{} {}", "Simulation failed:".red().bold(), e);
                    std::process::exit(1);
                }
            };
            println!(
                "{} Completed {} simulation steps, recorded {} audit events.",
                "✔".green().bold(),
                report.steps_executed,
                report.total_events_recorded
            );
            if let Some(path) = report.audit_log_path {
                println!("  Flight recorder log written to: {}", path.display());
            }
        }
        Commands::Config(args) => {
            let config_str = match generate_client_config(&args) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("{} {}", "Failed to generate config:".red().bold(), e);
                    std::process::exit(1);
                }
            };
            print!("{}", config_str);
        }
        Commands::AuditVerify(args) => {
            let report = match run_audit_verify(&args) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("{} {}", "Verification failed:".red().bold(), e);
                    std::process::exit(1);
                }
            };

            if report.is_valid {
                println!(
                    "{} Audit log verification SUCCESSFUL. Verified {} frames up to sequence {}.",
                    "✔".green().bold(),
                    report.verified_count,
                    report.last_sequence_id
                );
            } else {
                eprintln!(
                    "{} Audit log verification FAILED. Verified {} frames. Error: {}",
                    "✖".red().bold(),
                    report.verified_count,
                    report.error.as_deref().unwrap_or("Unknown failure")
                );
                std::process::exit(1);
            }
        }
    }

    Ok(())
}
