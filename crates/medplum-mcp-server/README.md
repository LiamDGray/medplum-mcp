# medplum-mcp-server

High-performance Model Context Protocol (MCP) server for Medplum FHIR with Streamable HTTP and zero-copy IPC.

## Features

- **MCP Protocol Conformance**: 100% compliant with official `@modelcontextprotocol/conformance` test suite. Supports both JSON-RPC stdio transport and modern Streamable HTTP (`/mcp`, `/sse`, `/message`) with UUID session tracking and local DNS rebinding defenses.
- **Zero-Copy Local IPC**: High-throughput pipe splicing (`splice(2)` / `vmsplice(2)`) moving pages directly across kernel pipe buffers without user-space buffer churn.
- **Clinical Sandbox & Native FHIR**: Embedded pediatric oncology clinical dataset (St. Jude Children's Research Hospital ALL protocol) providing instant, reproducible, zero-latency sandbox testing with full FHIR R4 schema coverage.
- **OAuth 2.1 & RFC 9728 Integration**: Built-in support for OAuth 2.1 client credentials, Authorization Code flow with PKCE, and RFC 9728 OAuth 2.0 Protected Resource Metadata (`/.well-known/oauth-protected-resource`).
- **Bidirectional Human-in-the-Loop Elicitation**: Dedicated `PhysicianWitness` safety gate preventing autonomous commitment of clinical orders.

## License

Licensed under Apache-2.0.
