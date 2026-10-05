//! Clinical Safety Invariants, Dynamic Safety Gates, and Homoglyph-Resistant Normalization.

use unicode_normalization::UnicodeNormalization;

/// FHIR clinical resource statuses that represent binding, irrevocable, or clinical terminal states.
/// Mutations attempting to set these statuses are forbidden to autonomous agents.
pub const FORBIDDEN_CLINICAL_STATUSES: &[&str] = &[
    "active",
    "completed",
    "final",
    "amended",
    "corrected",
    "resolved",
    "refuted",
    "entered-in-error",
    "cancelled",
];

/// Errors returned by the runtime clinical safety gate.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum SafetyViolationError {
    #[error("Write operations are disabled for '{resource_type}'. Explicit permission required.")]
    PermissionDenied { resource_type: String },

    #[error(
        "Safety invariant violation on '{resource_type}': Status '{raw_status}' \
         (normalized: '{normalized_status}') is forbidden. Mutations are strictly restricted \
         to draft states requiring human-in-the-loop sign-off."
    )]
    ForbiddenTerminalStatus {
        resource_type: String,
        raw_status: String,
        normalized_status: String,
    },
}

/// Translate confusable Cyrillic, Greek, and other homoglyphs to Latin ASCII equivalents.
fn translate_homoglyph(c: char) -> char {
    match c {
        // Cyrillic lowercase
        '\u{0430}' => 'a',
        '\u{0435}' => 'e',
        '\u{043e}' => 'o',
        '\u{0440}' => 'p',
        '\u{0441}' => 'c',
        '\u{0443}' => 'y',
        '\u{0445}' => 'x',
        '\u{0456}' => 'i',
        '\u{0458}' => 'j',
        '\u{0455}' => 's',
        '\u{043c}' => 'm',
        '\u{0501}' => 'd',
        '\u{051b}' => 'q',
        '\u{051d}' => 'w',
        // Cyrillic uppercase
        '\u{0410}' => 'A',
        '\u{0412}' => 'B',
        '\u{0421}' => 'C',
        '\u{0415}' => 'E',
        '\u{041d}' => 'H',
        '\u{041e}' => 'O',
        '\u{0420}' => 'P',
        '\u{0422}' => 'T',
        '\u{0425}' => 'X',
        '\u{0406}' => 'I',
        '\u{041c}' => 'M',
        // Greek lowercase & uppercase
        '\u{03b1}' => 'a',
        '\u{03b5}' => 'e',
        '\u{03b9}' => 'i',
        '\u{03bf}' => 'o',
        '\u{03c1}' => 'p',
        '\u{03c2}' => 's',
        '\u{03c3}' => 's',
        '\u{03c4}' => 't',
        '\u{03c5}' => 'u',
        '\u{03bd}' => 'v',
        other => other,
    }
}

/// Returns true if character is an invisible, formatting, or zero-width code point.
fn is_invisible_char(c: char) -> bool {
    matches!(
        c,
        '\u{200b}'..='\u{200f}'
        | '\u{202a}'..='\u{202e}'
        | '\u{2060}'..='\u{206f}'
        | '\u{feff}'
    )
}

/// Normalize status string using Unicode NFKC normalization, homoglyph translation,
/// and invisible character removal.
pub fn normalize_status(status: &str) -> String {
    // 1. Unicode NFKC normalization (handles full-width, compatibility characters)
    let nfkc_str: String = status.nfkc().collect();

    // 2. Homoglyph translation & 3. Strip invisible / zero-width characters
    let cleaned: String = nfkc_str
        .chars()
        .filter(|&c| !is_invisible_char(c))
        .map(translate_homoglyph)
        .collect();

    // 4. Trim whitespace and lowercase
    cleaned.trim().to_ascii_lowercase()
}

/// Recursively extract all status fields from a FHIR JSON resource payload.
pub fn extract_statuses(payload: &serde_json::Value, parent_key: &str) -> Vec<String> {
    let mut statuses = Vec::new();

    match payload {
        serde_json::Value::String(s) => {
            let pk_lower = parent_key.to_ascii_lowercase();
            if pk_lower == "status" || pk_lower.ends_with("status") || pk_lower == "code" {
                statuses.push(s.clone());
            }
        }
        serde_json::Value::Object(map) => {
            for (key, val) in map {
                let k_lower = key.to_ascii_lowercase();
                if k_lower == "status" || k_lower.ends_with("status") {
                    match val {
                        serde_json::Value::String(s) => statuses.push(s.clone()),
                        serde_json::Value::Object(_) => {
                            statuses.extend(extract_statuses(val, &k_lower));
                        }
                        serde_json::Value::Array(arr) => {
                            for item in arr {
                                if let serde_json::Value::String(s) = item {
                                    statuses.push(s.clone());
                                } else {
                                    statuses.extend(extract_statuses(item, &k_lower));
                                }
                            }
                        }
                        _ => {}
                    }
                } else if parent_key.to_ascii_lowercase().ends_with("status")
                    && k_lower == "code"
                    && val.is_string()
                {
                    if let serde_json::Value::String(s) = val {
                        statuses.push(s.clone());
                    }
                } else {
                    statuses.extend(extract_statuses(val, &k_lower));
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                statuses.extend(extract_statuses(item, parent_key));
            }
        }
        _ => {}
    }

    statuses
}

/// Non-bypassable runtime safety gate enforcing write permissions and draft-only invariants.
///
/// Ensures AI agents cannot transition external clinical records into binding states.
pub fn assert_write_permitted(
    resource_type: &str,
    payload: &serde_json::Value,
    allow_writes: bool,
) -> Result<(), SafetyViolationError> {
    if !allow_writes {
        return Err(SafetyViolationError::PermissionDenied {
            resource_type: resource_type.to_string(),
        });
    }

    let mut statuses = extract_statuses(payload, "");
    if statuses.is_empty() {
        if let serde_json::Value::String(s) = payload {
            statuses.push(s.clone());
        }
    }

    for raw_status in statuses {
        let clean_status = normalize_status(&raw_status);
        if FORBIDDEN_CLINICAL_STATUSES.contains(&clean_status.as_str()) {
            return Err(SafetyViolationError::ForbiddenTerminalStatus {
                resource_type: resource_type.to_string(),
                raw_status,
                normalized_status: clean_status,
            });
        }
    }

    Ok(())
}
