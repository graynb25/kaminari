// Extension storage at rest. Values are protected with Windows DPAPI in the
// current-user scope before they reach SQLite and stored as
// `dpapi:v1:<base64>`. The extension host still reads and writes plaintext;
// this module is only the on-disk representation.
//
// A value that starts with `dpapi:` but does not parse as v1 is treated as
// unreadable, never as legacy plaintext, so a corrupt row cannot be silently
// double-encrypted or returned as ciphertext.

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;

use crate::error::{AppError, AppResult};

pub const PREFIX: &str = "dpapi:v1:";
const FAMILY_PREFIX: &str = "dpapi:";

pub fn is_encrypted(value: &str) -> bool {
    value.starts_with(FAMILY_PREFIX)
}

pub fn encrypt(value: &str) -> AppResult<String> {
    if value.is_empty() {
        // DPAPI rejects an empty input blob. An empty payload after the
        // version prefix is the canonical empty value and never plaintext.
        return Ok(PREFIX.to_string());
    }
    let protected = platform::protect(value.as_bytes()).map_err(|err| {
        AppError::Crypto(format!("extension storage could not be encrypted: {err}"))
    })?;
    Ok(format!("{PREFIX}{}", BASE64.encode(protected)))
}

pub fn decrypt(stored: &str) -> AppResult<String> {
    let encoded = stored.strip_prefix(PREFIX).ok_or_else(|| {
        if is_encrypted(stored) {
            AppError::Crypto("extension storage uses an unsupported encryption version".to_string())
        } else {
            AppError::Crypto("extension storage value is not encrypted".to_string())
        }
    })?;
    if encoded.is_empty() {
        return Ok(String::new());
    }
    let protected = BASE64.decode(encoded).map_err(|err| {
        AppError::Crypto(format!(
            "extension storage ciphertext is not valid base64: {err}"
        ))
    })?;
    if protected.is_empty() {
        return Err(AppError::Crypto(
            "extension storage ciphertext is empty".to_string(),
        ));
    }
    let plain = platform::unprotect(&protected).map_err(|err| {
        AppError::Crypto(format!(
            "extension storage ciphertext could not be decrypted: {err}"
        ))
    })?;
    String::from_utf8(plain)
        .map_err(|_| AppError::Crypto("extension storage plaintext is not valid UTF-8".to_string()))
}

#[cfg(windows)]
mod platform {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    fn input_blob(data: &[u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr().cast_mut(),
        }
    }

    fn copy_and_free(blob: CRYPT_INTEGER_BLOB) -> Vec<u8> {
        if blob.pbData.is_null() {
            return Vec::new();
        }
        // The copy is made before LocalFree. Both pointers and the length come
        // from the OS API, not from the stored string.
        let bytes = if blob.cbData == 0 {
            Vec::new()
        } else {
            unsafe { std::slice::from_raw_parts(blob.pbData, blob.cbData as usize) }.to_vec()
        };
        unsafe {
            let _ = LocalFree(Some(HLOCAL(blob.pbData.cast())));
        }
        bytes
    }

    pub fn protect(data: &[u8]) -> Result<Vec<u8>, String> {
        if data.is_empty() {
            return Err("cannot protect an empty value".to_string());
        }
        let input = input_blob(data);
        let mut output = CRYPT_INTEGER_BLOB::default();
        unsafe {
            CryptProtectData(
                &input,
                PCWSTR::null(),
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
        .map_err(|err| format!("CryptProtectData failed: {err}"))?;
        Ok(copy_and_free(output))
    }

    pub fn unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
        if data.is_empty() {
            return Err("cannot unprotect an empty value".to_string());
        }
        let input = input_blob(data);
        let mut output = CRYPT_INTEGER_BLOB::default();
        unsafe {
            CryptUnprotectData(
                &input,
                None,
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
        .map_err(|err| format!("CryptUnprotectData failed: {err}"))?;
        Ok(copy_and_free(output))
    }
}

#[cfg(not(windows))]
mod platform {
    pub fn protect(_data: &[u8]) -> Result<Vec<u8>, String> {
        Err("Windows DPAPI is only available on Windows".to_string())
    }

    pub fn unprotect(_data: &[u8]) -> Result<Vec<u8>, String> {
        Err("Windows DPAPI is only available on Windows".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_plaintext_and_unicode() {
        for value in [
            "token-123",
            "日本語のテキスト",
            "line\nwith\ttabs and nul\0",
        ] {
            let encrypted = encrypt(value).unwrap();
            assert!(encrypted.starts_with(PREFIX));
            assert!(!encrypted.contains(value));
            assert_eq!(decrypt(&encrypted).unwrap(), value);
        }
    }

    #[test]
    fn empty_values_round_trip_without_plaintext() {
        let encrypted = encrypt("").unwrap();
        assert_eq!(encrypted, PREFIX);
        assert_eq!(decrypt(&encrypted).unwrap(), "");
    }

    #[test]
    fn malformed_ciphertext_returns_an_error() {
        for stored in [
            "dpapi:v1:not base64",
            "dpapi:v1:AAAA",
            "dpapi:v2:AAAA",
            "dpapi:",
        ] {
            assert!(decrypt(stored).is_err(), "{stored} should not decrypt");
        }
    }

    #[test]
    fn plaintext_is_not_encrypted_material() {
        assert!(!is_encrypted("plain token"));
        assert!(decrypt("plain token").is_err());
        assert!(is_encrypted(&encrypt("plain token").unwrap()));
    }
}
