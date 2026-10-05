# medplum-mcp-cli

CLI entrypoint, TUI dashboard, continuous soak harness, and multi-client config generator for Medplum MCP.

## Features

- **Multi-Transport Server Launch**: Start the Medplum MCP server in `stdio` or `sse` (Streamable HTTP) transport mode with `--demo`, `--allow-writes`, and custom port/audit configuration.
- **Continuous Soak Harness**: Long-duration multi-threaded stress and soak test harness verifying RSS leak-free performance, mutex contention, and JSON-RPC dispatch across billions of cycles.
- **Interactive TUI Dashboard**: Real-time terminal interface displaying audit flight logs, token diet distillation ratios, throughput, and system resource metrics.
- **Config Generator**: Automated configuration generation for major AI desktop and developer environments including Claude Desktop, Cursor, Zed, Cline, and Continue.
- **Verification & Audit Inspector**: Command-line verification of cryptographic audit trails and cryptographic tamper detection.

## Binary Installation

```bash
cargo install medplum-mcp-cli
```

The installed binary is `medplum-mcp-rs`.

## License

Licensed under Apache-2.0.
