# medplum-mcp-core

High-assurance FHIR R4 token distillation, affine typestates, and zero-copy cryptographic audit vault for Medplum MCP.

## Features

- **3-Tier Context Distillation**: Extreme token diet for FHIR R4 resources (`Compact`, `Standard`, `Executive`), reducing LLM context window consumption by up to 88% while preserving all critical clinical semantics.
- **Affine Safety Typestates**: Non-bypassable compile-time state machines enforcing Zero Unauthorized Commitment invariants. Draft resources cannot be transitioned into legally binding active orders without an explicit, cryptographically verifiable `PhysicianWitness` signature.
- **Dual-Storage Zero-Copy Audit Vault**: Immutable, append-only flight recorder combining human-readable JSONL with byte-aligned binary frame headers transmutable via `zerocopy` with zero memory copies.
- **SIMD Acceleration**: SIMD-accelerated string distillation and boundary scanning via runtime AVX2/AVX-512 detection.
- **Formal Verification**: Continuous verification via Kani Bounded Model Checking, Miri Undefined Behavior & Provenance analysis, and libFuzzer coverage-guided mutations.

## License

Licensed under Apache-2.0.
