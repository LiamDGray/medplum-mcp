# Security Architecture & Cryptographic Integrity

> **Standard**: Health AI Model Context Protocol (HAMCP) Security Specification  
> **Compliance Target**: HIPAA 45 CFR § 164.312 (Security Standards for Protection of e-PHI)  
> **Revision**: October 2026  

---

## 1. Zero-Leak Credential Isolation

The `medplum-mcp` server enforces strict zero-leak boundaries to guarantee that sensitive authentication tokens, cryptographic keys, and patient identifiers never escape into Large Language Model (LLM) context windows or error logs:

1. **`SecretString` Enclave Wrapping**:
   - All client secrets, bearer tokens, and HMAC signing keys are wrapped in `SecretString` types.
   - Standard formatting implementations (`fmt::Display`, `fmt::Debug`, `serde::Serialize`) unconditionally output masked literals: `***` or `SecretString("***")`.
   - Access to plaintext secrets is isolated behind explicit methods (`.expose_secret()`) requiring intentional developer action.
2. **Multi-Vault Resolution**:
   - Credentials are never stored in plaintext configuration files.
   - The runtime dynamically resolves secrets through 1Password CLI, AWS Secrets Manager, or HashiCorp Vault.
3. **Payload Sanitization**:
   - Outbound tool arguments and JSON-RPC responses are scrubbed for authorization headers, bearer tokens, and private URIs before serialization.

---

## 2. HIPAA § 164.312(b) Cryptographic Flight Recorder

In compliance with federal audit control requirements, every tool execution, mutation request, and security intercept is recorded in a tamper-evident audit ledger:

```
┌────────────────────────────────────────────────────────────────────────┐
│             HMAC-SHA256 CHAINED AUDIT LOG ARCHITECTURE                 │
├───────────────┬────────────────────────────────────────────────────────┤
│ Sequence ID   │ Monotonically increasing 64-bit integer (1, 2, 3...)   │
│ Timestamp     │ UTC ISO-8601 millisecond timestamp                    │
│ Action Status │ ALLOWED (0), BLOCKED (1), ERROR (2)                    │
│ Tool Name     │ Name of invoked FastMCP clinical tool                  │
│ Payload Hash  │ SHA-256 digest of input parameters and context         │
│ Prev Sig      │ HMAC-SHA256 signature of the preceding log entry       │
│ Signature     │ HMAC-SHA256(Key, Seq : Time : Status : Tool : Hash : Prev)│
└───────────────┴────────────────────────────────────────────────────────┘
```

* **Genesis Anchor**: The initial entry is cryptographically anchored to a constant genesis signature ($0^{64}$).
* **Hash Chaining**: Altering, reordering, deleting, or injecting any record invalidates all subsequent signatures across the entire chain.
* **Independent Verification**: The CLI subcommand `medplum-mcp-rs verify --strict` recalculates all cryptographic signatures from genesis to verify log integrity in linear $O(N)$ time.

---

## 3. Formal State Machine & Memory Invariants

1. **Compile-Time Affine Typestates**:
   - Mutating clinical orders from `Draft` to `Active` is structurally impossible without presenting an unforgeable `PhysicianWitness` capability token.
   - AI agents cannot forge capability tokens at compile time.
2. **Dynamic Runtime Safety Gate (`assert_write_permitted`)**:
   - Blocks any mutation attempting terminal or irrevocable states (`active`, `completed`, `cancelled`, `final`).
   - Normalizes input strings via Unicode NFKC normalization, preventing homoglyph evasion attacks (e.g. Cyrillic `а` or full-width `ａ` substituted for ASCII `a`).
3. **Formal Verification Proofs**:
   - Inductive safety proven in Lean 4 ([`formal/MedplumSafety.lean`](../formal/MedplumSafety.lean) and [`formal/ZeroCopyCorrectness.lean`](../formal/ZeroCopyCorrectness.lean)).
   - Concurrency temporal logic specified in TLA+ ([`formal/fhir_safety.tla`](../formal/fhir_safety.tla) and [`formal/zerocopy_concurrency.tla`](../formal/zerocopy_concurrency.tla)).
   - Automated SMT bounded model checking proving `UNSAT` terminal reachability across all 5 clinical state machines.

---

## 4. Vulnerability Disclosure & Incident Response

We take security vulnerabilities seriously. If you discover a potential vulnerability in `medplum-mcp`:

* **Reporting Email**: `security@medplum-mcp.org`
* **PGP Encryption**: Encrypt reports using our public PGP security key (fingerprint available on request).
* **Response SLA**: Initial triage and acknowledgment within 24 hours; critical security patch and CVE advisory issued within 72 hours.
* **Coordinated Disclosure**: We adhere to responsible 90-day coordinated vulnerability disclosure guidelines.
