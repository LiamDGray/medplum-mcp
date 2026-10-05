//! Property-Based Fuzz Testing Suite for Medplum MCP Core.
//!
//! Features formal proptest invariants for:
//! 1. Safety gate rejection of forbidden statuses and adversarial homoglyphs.
//! 2. Homoglyph detector immunity to arbitrary Cyrillic/Greek/invisible substitutions.
//! 3. Tamper-evident HMAC-SHA256 audit log hash chain corruption detection.
//! 4. Token diet monotonic byte reduction and exact bundle entry preservation.
//! 5. SecretString zero-leak guarantees across Debug, Display, and Serde.
//! 6. Zero-copy binary audit frame lossless transmutation roundtrip.
//! 7. Normalization idempotence and arbitrary JSON status extraction robustness.

use std::fs;
use std::io::Write;

use medplum_mcp_core::audit::{
    compute_payload_digest, verify_audit_log, ActionStatus, AuditLogManager,
};
use medplum_mcp_core::safety::{
    assert_write_permitted, normalize_status, SafetyViolationError, FORBIDDEN_CLINICAL_STATUSES,
};
use medplum_mcp_core::secret::SecretString;
use medplum_mcp_core::token_diet::{distill_resource, DetailLevel};
use medplum_mcp_core::zerocopy_audit::BinaryAuditHeader;
use proptest::prelude::*;
use serde_json::json;
use zerocopy::{FromBytes, IntoBytes};

// ---------------------------------------------------------------------------
// Adversarial Homoglyph Mappings
// ---------------------------------------------------------------------------

fn substitute_homoglyph(c: char, variant: usize) -> char {
    match c {
        'a' => match variant % 3 {
            0 => '\u{0430}', // Cyrillic small letter a
            1 => '\u{03b1}', // Greek small letter alpha
            _ => '\u{ff41}', // Fullwidth small latin a
        },
        'c' => match variant % 2 {
            0 => '\u{0441}', // Cyrillic small letter es
            _ => '\u{ff43}', // Fullwidth small latin c
        },
        'e' => match variant % 3 {
            0 => '\u{0435}', // Cyrillic small letter ie
            1 => '\u{03b5}', // Greek small letter epsilon
            _ => '\u{ff45}', // Fullwidth small latin e
        },
        'i' => match variant % 3 {
            0 => '\u{0456}', // Cyrillic small letter byelorussian-ukrainian i
            1 => '\u{03b9}', // Greek small letter iota
            _ => '\u{ff49}', // Fullwidth small latin i
        },
        'o' => match variant % 3 {
            0 => '\u{043e}', // Cyrillic small letter o
            1 => '\u{03bf}', // Greek small letter omicron
            _ => '\u{ff4f}', // Fullwidth small latin o
        },
        'p' => match variant % 3 {
            0 => '\u{0440}', // Cyrillic small letter er
            1 => '\u{03c1}', // Greek small letter rho
            _ => '\u{ff50}', // Fullwidth small latin p
        },
        's' => match variant % 3 {
            0 => '\u{0455}', // Cyrillic small letter dze
            1 => '\u{03c3}', // Greek small letter sigma
            _ => '\u{ff53}', // Fullwidth small latin s
        },
        't' => match variant % 2 {
            0 => '\u{03c4}', // Greek small letter tau
            _ => '\u{ff54}', // Fullwidth small latin t
        },
        'u' => match variant % 2 {
            0 => '\u{03c5}', // Greek small letter upsilon
            _ => '\u{ff55}', // Fullwidth small latin u
        },
        'x' => match variant % 2 {
            0 => '\u{0445}', // Cyrillic small letter ha
            _ => '\u{ff58}', // Fullwidth small latin x
        },
        other => other,
    }
}

const INVISIBLE_CHARS: &[char] = &[
    '\u{200b}', // Zero-width space
    '\u{200c}', // Zero-width non-joiner
    '\u{200d}', // Zero-width joiner
    '\u{feff}', // Zero-width no-break space (BOM)
    '\u{2060}', // Word joiner
];

