// Extension host. One Runtime per extension, fresh Context per call.
// Timeout 5 seconds, 10 for resolve. Memory cap 64MB. Bridge only.

pub struct ExtHost {
    pub timeout_secs: u64,
    pub memory_bytes: usize,
}

impl Default for ExtHost {
    fn default() -> Self {
        Self { timeout_secs: 5, memory_bytes: 64 * 1024 * 1024 }
    }
}

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
