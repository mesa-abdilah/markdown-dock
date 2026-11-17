use data_encoding::BASE32_NOPAD;
use hkdf::Hkdf;
use secrecy::{ExposeSecret, SecretBox};
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::error::AppError;

const INFO_BLOB: &[u8] = b"dmd/v1/blob";
const INFO_MANIFEST: &[u8] = b"dmd/v1/manifest";
const INFO_NAME: &[u8] = b"dmd/v1/name";
const INFO_RECOVERY: &[u8] = b"dmd/v1/recovery";
const INFO_FILE_KEY: &[u8] = b"dmd/v1/file-key";
const INFO_MANIFEST_KEY: &[u8] = b"dmd/v1/manifest-key";

/// 256-bit secret key wrapped in `SecretBox` (zeroized on drop, redacted in Debug).
pub struct SecretKey32(SecretBox<[u8; 32]>);

impl SecretKey32 {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(SecretBox::new(Box::new(bytes)))
    }

    pub fn random() -> Result<Self, AppError> {
        let mut buf = Zeroizing::new([0u8; 32]);
        getrandom::fill(buf.as_mut())
            .map_err(|e| AppError::Crypto(format!("CSPRNG failed: {e}")))?;
        Ok(Self::from_bytes(*buf))
    }

    pub fn expose(&self) -> &[u8; 32] {
        self.0.expose_secret()
    }
}

impl Clone for SecretKey32 {
    fn clone(&self) -> Self {
        Self::from_bytes(*self.expose())
    }
}

/// Complete unlocked key hierarchy held in memory while the Vault is unlocked.
/// Intentionally omits `Debug` and `Display` so keys can never appear in logs.
#[derive(Clone)]
pub struct VaultKeys {
    pub vault_key: SecretKey32,
    pub k_blob: SecretKey32,
    pub k_manifest: SecretKey32,
    pub k_name: SecretKey32,
}

impl VaultKeys {
    pub fn derive_from_vault_key(vault_key: SecretKey32) -> Result<Self, AppError> {
        let k_blob = hkdf_expand(vault_key.expose(), None, INFO_BLOB)?;
        let k_manifest = hkdf_expand(vault_key.expose(), None, INFO_MANIFEST)?;
        let k_name = hkdf_expand(vault_key.expose(), None, INFO_NAME)?;
        Ok(Self {
            vault_key,
            k_blob,
            k_manifest,
            k_name,
        })
    }

    pub fn derive_file_key(&self, header_salt: &[u8; 32]) -> Result<SecretKey32, AppError> {
        hkdf_expand(self.k_blob.expose(), Some(header_salt), INFO_FILE_KEY)
    }

    pub fn derive_manifest_key(&self, header_salt: &[u8; 32]) -> Result<SecretKey32, AppError> {
        hkdf_expand(
            self.k_manifest.expose(),
            Some(header_salt),
            INFO_MANIFEST_KEY,
        )
    }
}

pub fn hkdf_expand(ikm: &[u8], salt: Option<&[u8]>, info: &[u8]) -> Result<SecretKey32, AppError> {
    let hk = Hkdf::<Sha256>::new(salt, ikm);
    let mut okm = Zeroizing::new([0u8; 32]);
    hk.expand(info, okm.as_mut())
        .map_err(|e| AppError::Crypto(format!("HKDF expand failed: {e}")))?;
    Ok(SecretKey32::from_bytes(*okm))
}

/// Formats a 256-bit Recovery Key as 52 base32 characters grouped by 4 (`XXXX-XXXX-...`).
pub fn format_recovery_key(key: &SecretKey32) -> Zeroizing<String> {
    let raw = BASE32_NOPAD.encode(key.expose());
    let mut grouped = String::with_capacity(raw.len() + 13);
    for (i, ch) in raw.chars().enumerate() {
        if i > 0 && i % 4 == 0 {
            grouped.push('-');
        }
        grouped.push(ch);
    }
    Zeroizing::new(grouped)
}

/// Parses a base32 Recovery Key (with or without hyphens/spaces) and derives `KEK_recovery`.
pub fn derive_recovery_kek(recovery_input: &str) -> Result<SecretKey32, AppError> {
    let cleaned: String = recovery_input
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .map(|c| c.to_ascii_uppercase())
        .collect();
    let decoded = BASE32_NOPAD
        .decode(cleaned.as_bytes())
        .map_err(|_| AppError::Crypto("Invalid Recovery Key format".into()))?;
    if decoded.len() != 32 {
        return Err(AppError::Crypto(
            "Recovery Key must decode to 256 bits".into(),
        ));
    }
    hkdf_expand(&decoded, None, INFO_RECOVERY)
}

pub fn derive_recovery_kek_from_key(recovery_key: &SecretKey32) -> Result<SecretKey32, AppError> {
    hkdf_expand(recovery_key.expose(), None, INFO_RECOVERY)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hkdf_sha256_rfc5869_test_case_1() {
        // RFC 5869 A.1 (first 32 bytes of 42-byte OKM)
        let ikm = hex::decode("0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b").unwrap();
        let salt = hex::decode("000102030405060708090a0b0c").unwrap();
        let info = hex::decode("f0f1f2f3f4f5f6f7f8f9").unwrap();
        let okm = hkdf_expand(&ikm, Some(&salt), &info).unwrap();
        assert_eq!(
            hex::encode(okm.expose()),
            "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf"
        );
    }

    #[test]
    fn recovery_key_round_trips_formatting() {
        let rk = SecretKey32::random().unwrap();
        let formatted = format_recovery_key(&rk);
        // 32 bytes -> 52 base32 chars + 12 hyphens = 64 chars
        assert_eq!(formatted.len(), 64);
        let kek1 = derive_recovery_kek_from_key(&rk).unwrap();
        let kek2 = derive_recovery_kek(&formatted.to_lowercase()).unwrap();
        assert_eq!(kek1.expose(), kek2.expose());
    }
}
