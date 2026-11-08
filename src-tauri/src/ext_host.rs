// Extension host limits. One Runtime per extension, fresh Context per call.
// Timeout 5 seconds, 10 for resolve. Memory cap 64MB. Bridge only.

pub const TIMEOUT_SECS: u64 = 5;
pub const RESOLVE_TIMEOUT_SECS: u64 = 10;
pub const MEMORY_BYTES: usize = 64 * 1024 * 1024;

pub fn validate_source_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_must_be_digits_only() {
        assert!(validate_source_id("8542735178285060053"));
        assert!(!validate_source_id("abc"));
        assert!(!validate_source_id(""));
    }
}