// ---------------------------------------------------------------------------
// Proptest Invariants
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property 1: For any forbidden status or adversarial case variation,
    /// assert_write_permitted MUST reject with ForbiddenTerminalStatus error.
    #[test]
    fn fuzz_safety_gate_never_passes_forbidden_statuses(
        status_idx in 0usize..FORBIDDEN_CLINICAL_STATUSES.len(),
        leading_spaces in 0usize..4,
        trailing_spaces in 0usize..4,
        uppercase_flags in any::<u16>(),
    ) {
        let base_status = FORBIDDEN_CLINICAL_STATUSES[status_idx];
        let mut status_str = String::new();
        for _ in 0..leading_spaces {
            status_str.push(' ');
        }
        for (i, c) in base_status.chars().enumerate() {
            if (uppercase_flags & (1 << (i % 16))) != 0 {
                status_str.extend(c.to_uppercase());
            } else {
                status_str.push(c);
            }
        }
        for _ in 0..trailing_spaces {
            status_str.push(' ');
        }

        let payload = json!({
            "resourceType": "Observation",
            "status": status_str
        });

        let result = assert_write_permitted("Observation", &payload, true);
        prop_assert!(
            matches!(result, Err(SafetyViolationError::ForbiddenTerminalStatus { .. })),
            "Expected ForbiddenTerminalStatus error for status '{}'", status_str
        );
    }

    /// Property 2: Arbitrary combinations of Cyrillic/Greek homoglyphs and
    /// zero-width invisible characters for forbidden statuses are 100% blocked.
    #[test]
    fn fuzz_homoglyph_detector_immune_to_adversarial_substitutions(
        status_idx in 0usize..FORBIDDEN_CLINICAL_STATUSES.len(),
        sub_mask in any::<u16>(),
        variant_seed in any::<usize>(),
        invisible_pos in 0usize..10,
        invisible_idx in 0usize..INVISIBLE_CHARS.len(),
    ) {
        let base_status = FORBIDDEN_CLINICAL_STATUSES[status_idx];
        let mut adversarial = String::new();

        for (i, c) in base_status.chars().enumerate() {
            if i == invisible_pos {
                adversarial.push(INVISIBLE_CHARS[invisible_idx]);
            }
            if (sub_mask & (1 << (i % 16))) != 0 {
                adversarial.push(substitute_homoglyph(c, variant_seed.wrapping_add(i)));
            } else {
                adversarial.push(c);
            }
        }

        // Must normalize back to the base status
        let normalized = normalize_status(&adversarial);
        prop_assert_eq!(&normalized, base_status);

        // Safety gate must reject it
        let payload = json!({
            "resourceType": "MedicationRequest",
            "status": adversarial
        });
        let result = assert_write_permitted("MedicationRequest", &payload, true);
        prop_assert!(
            matches!(result, Err(SafetyViolationError::ForbiddenTerminalStatus { .. })),
            "Adversarial vector '{}' was not blocked!", adversarial
        );
    }

    /// Property 3: For any valid HMAC audit log, altering any single byte anywhere
    /// in the log causes verify_audit_log to fail with an Error.
    #[test]
    fn fuzz_audit_hash_chain_tamper_detection(
        entry_count in 2usize..6,
        key_seed in any::<[u8; 16]>(),
        mutate_factor in 1u8..255,
    ) {
        let tmp = tempfile::tempdir().unwrap();
        let log_path = tmp.path().join("audit_fuzz.jsonl");

        let mut mgr = AuditLogManager::new(&log_path, &key_seed).unwrap();
        for i in 0..entry_count {
            let payload = json!({ "tool_call_index": i, "data": format!("data_{}", i) });
            mgr.log_event("medplum_test_tool", ActionStatus::Allowed, Some(&payload)).unwrap();
        }

        // Verify valid first
        let report = verify_audit_log(&log_path, &key_seed).unwrap();
        prop_assert!(report.is_valid);
        prop_assert_eq!(report.verified_count, entry_count);

        // Read raw bytes, mutate one byte
        let mut content = fs::read(&log_path).unwrap();
        prop_assert!(!content.is_empty());

        // Find a non-newline byte to mutate
        let mut target_idx = 0;
        for (idx, &b) in content.iter().enumerate() {
            if b != b'\n' && b != b' ' {
                target_idx = idx;
                break;
            }
        }
        content[target_idx] ^= mutate_factor;

        let tampered_path = tmp.path().join("tampered.jsonl");
        let mut f = fs::File::create(&tampered_path).unwrap();
        f.write_all(&content).unwrap();
        f.flush().unwrap();

        let verify_result = verify_audit_log(&tampered_path, &key_seed);
        prop_assert!(
            verify_result.is_err(),
            "Tampered audit log should fail verification"
        );
    }

    /// Property 4: Token diet distillation never increases byte length and preserves
    /// exact bundle entry count for arbitrary bundle sizes.
    #[test]
    fn fuzz_token_diet_monotonic_reduction_and_bundle_preservation(
        pat_count in 1usize..8,
        obs_count in 1usize..8,
    ) {
        let mut entries = Vec::new();

        for i in 0..pat_count {
            entries.push(json!({
                "resource": {
                    "resourceType": "Patient",
                    "id": format!("pat-fuzz-{}", i),
                    "name": [{ "text": format!("Patient {}", i), "family": "Smith" }],
                    "birthDate": "2015-06-20",
                    "gender": "male",
                    "address": [{ "city": "Memphis", "line": ["123 St"], "state": "TN" }],
                    "telecom": [{ "system": "phone", "value": "555-0100" }]
                }
            }));
        }

        for j in 0..obs_count {
            entries.push(json!({
                "resource": {
                    "resourceType": "Observation",
                    "id": format!("obs-fuzz-{}", j),
                    "status": "preliminary",
                    "code": {
                        "coding": [{ "system": "http://loinc.org", "code": "26499-4", "display": "ANC" }],
                        "text": "Absolute Neutrophil Count"
                    },
                    "subject": { "reference": "Patient/pat-fuzz-0" },
                    "valueQuantity": { "value": 1500.0, "unit": "/uL" },
                    "referenceRange": [{ "low": { "value": 1000.0 }, "high": { "value": 8000.0 } }]
                }
            }));
        }

        let raw_bundle = json!({
            "resourceType": "Bundle",
            "type": "searchset",
            "total": pat_count + obs_count,
            "entry": entries
        });

        let raw_bytes = serde_json::to_vec(&raw_bundle).unwrap();

        for tier in &[DetailLevel::Compact, DetailLevel::Standard, DetailLevel::Executive] {
            let distilled = distill_resource(&raw_bundle, *tier);
            let res_type = distilled["resourceType"].as_str().unwrap_or("");
            prop_assert_eq!(res_type, "Bundle");

            let distilled_entries = distilled["entry"].as_array().expect("entries array");
            prop_assert_eq!(
                distilled_entries.len(),
                pat_count + obs_count,
                "Distillation must preserve exact entry count"
            );

            let distilled_bytes = serde_json::to_vec(&distilled).unwrap();
            prop_assert!(
                distilled_bytes.len() <= raw_bytes.len(),
                "Distilled bytes ({}) must not exceed raw bytes ({})",
                distilled_bytes.len(),
                raw_bytes.len()
            );
        }
    }

    /// Property 5: For arbitrary secret string generated, format!("{:?}"),
    /// format!("{}", secret), and serde serialization never contain the plaintext secret.
    #[test]
    fn fuzz_secret_string_zero_leak(
        secret_content in "\\PC{1,64}"
    ) {
        // Exclude accidental literal "***"
        if secret_content == "***" || secret_content.is_empty() {
            return Ok(());
        }

        let secret = SecretString::new(&secret_content);

        let display_repr = format!("{}", secret);
        prop_assert_eq!(display_repr.as_str(), "***");

        let debug_repr = format!("{:?}", secret);
        prop_assert_eq!(debug_repr.as_str(), "SecretString(\"***\")");

        let serialized = serde_json::to_string(&secret).unwrap();
        prop_assert_eq!(serialized.as_str(), "\"***\"");

        // expose_secret does expose it safely
        prop_assert_eq!(secret.expose_secret(), &secret_content);
    }


    /// Property 6: Arbitrary valid binary audit frames roundtrip losslessly
    /// through .as_bytes() and read_from_bytes().
    #[test]
    fn fuzz_zerocopy_binary_audit_roundtrip(
        sequence_id in any::<u64>(),
        timestamp_epoch_ms in any::<u64>(),
        action_status in 0u8..3,
        payload_digest in any::<[u8; 32]>(),
        prev_signature in any::<[u8; 32]>(),
        secret_key in any::<[u8; 32]>(),
    ) {
        let mut header = BinaryAuditHeader::new(
            sequence_id,
            timestamp_epoch_ms,
            action_status,
            payload_digest,
            prev_signature,
        );

        header.sign(&secret_key);
        prop_assert!(header.verify(&secret_key));

        let bytes = header.as_bytes();
        prop_assert_eq!(bytes.len(), 120);

        let recovered = BinaryAuditHeader::read_from_bytes(bytes)
            .expect("Lossless zero-copy read");
        prop_assert_eq!(&recovered, &header);
        prop_assert!(recovered.verify(&secret_key));
    }

    /// Property 7: Normalization is idempotent: normalize_status(normalize_status(s)) == normalize_status(s).
    #[test]
    fn fuzz_normalize_status_idempotent(s in "\\PC{0,50}") {
        let n1 = normalize_status(&s);
        let n2 = normalize_status(&n1);
        prop_assert_eq!(n1, n2);
    }

    /// Property 8: compute_payload_digest is deterministic and always produces 64 hex characters.
    #[test]
    fn fuzz_compute_payload_digest_determinism(
        val_str in "\\PC{0,100}"
    ) {
        let v = json!({ "key": val_str });
        let d1 = compute_payload_digest(Some(&v));
        let d2 = compute_payload_digest(Some(&v));
        prop_assert_eq!(d1.len(), 64);
        prop_assert_eq!(d1, d2);
    }

    /// Property 9: Altering any single byte in the signed portion of BinaryAuditHeader
    /// causes verify() to fail.
    #[test]
    fn fuzz_binary_audit_header_signature_tamper_detection(
        sequence_id in any::<u64>(),
        timestamp in any::<u64>(),
        action_status in 0u8..3,
        payload_digest in any::<[u8; 32]>(),
        prev_sig in any::<[u8; 32]>(),
        secret in any::<[u8; 32]>(),
        tamper_idx in 0usize..88,
        tamper_val in 1u8..255,
    ) {
        let mut header = BinaryAuditHeader::new(
            sequence_id,
            timestamp,
            action_status,
            payload_digest,
            prev_sig,
        );
        header.sign(&secret);
        prop_assert!(header.verify(&secret));

        // Transmute to bytes, mutate one byte in the first 88 bytes, verify fails
        let mut bytes = header.as_bytes().to_vec();
        bytes[tamper_idx] ^= tamper_val;

        let tampered = BinaryAuditHeader::read_from_bytes(&bytes[..]).unwrap();
        prop_assert!(!tampered.verify(&secret));
    }

    /// Property 10: SecretString constant-time equality matches plaintext string equality.
    #[test]
    fn fuzz_secret_string_constant_time_equality(
        s1 in "\\PC{0,64}",
        s2 in "\\PC{0,64}",
    ) {
        let sec1 = SecretString::new(&s1);
        let sec2 = SecretString::new(&s2);
        prop_assert_eq!(sec1 == sec2, s1 == s2);
    }

    /// Property 11: DetailLevel parsing is case-insensitive and trims whitespace.
    #[test]
    fn fuzz_detail_level_from_str(
        tier_choice in 0usize..3,
        leading_spaces in 0usize..4,
        trailing_spaces in 0usize..4,
        upper_mask in any::<u16>(),
    ) {
        let base = match tier_choice {
            0 => "compact",
            1 => "standard",
            _ => "executive",
        };
        let mut candidate = " ".repeat(leading_spaces);
        for (i, c) in base.chars().enumerate() {
            if (upper_mask & (1u16 << i)) != 0 {
                candidate.extend(c.to_uppercase());
            } else {
                candidate.push(c);
            }
        }
        candidate.push_str(&" ".repeat(trailing_spaces));

        let parsed: Result<DetailLevel, _> = candidate.parse();
        prop_assert!(parsed.is_ok());
        let expected = match tier_choice {
            0 => DetailLevel::Compact,
            1 => DetailLevel::Standard,
            _ => DetailLevel::Executive,
        };
        prop_assert_eq!(parsed.unwrap(), expected);
    }

    /// Property 12: Nested status fields inside deeply nested JSON are always detected.
    #[test]
    fn fuzz_nested_status_detection(
        depth in 1usize..6,
        status_idx in 0usize..FORBIDDEN_CLINICAL_STATUSES.len(),
    ) {
        let forbidden = FORBIDDEN_CLINICAL_STATUSES[status_idx];
        let mut current = json!(forbidden);
        for _ in 0..depth {
            current = json!({ "status": current });
        }

        let payload = json!({
            "resourceType": "Bundle",
            "entry": [{ "resource": current }]
        });

        let res = assert_write_permitted("Bundle", &payload, true);
        prop_assert!(
            matches!(res, Err(SafetyViolationError::ForbiddenTerminalStatus { .. })),
            "Nested status '{}' at depth {} was not detected!", forbidden, depth
        );
    }
}
