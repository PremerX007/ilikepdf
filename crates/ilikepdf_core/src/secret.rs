use std::fmt::{self, Debug, Formatter};

/// An owned application secret whose formatted representation is always redacted.
///
/// This reduces accidental disclosure in diagnostics. It does not claim to erase
/// copies that may have existed before this value crossed the application boundary.
pub struct SecretString(Vec<u8>);

impl SecretString {
    pub fn new(value: String) -> Self {
        Self(value.into_bytes())
    }

    pub fn expose_secret(&self) -> &str {
        std::str::from_utf8(&self.0).expect("SecretString always contains UTF-8")
    }
}

impl Debug for SecretString {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretString([REDACTED])")
    }
}

impl Drop for SecretString {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_output_is_redacted() {
        let secret = SecretString::new("not-for-diagnostics".to_owned());

        let formatted = format!("{secret:?}");

        assert_eq!(formatted, "SecretString([REDACTED])");
        assert!(!formatted.contains(secret.expose_secret()));
    }
}
