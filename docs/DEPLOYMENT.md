# Deployment & Operational Engineering Guide

> **Target Environments**: Claude Desktop, Cursor, Cloud Kubernetes, Docker, Bare-Metal Linux  
> **Transports**: Standard I/O (`stdio`), Server-Sent Events (`sse`), Linux Zero-Copy Sockets  

---

## 1. Local Agent Integrations (Stdio Transport)

The stdio transport runs as a child process launched directly by your AI coding assistant or desktop agent.

### Claude Desktop
Add to `~/.config/Claude/claude_desktop_config.json` (Linux) or `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS):

```json
{
  "mcpServers": {
    "medplum": {
      "command": "medplum-mcp-rs",
      "args": ["serve", "--demo", "--transport", "stdio"]
    }
  }
}
```

### Cursor & Windsurf
Add to `~/.cursor/mcp.json` or `~/.codeium/windsurf/mcp_config.json`:

```json
{
  "mcpServers": {
    "medplum": {
      "command": "medplum-mcp-rs",
      "args": ["serve", "--demo", "--transport", "stdio"]
    }
  }
}
```

### One-Click Automated Installer
Use the built-in configuration generator to merge settings automatically without destroying other server configs:

```bash
# Install to Claude Desktop:
medplum-mcp-rs config --client claude-desktop --install

# Install to Cursor with live production credentials:
medplum-mcp-rs config --client cursor --install

# Generate all 7 client configs to stdout:
medplum-mcp-rs config --all
```

---

## 2. Cloud-Native & Container Deployment (SSE Transport)

For centralized team deployments, multi-tenant agent platforms, or Kubernetes clusters, run `medplum-mcp-rs` in Server-Sent Events (SSE) mode:

```bash
medplum-mcp-rs serve \
  --transport sse \
  --host 0.0.0.0 \
  --port 8000 \
  --demo
```

### Endpoints
* `GET /sse`: Server-Sent Events stream for downstream client connection.
* `POST /message`: HTTP POST endpoint receiving client JSON-RPC 2.0 requests.
* `GET /healthz`: Health check endpoint returning HTTP 200 OK.

---

## 3. High-Performance Linux Kernel Tuning
 
To achieve maximum throughput and low latency in production pipe and socket operations:

1. **Socket Buffer Sizing**:
   ```bash
   sudo sysctl -w net.core.rmem_max=16777216
   sudo sysctl -w net.core.wmem_max=16777216
   ```
2. **Pipe Buffer Limits**:
   ```bash
   sudo sysctl -w fs.pipe-max-size=1048576
   ```

---

## 4. Observability & Telemetry

* **Live Terminal UI Dashboard**:
  ```bash
  medplum-mcp-rs tui --demo
  ```
* **Cryptographic Verification**:
  ```bash
  medplum-mcp-rs verify --strict
  ```
* **Microsecond Benchmarking**:
  ```bash
  medplum-mcp-rs bench
  ```
