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
│                   ENTERPRISE COMMERCIAL OFFERINGS                      │
├──────────────────────────────────┬─────────────────────────────────────┤
│   CORE BSL 1.1 EDITION           │       ENTERPRISE COMMERCIAL         │
│   (Open-Source / Evaluation)     │       (Commercial DMSA License)     │
├──────────────────────────────────┼─────────────────────────────────────┤
│ • Pure Python & Fast Rust Engine │ 🔒 HIPAA Business Associate         │
│ • 3-Tier FHIR Token Distillation │    Agreement (BAA) Execution        │
│ • Draft-Only Mutations & Gate    │ 🔒 Enterprise SLA (99.95% Uptime)   │
│ • Local HMAC-SHA256 Flight Log   │    & 24/7/365 Incident Response     │
│ • St. Jude Sandbox & Mock Server │ 🔒 Full IP Indemnification          │
│ • CLI Verifier & Benchmarks      │ 🔒 Custom EHR Integrations          │
│ • 60 FPS Ratatui Terminal TUI    │    (Epic, Cerner, On-Premise Medplum│
│                                  │ 🔒 Dedicated Clinical Engineering   │
│                                  │    Support & Architecture Review    │
└──────────────────────────────────┴─────────────────────────────────────┘
```

### 1. Enterprise Service Level Agreement (SLA) & Critical Support
- Guaranteed 99.95% production uptime SLA with 1-hour critical response times for Sev-1 operational incidents.
- Dedicated clinical engineering escalation channel with direct access to core maintainers.
- Coordinated release planning, patch testing, and zero-downtime migration guidance.

### 2. HIPAA Business Associate Agreement (BAA)
- Legally binding HIPAA Business Associate Agreement executed directly with your covered entity or health system.
- Formal security posture attestations aligned with HIPAA Security Rule 45 CFR § 164.312 (Audit Controls and Transmission Security).

### 3. Intellectual Property Defense & Full Indemnification
- Comprehensive, uncapped defense and indemnity against third-party copyright, patent, trade secret, or intellectual property claims.
- Commercial warranty ensuring license predictability for mission-critical enterprise infrastructure.

### 4. Custom EHR Integrations & Deployment Engineering
- Tailored connector engineering for proprietary hospital EHR environments (Epic on FHIR, Oracle Cerner Millennium, bespoke on-premise Medplum clusters).
- Deployment architecture reviews for high-security VPCs, Kubernetes clusters, and air-gapped clinical intranets.

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
