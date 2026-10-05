//! Zero-leak enclave holding sensitive tokens, keys, and credentials.
//!
//! Masked in Display, Debug, and serialization formats to prevent credential
//! leakage into LLM context windows, logs, or error traces.

use std::fmt;

/// An enclave wrapper around a sensitive string.
///
/// Prevents accidental logging, formatting, or leak of secrets.
#[derive(Clone, Default)]
pub struct SecretString {
    inner: String,
}

impl SecretString {
    /// Create a new `SecretString` enclave.
    pub fn new(secret: impl Into<String>) -> Self {
        Self {
            inner: secret.into(),
        }
    }

    /// Expose the underlying plaintext secret string.
    pub fn expose_secret(&self) -> &str {
        &self.inner
    }

    /// Expose the secret as a string slice (alias).
    pub fn as_str(&self) -> &str {
        &self.inner
    }

    /// Returns the length in bytes of the underlying secret.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns true if the underlying secret is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl fmt::Display for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "***")
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SecretString").field(&"***").finish()
    }
}

impl PartialEq for SecretString {
    fn eq(&self, other: &Self) -> bool {
        if self.inner.len() != other.inner.len() {
            return false;
        }
        // Constant-time byte equality comparison
        let a = self.inner.as_bytes();
        let b = other.inner.as_bytes();
        let mut diff = 0u8;
        for (x, y) in a.iter().zip(b.iter()) {
            diff |= x ^ y;
        }
        diff == 0
    }
}

impl Eq for SecretString {}

impl serde::Serialize for SecretString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("***")
    }
}

impl<'de> serde::Deserialize<'de> for SecretString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(SecretString::new(s))
    }
}
