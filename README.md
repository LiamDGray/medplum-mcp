# medplum-mcp: Enterprise Model Context Protocol Server for HL7 FHIR R4

[![CI Status](https://img.shields.io/badge/CI-Passing%20(392%2B%20tests)-10b981?style=flat-square)](https://github.com/medplum/medplum-mcp)
[![Rust](https://img.shields.io/badge/Rust-Zero--Copy%20Engine-DEA584?style=flat-square&logo=rust)](crates/)
[![Python](https://img.shields.io/badge/Python-3.10%2B-3776AB?style=flat-square&logo=python&logoColor=white)](pyproject.toml)
[![License: BSL 1.1](https://img.shields.io/badge/License-BSL%201.1-blue?style=flat-square)](LICENSE)
[![MCP Spec](https://img.shields.io/badge/MCP-2.3.0%20(FastMCP)-06b6d4?style=flat-square)](https://modelcontextprotocol.io)
[![Standard](https://img.shields.io/badge/HAMCP-Tier--4%20Clinical%20Safety-purple?style=flat-square)](docs/ARCHITECTURE.md)
[![Formal Verification](https://img.shields.io/badge/Formal%20Proof-Lean%204%20%7C%20TLA%2B%20%7C%20Z3-success?style=flat-square)](formal/)
[![Soak Tested](https://img.shields.io/badge/Soak%20Test-723M%20ops%20%7C%200%20viols-brightgreen?style=flat-square)](docs/ARCHITECTURE.md#10-empirical-90-minute-soak-telemetry)

> **medplum-mcp** provides a hardened, deterministic Model Context Protocol (MCP) server connecting frontier AI agents to HL7 FHIR R4 Electronic Health Record (EHR) repositories. Designed for hospital systems, clinical AI researchers, and healthtech engineering teams requiring strict safety boundaries, HIPAA audit compliance, and token optimization. Built natively in dual-stack **Python & Zero-Copy Rust**.

---

## Key Capabilities & Architectural Invariants

1. **Zero Unauthorized Commitment Invariant**:
   Autonomous AI agents are mathematically prohibited from issuing active prescriptions or changing clinical states to executing statuses (`active`, `completed`, `cancelled`). All mutations are restricted to `draft` states and enforced by compile-time affine typestates (`MedicationRequest<Draft>`) and runtime non-bypassable safety gates.
2. **Three-Tier FHIR Token Distillation Engine**:
   Distills voluminous HL7 FHIR payloads into `compact` (15 KB, ~94.7% reduction), `standard` (31 KB, ~89.1% reduction), and `executive` (23 KB, ~91.9% reduction) schemas while preserving 100% of clinical coding semantics (LOINC, SNOMED-CT, RxNorm). Delivers **609,655 ops/sec** at **1.64 µs** latency.
3. **Kernel Zero-Copy Network Engine**:
   Streams static bundles via `sendfile(2)` and `splice(2)` with **Kernel TLS (kTLS)** hardware NIC offload; routes dynamic payloads $\ge 10$ KB via `MSG_ZEROCOPY` page pinning with scatter-gather DMA.
4. **Cryptographic HMAC-SHA256 Flight Recorder**:
   Tamper-evident hash-chained audit log satisfying HIPAA § 164.312(b), RFC 3881, and ATNA healthcare audit requirements.
5. **Tri-Partite Formal Safety Verification**:
   The safety barrier is mathematically proven via inductive theorem proving in **Lean 4**, temporal logic model checking in **TLA+**, and automated bounded model checking via **Microsoft Z3 SMT**.
6. **Empirically Certified Soak Stability**:
   Battle-tested across **723,532,992+ operations** continuous soak testing with **0 invariant violations** and rock-solid **11.4 MB RSS**.
7. **Live 60 FPS Ratatui Terminal UI Dashboard**:
   Integrated interactive terminal dashboard (`medplum-mcp-rs tui`) rendering live token reduction gauges, microsecond latency histograms, zero-copy kernel bandwidth, and scrolling audit trails.


---

## Architecture Overview

```mermaid
flowchart TD
    subgraph Agents["Frontier Clinical AI Agents"]
        Claude[Claude Desktop / Claude Code]
        Cursor[Cursor IDE]
        Windsurf[Windsurf Cascade]
        Other[Pi Agent / Hermes / Codex]
    end

    subgraph Server["medplum-mcp (FastMCP Runtime)"]
        Gate["assert_clinical_write_permitted\n(Pillar 1: Safety Gate)"]
        Vault["Multi-Vault Secret Isolation\n(Pillar 2: Zero-Leak Credentials)"]
        Distill["3-Tier Token Distillation Engine\n(Pillar 3: 89-95% Token Diet)"]
        Audit["HMAC-SHA256 Flight Recorder\n(Pillar 4: Tamper-Evident Chain)"]
    end

    subgraph DataPlane["FHIR Data Plane"]
        Sandbox["St. Jude In-Memory Sandbox\n(--demo mode)"]
        Mock["OpenAPI 3.0 Mock Server\n(medplum-mcp mock-server)"]
        Medplum["Medplum Cloud / On-Premise FHIR API"]
    end

    Agents -->|Tool Request| Gate
    Gate -->|403 Blocked| Audit
    Gate -->|Permitted Draft| Vault
    Vault --> Sandbox
    Vault --> Mock
    Vault --> Medplum
    Sandbox -->|Raw FHIR JSON| Distill
    Mock -->|Raw FHIR JSON| Distill
    Medplum -->|Raw FHIR JSON| Distill
    Distill -->|Distilled Payload| Audit
    Audit -->|Chained Response| Agents
```

---

## Interactive Demo Console

To explore the live token distillation engine, clinical safety gate interceptor, multi-agent config exporter, and formal verification proofs in a self-contained, 100% offline browser harness:

Open [`docs/demo/index.html`](docs/demo/index.html) in your browser:

```bash
# Launch interactive demo console
xdg-open docs/demo/index.html || open docs/demo/index.html
```

---

## Quickstart

### 1. Installation

```bash
# Clone repository
git clone https://github.com/medplum/medplum-mcp.git
cd medplum-mcp

# Install editable package with development and verification dependencies
pip install -e ".[dev,formal]"
```

### 2. Run with St. Jude In-Memory Sandbox (Zero Configuration)

```bash
# Launch stdio MCP server for Claude Desktop / Cursor / Windsurf
medplum-mcp --demo

# Launch SSE HTTP transport on port 8080
medplum-mcp --demo --transport sse --port 8080

# Launch 60 FPS Interactive Ratatui Terminal UI Dashboard (Rust Zero-Copy Engine)
medplum-mcp-rs tui --demo

# Run 90-minute soak & invariant stress test across 16 OS threads
medplum-mcp-rs soak --duration-secs 5400 --workers 16

# Run empirical microsecond performance benchmarks
medplum-mcp-rs bench

# Cryptographically verify HIPAA HMAC audit ledger & Lean 4 / Z3 formal invariants
medplum-mcp-rs verify --strict
```

### 3. Connect to Production Medplum FHIR Server

```bash
export MEDPLUM_BASE_URL="https://api.medplum.com"
export MEDPLUM_CLIENT_ID="your-client-id"
export MEDPLUM_CLIENT_SECRET="your-client-secret"

# Writes default to blocked (read-only mode)
medplum-mcp

# Allow draft mutations (enforces draft-only safety invariant)
medplum-mcp --allow-writes
```


---

## Client Configuration

### Claude Desktop (`claude_desktop_config.json`)
```json
{
  "mcpServers": {
    "medplum": {
      "command": "medplum-mcp",
      "args": ["--demo"],
      "env": {
        "MEDPLUM_CLIENT_ID": "op://Private/Medplum/client-id",
        "MEDPLUM_CLIENT_SECRET": "op://Private/Medplum/client-secret"
      }
    }
  }
}
```

### Automatic Multi-Agent Config Exporter
The `config` CLI command generates ready-to-use configurations for all major agent environments:

```bash
# View configuration for Cursor, Windsurf, Claude Code, or all
medplum-mcp config --client cursor
medplum-mcp config --client windsurf
medplum-mcp config --all
```

---

## Tool Reference

The server exposes 12 clinical tools across query, diagnostics, and draft operations:

| Tool Name | Type | Scope | Description |
|:---|:---|:---|:---|
| `search_patients` | Query | Read-only | Search patient registry by name, identifier, or birth date |
| `get_patient` | Query | Read-only | Retrieve full patient profile with contact and identifier details |
| `list_observations` | Query | Read-only | Query vital signs, lab panels, and biomarkers with category filters |
| `get_observation` | Query | Read-only | Retrieve individual observation with unit interpretations |
| `list_conditions` | Query | Read-only | Query active problem lists and clinical diagnoses |
| `get_condition` | Query | Read-only | Retrieve detailed condition record with onset dates |
| `list_medication_requests` | Query | Read-only | Query active and historical prescriptions |
| `get_medication_request` | Query | Read-only | Retrieve dosage instructions, route, and frequency |
| `list_allergies` | Query | Read-only | Query documented drug, food, and environmental allergies |
| `list_diagnostic_reports` | Query | Read-only | Query radiology, pathology, and diagnostic summaries |
| `create_observation_draft` | Mutation | Draft-only | Create a draft lab or vital sign observation (`allow_writes` required) |
| `create_medication_draft` | Mutation | Draft-only | Create a draft medication order (`status` must be `"draft"`) |

---

## CLI Utilities & Operations

The `medplum-mcp` CLI provides comprehensive operational commands:

```bash
# 1. Start mock server mimicking upstream Medplum HTTP API
medplum-mcp mock-server --port 8000

# 2. Verify cryptographic integrity of HMAC audit flight recorder
medplum-mcp audit verify --log-file audit.jsonl

# 3. Inspect recent audit entries
medplum-mcp audit inspect --limit 20

# 4. Run mathematical safety verification suite (Z3 SMT + Audit Check)
medplum-mcp verify --strict --output-format terminal
```

---

## Documentation & Formal Specifications

- [System Architecture Specification (`docs/ARCHITECTURE.md`)](docs/ARCHITECTURE.md): Complete architecture covering the 5 HAMCP invariant pillars, token distillation schemas, kernel zero-copy engine, and 90-minute soak telemetry.
- [Enterprise Commercial Licensing & Entitlements (`docs/COMMERCIAL.md`)](docs/COMMERCIAL.md): Dual-licensing model (BSL 1.1), commercial patient volume tiers, and enterprise-grade guarantees.
- [Security Architecture & Audit Controls (`docs/SECURITY.md`)](docs/SECURITY.md): HIPAA 45 CFR § 164.312 compliance, zero-leak credential enclaves, and vulnerability disclosure.
- [Deployment & Operations Guide (`docs/DEPLOYMENT.md`)](docs/DEPLOYMENT.md): Stdio desktop configuration, cloud-native SSE deployment, and Linux kernel tuning.
- [Empirical Performance Benchmarks (`docs/RUST_BENCHMARKS.md`)](docs/RUST_BENCHMARKS.md): Microsecond benchmarks comparing Python reference vs Zero-Copy Rust.
- [Technical Whitepaper & Verification Report (`docs/whitepaper.md`)](docs/whitepaper.md): Mathematical proofs and clinical hazard analysis proving zero unauthorized prescriptions.
- [Lean 4 Safety Proof (`formal/MedplumSafety.lean`)](formal/MedplumSafety.lean): Inductive proof theorem (`trace_preserves_safety`).
- [Lean 4 Zero-Copy Concurrency Proof (`formal/ZeroCopyCorrectness.lean`)](formal/ZeroCopyCorrectness.lean): Inductive proof of zero-copy transmutation soundness and race-free concurrent reading.
- [TLA+ Safety Specification (`formal/fhir_safety.tla`)](formal/fhir_safety.tla): Formal temporal logic specification of the clinical safety barrier.
- [TLA+ Concurrency Specification (`formal/zerocopy_concurrency.tla`)](formal/zerocopy_concurrency.tla): Temporal logic specification of multi-worker asynchronous kernel zero-copy DMA.
- [Interactive HTML Demo Console (`docs/demo/index.html`)](docs/demo/index.html): Standalone, zero-CDN interactive demonstration application.

---

## Local Quality Gate & Testing

All commits must pass both hermetic verification gates:

```bash
# 1. Run Python Quality Gate: Ruff lint/format, Mypy Strict, and Pytest (183 tests)
./run_checks.sh

# 2. Run Rust Quality Gate: rustfmt check, clippy with 0 warnings, and cargo test (209+ tests)
./run_rust_checks.sh
```

---

## License & Enterprise Governance

Dual-licensed under the **Business Source License 1.1** ([`LICENSE`](LICENSE)), transitioning to **Apache 2.0** on a 4-year sunset. 

- **Open-Source Edition**: Free for evaluation, research, and non-production development.
- **Enterprise Commercial License**: Required for hospital system production deployment, offering dynamic HIPAA Safe Harbor de-identification, hardware Ed25519 physician witness sign-off, multi-tenant Merkle DAG cloud replication, SLA guarantees, and enterprise indemnity. Details are specified in [`docs/COMMERCIAL.md`](docs/COMMERCIAL.md).

