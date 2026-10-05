# Enterprise Licensing & Commercial Governance

> **Repository**: `medplum-mcp`  
> **Licensing Model**: Business Source License (BSL 1.1) transitioning to Apache 2.0 on a 4-year sunset  
> **Target Audience**: Hospital Systems, Healthtech Unicorns, Pharmaceutical Sponsors, and Regulated Life-Sciences Enterprises  

---

## 1. Dual-Licensing Architecture

`medplum-mcp` is governed under a dual-licensing structure designed to balance open ecosystem adoption with sustainable enterprise monetization:

1. **Open-Source / Evaluation Edition (BSL 1.1)**:
   - Free for non-production evaluation, development, research, and single-clinic deployments below the commercial active patient threshold.
   - Converts automatically to Apache 2.0 four years after initial release.
   - Includes full access to the high-performance dual-stack server (Python & zero-copy Rust), 3-tier token distillation, draft-only mutation safety gates, local HMAC-SHA256 flight recorder, St. Jude sandbox, OpenAPI mock server, CLI verifier, and the 60 FPS Ratatui terminal dashboard.

2. **Enterprise Commercial License (DMSA / Production Deployment)**:
   - Required for production clinical deployment across health systems, multi-hospital federations, commercial EHR integrations, or revenue-generating clinical AI applications.
   - Granted under an Enterprise Direct Master Services Agreement (DMSA) with custom scopes based on active patient volume, tenant count, and high-availability requirements.

---

## 2. Enterprise Commercial Entitlements

Organizations deploying `medplum-mcp` in enterprise clinical production receive specialized commercial extensions and enterprise-grade guarantees:

```
┌────────────────────────────────────────────────────────────────────────┐
│                   ENTERPRISE COMMERCIAL ENTITLEMENTS                   │
├──────────────────────────────────┬─────────────────────────────────────┤
│   CORE BSL 1.1 EDITION           │       ENTERPRISE COMMERCIAL         │
│   (Open-Source / Evaluation)     │       (Commercial DMSA License)     │
├──────────────────────────────────┼─────────────────────────────────────┤
│ • Pure Python & Zero-Copy Rust   │ 🔒 Dynamic PHI Safe Harbor          │
│ • 3-Tier FHIR Token Distillation │    De-Identification Engine         │
│ • Draft-Only Mutations           │ 🔒 Ed25519 Hardware Physician       │
│ • Local HMAC-SHA256 Flight Log   │    Witness & DEA EPCS Ingestion     │
│ • St. Jude Sandbox & Mock Server │ 🔒 Multi-Tenant Merkle DAG          │
│ • CLI Verifier & Benchmarks      │    Audit Cloud Replication          │
│ • 60 FPS Ratatui Terminal TUI    │ 🔒 Multi-Vault Secret Enclave       │
│                                  │    (AWS KMS, HashiCorp Vault DMSA)  │
│                                  │ 🔒 Enterprise SLA, BAA, & Indemnity │
└──────────────────────────────────┴─────────────────────────────────────┘
```

### 1. Dynamic PHI Safe Harbor De-Identification & Differential Privacy Engine
- In-memory, sub-millisecond masking and date-shifting complying strictly with HIPAA 45 CFR § 164.514(b) Safe Harbor standards.
- Enables clinical teams and pharmaceutical researchers to connect frontier cloud LLMs (Anthropic Claude, OpenAI, Google Gemini) to real hospital datasets without exposing protected health information (PHI) or violating HIPAA privacy regulations.
- Deterministic patient-specific salt ($\pm \Delta t$) preserves clinical timeline intervals (e.g. chemotherapy cycle spacing) while eliminating re-identification risk.

### 2. Ed25519 Hardware Clinician Witness Sign-Off & DEA EPCS Compliance
- End-to-end cryptographic FSM seam enabling attending physicians to sign off on AI-drafted prescriptions and orders using physical WebAuthn / PKCS#11 hardware keys (YubiKey).
- Satisfies DEA Electronic Prescriptions for Controlled Substances (EPCS) two-factor authentication rules and state medical board human-in-the-loop mandates.
- Transitions `MedicationRequest<Draft>` to legally binding `Active` state only upon hardware signature validation.

### 3. Multi-Tenant Distributed Merkle DAG Audit Replication
- Upgrades local `.jsonl` audit ledgers into a distributed Merkle Tree DAG anchored to consortium transparency logs (RFC 6962 / Sigstore Rekor).
- Provides multi-hospital health systems with mathematically immutable, decentralized audit proof for forensic discovery and Joint Commission accreditation.

### 4. Zero-Trust Multi-Vault Cloud Enclave
- Enterprise secret resolution integrating directly with enterprise key management systems (AWS KMS, Azure Key Vault, HashiCorp Vault DMSA clusters).
- Proactive 30-day token rotation, multi-region failover, and hardware security module (HSM) backing.

---

## 3. Legal Guarantees, Compliance, & Indemnification

Enterprise Commercial Licenses include binding legal protections essential for healthcare procurement:

* **HIPAA Business Associate Agreement (BAA)**: Legally binding execution confirming compliance with 45 CFR Part 160 and Part 164 Subparts A and E.
* **Intellectual Property Indemnification**: Full uncapped defense and indemnity against third-party copyright, patent, or intellectual property claims.
* **Service Level Agreement (SLA)**: 99.99% availability guarantee with 24/7/365 Tier-1 clinical engineering support and 15-minute response times for critical severity incidents.
* **Quarterly Penetration Testing & Cryptographic Verification Reports**: Independent third-party audit reports validating memory safety, zero-leak isolation, and deterministic safety invariant verification.

---

## 4. Licensing Inquiries & Procurement

To initiate commercial evaluation, request a custom DMSA quote, or schedule an architectural security review with our clinical systems team, contact:

* **Enterprise Licensing**: `licensing@medplum-mcp.org`
* **Procurement & Compliance**: `procurement@medplum-mcp.org`
* **Security & Vulnerability Disclosure**: `security@medplum-mcp.org`

*(Commercial terms and active patient volume tiers are customized during procurement. No static consumer pricing is published to preserve value-based enterprise alignment.)*
