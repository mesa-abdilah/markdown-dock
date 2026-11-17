use data_encoding::BASE32_NOPAD;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use unicode_normalization::UnicodeNormalization;

use crate::crypto::keys::SecretKey32;

type HmacSha256 = Hmac<Sha256>;

pub fn hmac_sha256(key: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut mac =
        <HmacSha256 as KeyInit>::new_from_slice(key).expect("HMAC accepts any key length");
    for part in parts {
        mac.update(part);
    }
    let result = mac.finalize().into_bytes();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    out
}

/// Deterministic file identifier across devices: HMAC-SHA256(K_name, root_uuid || "/" || rel_path_nfc).
pub fn derive_file_id(k_name: &SecretKey32, root_uuid: &str, rel_path: &str) -> [u8; 32] {
    let rel_nfc: String = rel_path.replace('\\', "/").nfc().collect();
    hmac_sha256(
        k_name.expose(),
        &[root_uuid.as_bytes(), b"/", rel_nfc.as_bytes()],
    )
}

/// Opaque Drive blob filename: `<base32(HMAC(K_name, "blob" || file_id || plaintext_sha256))[:32]>.vault`.
pub fn blob_filename(
    k_name: &SecretKey32,
    file_id: &[u8; 32],
    plaintext_sha256: &[u8; 32],
) -> String {
    let tag = hmac_sha256(k_name.expose(), &[b"blob", file_id, plaintext_sha256]);
    let b32 = BASE32_NOPAD.encode(&tag).to_ascii_lowercase();
    format!("{}.vault", &b32[..32])
}

/// Opaque per-device manifest filename: `m-<base32(HMAC(K_name, "manifest" || device_id))[:26]>.vault`.
pub fn manifest_filename(k_name: &SecretKey32, device_id: &str) -> String {
    let tag = hmac_sha256(k_name.expose(), &[b"manifest", device_id.as_bytes()]);
    let b32 = BASE32_NOPAD.encode(&tag).to_ascii_lowercase();
    format!("m-{}.vault", &b32[..26])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_sha256_rfc4231_test_case_1() {
        let key = [0x0bu8; 20];
        let data = b"Hi There";
        let mac = hmac_sha256(&key, &[data]);
        assert_eq!(
            hex::encode(mac),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }

    #[test]
    fn blob_and_manifest_names_are_opaque_and_deterministic() {
        let kn = SecretKey32::from_bytes([0x5A; 32]);
        let fid1 = derive_file_id(&kn, "root-1", "notes/hello.md");
        let fid2 = derive_file_id(&kn, "root-1", "notes/hello.md");
        assert_eq!(fid1, fid2);

        let name = blob_filename(&kn, &fid1, &[0x11; 32]);
        assert!(name.ends_with(".vault"));
        assert!(!name.contains("hello"));
        assert!(!name.contains("notes"));
        assert_eq!(name.len(), 38); // 32 + ".vault"

        let mname = manifest_filename(&kn, "dev-abc");
        assert!(mname.starts_with("m-"));
        assert!(mname.ends_with(".vault"));
        assert_eq!(mname.len(), 34); // "m-" + 26 + ".vault"
    }
}
