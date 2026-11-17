use aes_gcm::Aes256Gcm;
use aes_gcm::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::ChaCha20Poly1305;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

use crate::crypto::compress::{
    compress_zstd, decompress_zstd, frame_padded_inner, unframe_padded_inner,
};
use crate::crypto::kdf::{default_kdf_params, derive_kek};
use crate::crypto::keys::{
    SecretKey32, VaultKeys, derive_recovery_kek, derive_recovery_kek_from_key,
};
use crate::crypto::vault_format::{
    AeadAlg, BASE_HEADER_LEN, CompressionAlg, FLAG_PADDED, KEYFILE_HEADER_LEN, KdfHeaderParams,
    VAULT_VERSION_1, VaultHeader, VaultKind,
};
use crate::error::AppError;

#[derive(Debug, Serialize, Deserialize)]
struct KeyfileBody {
    vault_id: [u8; 16],
    recovery_nonce: [u8; 12],
    pw_wrapped_vk: Vec<u8>,
    recovery_wrapped_vk: Vec<u8>,
}

pub fn read_keyfile_vault_id(keyfile_bytes: &[u8]) -> Result<([u8; 16], VaultHeader), AppError> {
    let (header, _, body_bytes) = VaultHeader::parse(keyfile_bytes)?;
    if header.kind != VaultKind::Keyfile {
        return Err(AppError::Crypto("Expected keyfile vault container".into()));
    }
    let body: KeyfileBody = ciborium::de::from_reader(body_bytes)
        .map_err(|e| AppError::Crypto(format!("Invalid keyfile body: {e}")))?;
    Ok((body.vault_id, header))
}

pub struct KeyfileInfo {
    pub vault_id: [u8; 16],
    pub key_epoch: u32,
    pub aead: AeadAlg,
    pub kdf: KdfHeaderParams,
}

fn random_bytes<const N: usize>() -> Result<[u8; N], AppError> {
    let mut buf = [0u8; N];
    getrandom::fill(&mut buf).map_err(|e| AppError::Crypto(format!("CSPRNG failed: {e}")))?;
    Ok(buf)
}

pub fn aead_encrypt(
    aead: AeadAlg,
    key: &[u8; 32],
    nonce: &[u8; 12],
    plaintext: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, AppError> {
    let payload = Payload {
        msg: plaintext,
        aad,
    };
    match aead {
        AeadAlg::Aes256Gcm => {
            let cipher = Aes256Gcm::new_from_slice(key)
                .map_err(|_| AppError::Crypto("Invalid AES-256 key".into()))?;
            cipher
                .encrypt(&(*nonce).into(), payload)
                .map_err(|_| AppError::Crypto("AES-256-GCM encryption failed".into()))
        }
        AeadAlg::ChaCha20Poly1305 => {
            let cipher = ChaCha20Poly1305::new_from_slice(key)
                .map_err(|_| AppError::Crypto("Invalid ChaCha20 key".into()))?;
            cipher
                .encrypt(&(*nonce).into(), payload)
                .map_err(|_| AppError::Crypto("ChaCha20-Poly1305 encryption failed".into()))
        }
    }
}

pub fn aead_decrypt(
    aead: AeadAlg,
    key: &[u8; 32],
    nonce: &[u8; 12],
    ciphertext: &[u8],
    aad: &[u8],
) -> Result<Zeroizing<Vec<u8>>, AppError> {
    let payload = Payload {
        msg: ciphertext,
        aad,
    };
    let pt = match aead {
        AeadAlg::Aes256Gcm => {
            let cipher = Aes256Gcm::new_from_slice(key)
                .map_err(|_| AppError::Crypto("Invalid AES-256 key".into()))?;
            cipher
                .decrypt(&(*nonce).into(), payload)
                .map_err(|_| AppError::Crypto("Authentication failed".into()))?
        }
        AeadAlg::ChaCha20Poly1305 => {
            let cipher = ChaCha20Poly1305::new_from_slice(key)
                .map_err(|_| AppError::Crypto("Invalid ChaCha20 key".into()))?;
            cipher
                .decrypt(&(*nonce).into(), payload)
                .map_err(|_| AppError::Crypto("Authentication failed".into()))?
        }
    };
    Ok(Zeroizing::new(pt))
}

fn build_blob_aad(header_bytes: &[u8], file_id: &[u8; 32], plaintext_sha256: &[u8; 32]) -> Vec<u8> {
    let mut aad = Vec::with_capacity(header_bytes.len() + 64);
    aad.extend_from_slice(header_bytes);
    aad.extend_from_slice(file_id);
    aad.extend_from_slice(plaintext_sha256);
    aad
}

fn build_pw_slot_aad(header_bytes: &[u8], vault_id: &[u8; 16]) -> Vec<u8> {
    let mut aad = Vec::with_capacity(header_bytes.len() + 16 + 7);
    aad.extend_from_slice(header_bytes);
    aad.extend_from_slice(vault_id);
    aad.extend_from_slice(b"pw-slot");
    aad
}

fn build_recovery_slot_aad(
    header: &VaultHeader,
    vault_id: &[u8; 16],
    recovery_nonce: &[u8; 12],
) -> Vec<u8> {
    // Bound to invariant vault identity + algorithm + epoch + recovery_nonce so
    // rotating Master Password does not invalidate the Recovery Key slot.
    let mut aad = Vec::with_capacity(64);
    aad.extend_from_slice(b"DMDV");
    aad.push(header.version);
    aad.push(header.kind as u8);
    aad.push(header.aead as u8);
    aad.extend_from_slice(&header.key_epoch.to_le_bytes());
    aad.extend_from_slice(vault_id);
    aad.extend_from_slice(recovery_nonce);
    aad.extend_from_slice(b"recovery-slot");
    aad
}

pub fn sha256_bytes(data: &[u8]) -> [u8; 32] {
    let digest = Sha256::digest(data);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

/// Compresses (zstd) and encrypts a Markdown file into a `.vault` binary blob bound to `file_id` and `plaintext_sha256`.
pub fn encrypt_blob(
    keys: &VaultKeys,
    key_epoch: u32,
    aead: AeadAlg,
    file_id: &[u8; 32],
    plaintext: &[u8],
    pad: bool,
) -> Result<(Vec<u8>, [u8; 32]), AppError> {
    let pt_sha = sha256_bytes(plaintext);
    let compressed = compress_zstd(plaintext)?;
    let inner = if pad {
        frame_padded_inner(&compressed)?
    } else {
        compressed
    };

    let salt: [u8; 32] = random_bytes()?;
    let nonce: [u8; 12] = random_bytes()?;
    let header = VaultHeader {
        version: VAULT_VERSION_1,
        kind: VaultKind::Blob,
        aead,
        compression: CompressionAlg::Zstd,
        flags: if pad { FLAG_PADDED } else { 0 },
        header_len: BASE_HEADER_LEN as u16,
        key_epoch,
        salt,
        nonce,
        kdf: None,
    };
    let header_bytes = header.encode()?;
    let file_key = keys.derive_file_key(&salt)?;
    let aad = build_blob_aad(&header_bytes, file_id, &pt_sha);
    let ct = aead_encrypt(aead, file_key.expose(), &nonce, &inner, &aad)?;

    let mut out = Vec::with_capacity(header_bytes.len() + ct.len());
    out.extend_from_slice(&header_bytes);
    out.extend_from_slice(&ct);
    Ok((out, pt_sha))
}

/// Decrypts and decompresses a `.vault` blob, verifying header AAD, `file_id`, and `plaintext_sha256`.
pub fn decrypt_blob(
    keys: &VaultKeys,
    file_id: &[u8; 32],
    expected_sha256: &[u8; 32],
    vault_bytes: &[u8],
) -> Result<Vec<u8>, AppError> {
    let (header, header_bytes, body) = VaultHeader::parse(vault_bytes)?;
    if header.kind != VaultKind::Blob {
        return Err(AppError::Crypto("Expected blob vault container".into()));
    }
    let file_key = keys.derive_file_key(&header.salt)?;
    let aad = build_blob_aad(header_bytes, file_id, expected_sha256);
    let inner = aead_decrypt(header.aead, file_key.expose(), &header.nonce, body, &aad)?;

    let compressed = if header.flags & FLAG_PADDED != 0 {
        unframe_padded_inner(&inner)?
    } else {
        inner.as_slice()
    };

    let plaintext = match header.compression {
        CompressionAlg::Zstd => decompress_zstd(compressed)?,
        CompressionAlg::None => compressed.to_vec(),
    };

    let actual_sha = sha256_bytes(&plaintext);
    if !bool::from(actual_sha.ct_eq(expected_sha256)) {
        return Err(AppError::Crypto(
            "Decrypted plaintext SHA-256 mismatch".into(),
        ));
    }
    Ok(plaintext)
}

pub fn encrypt_manifest(
    keys: &VaultKeys,
    key_epoch: u32,
    aead: AeadAlg,
    vault_id: &[u8; 16],
    cbor_bytes: &[u8],
) -> Result<Vec<u8>, AppError> {
    let compressed = compress_zstd(cbor_bytes)?;
    let inner = frame_padded_inner(&compressed)?;
    let salt: [u8; 32] = random_bytes()?;
    let nonce: [u8; 12] = random_bytes()?;
    let header = VaultHeader {
        version: VAULT_VERSION_1,
        kind: VaultKind::Manifest,
        aead,
        compression: CompressionAlg::Zstd,
        flags: FLAG_PADDED,
        header_len: BASE_HEADER_LEN as u16,
        key_epoch,
        salt,
        nonce,
        kdf: None,
    };
    let header_bytes = header.encode()?;
    let mkey = keys.derive_manifest_key(&salt)?;
    let mut aad = Vec::with_capacity(header_bytes.len() + 16 + 11);
    aad.extend_from_slice(&header_bytes);
    aad.extend_from_slice(vault_id);
    aad.extend_from_slice(b"manifest-v1");
    let ct = aead_encrypt(aead, mkey.expose(), &nonce, &inner, &aad)?;

    let mut out = Vec::with_capacity(header_bytes.len() + ct.len());
    out.extend_from_slice(&header_bytes);
    out.extend_from_slice(&ct);
    Ok(out)
}

pub fn decrypt_manifest(
    keys: &VaultKeys,
    vault_id: &[u8; 16],
    vault_bytes: &[u8],
) -> Result<Vec<u8>, AppError> {
    let (header, header_bytes, body) = VaultHeader::parse(vault_bytes)?;
    if header.kind != VaultKind::Manifest {
        return Err(AppError::Crypto("Expected manifest vault container".into()));
    }
    let mkey = keys.derive_manifest_key(&header.salt)?;
    let mut aad = Vec::with_capacity(header_bytes.len() + 16 + 11);
    aad.extend_from_slice(header_bytes);
    aad.extend_from_slice(vault_id);
    aad.extend_from_slice(b"manifest-v1");
    let inner = aead_decrypt(header.aead, mkey.expose(), &header.nonce, body, &aad)?;

    let compressed = if header.flags & FLAG_PADDED != 0 {
        unframe_padded_inner(&inner)?
    } else {
        inner.as_slice()
    };
    match header.compression {
        CompressionAlg::Zstd => decompress_zstd(compressed),
        CompressionAlg::None => Ok(compressed.to_vec()),
    }
}

/// Creates a brand-new `keyfile.vault` wrapping a fresh random `VaultKey` under both
/// Master Password (Argon2id KEK) and a mandatory 256-bit Recovery Key.
pub fn create_keyfile(
    password: &str,
    aead: AeadAlg,
) -> Result<(Vec<u8>, VaultKeys, SecretKey32, KeyfileInfo), AppError> {
    let vault_id: [u8; 16] = random_bytes()?;
    let vault_key = SecretKey32::random()?;
    let recovery_key = SecretKey32::random()?;
    let kdf = default_kdf_params();
    let salt: [u8; 32] = random_bytes()?;
    let pw_nonce: [u8; 12] = random_bytes()?;
    let recovery_nonce: [u8; 12] = random_bytes()?;

    let header = VaultHeader {
        version: VAULT_VERSION_1,
        kind: VaultKind::Keyfile,
        aead,
        compression: CompressionAlg::None,
        flags: 0,
        header_len: KEYFILE_HEADER_LEN as u16,
        key_epoch: 1,
        salt,
        nonce: pw_nonce,
        kdf: Some(kdf),
    };
    let header_bytes = header.encode()?;

    let kek_pw = derive_kek(password, &salt, &kdf)?;
    let pw_aad = build_pw_slot_aad(&header_bytes, &vault_id);
    let pw_wrapped_vk = aead_encrypt(
        aead,
        kek_pw.expose(),
        &pw_nonce,
        vault_key.expose(),
        &pw_aad,
    )?;

    let kek_rec = derive_recovery_kek_from_key(&recovery_key)?;
    let rec_aad = build_recovery_slot_aad(&header, &vault_id, &recovery_nonce);
    let recovery_wrapped_vk = aead_encrypt(
        aead,
        kek_rec.expose(),
        &recovery_nonce,
        vault_key.expose(),
        &rec_aad,
    )?;

    let body = KeyfileBody {
        vault_id,
        recovery_nonce,
        pw_wrapped_vk,
        recovery_wrapped_vk,
    };
    let mut cbor = Vec::new();
    ciborium::ser::into_writer(&body, &mut cbor)
        .map_err(|e| AppError::Crypto(format!("Keyfile CBOR encode failed: {e}")))?;

    let mut out = Vec::with_capacity(header_bytes.len() + cbor.len());
    out.extend_from_slice(&header_bytes);
    out.extend_from_slice(&cbor);

    let keys = VaultKeys::derive_from_vault_key(vault_key)?;
    let info = KeyfileInfo {
        vault_id,
        key_epoch: 1,
        aead,
        kdf,
    };
    Ok((out, keys, recovery_key, info))
}

pub fn unwrap_keyfile_with_password(
    keyfile_bytes: &[u8],
    password: &str,
) -> Result<(VaultKeys, KeyfileInfo), AppError> {
    let (header, header_bytes, body_bytes) = VaultHeader::parse(keyfile_bytes)?;
    if header.kind != VaultKind::Keyfile {
        return Err(AppError::Crypto("Expected keyfile vault container".into()));
    }
    let kdf = header
        .kdf
        .ok_or_else(|| AppError::Crypto("Keyfile missing KDF header block".into()))?;
    let body: KeyfileBody = ciborium::de::from_reader(body_bytes)
        .map_err(|e| AppError::Crypto(format!("Invalid keyfile body: {e}")))?;

    let kek = derive_kek(password, &header.salt, &kdf)?;
    let aad = build_pw_slot_aad(header_bytes, &body.vault_id);
    let vk_bytes = aead_decrypt(
        header.aead,
        kek.expose(),
        &header.nonce,
        &body.pw_wrapped_vk,
        &aad,
    )?;
    if vk_bytes.len() != 32 {
        return Err(AppError::Crypto(
            "Invalid unwrapped Vault Key length".into(),
        ));
    }
    let mut raw = [0u8; 32];
    raw.copy_from_slice(&vk_bytes);
    let keys = VaultKeys::derive_from_vault_key(SecretKey32::from_bytes(raw))?;
    Ok((
        keys,
        KeyfileInfo {
            vault_id: body.vault_id,
            key_epoch: header.key_epoch,
            aead: header.aead,
            kdf,
        },
    ))
}

pub fn unwrap_keyfile_with_recovery(
    keyfile_bytes: &[u8],
    recovery_key_input: &str,
) -> Result<(VaultKeys, KeyfileInfo), AppError> {
    let (header, _header_bytes, body_bytes) = VaultHeader::parse(keyfile_bytes)?;
    if header.kind != VaultKind::Keyfile {
        return Err(AppError::Crypto("Expected keyfile vault container".into()));
    }
    let kdf = header
        .kdf
        .ok_or_else(|| AppError::Crypto("Keyfile missing KDF header block".into()))?;
    let body: KeyfileBody = ciborium::de::from_reader(body_bytes)
        .map_err(|e| AppError::Crypto(format!("Invalid keyfile body: {e}")))?;

    let kek_rec = derive_recovery_kek(recovery_key_input)?;
    let aad = build_recovery_slot_aad(&header, &body.vault_id, &body.recovery_nonce);
    let vk_bytes = aead_decrypt(
        header.aead,
        kek_rec.expose(),
        &body.recovery_nonce,
        &body.recovery_wrapped_vk,
        &aad,
    )?;
    if vk_bytes.len() != 32 {
        return Err(AppError::Crypto(
            "Invalid unwrapped Vault Key length".into(),
        ));
    }
    let mut raw = [0u8; 32];
    raw.copy_from_slice(&vk_bytes);
    let keys = VaultKeys::derive_from_vault_key(SecretKey32::from_bytes(raw))?;
    Ok((
        keys,
        KeyfileInfo {
            vault_id: body.vault_id,
            key_epoch: header.key_epoch,
            aead: header.aead,
            kdf,
        },
    ))
}

/// Re-wraps the existing `VaultKey` under a new Master Password in O(1) without re-encrypting blobs.
pub fn rewrap_keyfile_password(
    existing_keyfile: &[u8],
    keys: &VaultKeys,
    new_password: &str,
) -> Result<Vec<u8>, AppError> {
    let (old_header, _, body_bytes) = VaultHeader::parse(existing_keyfile)?;
    if old_header.kind != VaultKind::Keyfile {
        return Err(AppError::Crypto("Expected keyfile vault container".into()));
    }
    let mut body: KeyfileBody = ciborium::de::from_reader(body_bytes)
        .map_err(|e| AppError::Crypto(format!("Invalid keyfile body: {e}")))?;

    let new_salt: [u8; 32] = random_bytes()?;
    let new_nonce: [u8; 12] = random_bytes()?;
    let kdf = default_kdf_params();
    let new_header = VaultHeader {
        version: old_header.version,
        kind: VaultKind::Keyfile,
        aead: old_header.aead,
        compression: CompressionAlg::None,
        flags: 0,
        header_len: KEYFILE_HEADER_LEN as u16,
        key_epoch: old_header.key_epoch,
        salt: new_salt,
        nonce: new_nonce,
        kdf: Some(kdf),
    };
    let header_bytes = new_header.encode()?;
    let kek = derive_kek(new_password, &new_salt, &kdf)?;
    let aad = build_pw_slot_aad(&header_bytes, &body.vault_id);
    body.pw_wrapped_vk = aead_encrypt(
        old_header.aead,
        kek.expose(),
        &new_nonce,
        keys.vault_key.expose(),
        &aad,
    )?;

    let mut cbor = Vec::new();
    ciborium::ser::into_writer(&body, &mut cbor)
        .map_err(|e| AppError::Crypto(format!("Keyfile CBOR encode failed: {e}")))?;
    let mut out = Vec::with_capacity(header_bytes.len() + cbor.len());
    out.extend_from_slice(&header_bytes);
    out.extend_from_slice(&cbor);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nist_aes_256_gcm_known_answer_vector() {
        // NIST SP 800-38D Test Case 16 (256-bit key, 96-bit IV, plaintext + AAD)
        let key: [u8; 32] =
            hex::decode("feffe9928665731c6d6a8f9467308308feffe9928665731c6d6a8f9467308308")
                .unwrap()
                .try_into()
                .unwrap();
        let iv: [u8; 12] = hex::decode("cafebabefacedbaddecaf888")
            .unwrap()
            .try_into()
            .unwrap();
        let pt = hex::decode(
            "d9313225f88406e5a55909c5aff5269a86a7a9531534f7da2e4c303d8a318a72\
             1c3c0c95956809532fcf0e2449a6b525b16aedf5aa0de657ba637b39",
        )
        .unwrap();
        let aad = hex::decode("feedfacedeadbeeffeedfacedeadbeefabaddad2").unwrap();

        let ct_and_tag = aead_encrypt(AeadAlg::Aes256Gcm, &key, &iv, &pt, &aad).unwrap();
        assert_eq!(
            hex::encode(&ct_and_tag[..pt.len()]),
            "522dc1f099567d07f47f37a32a84427d643a8cdcbfe5c0c97598a2bd2555d1aa\
             8cb08e48590dbb3da7b08b1056828838c5f61e6393ba7a0abcc9f662"
        );
        assert_eq!(
            hex::encode(&ct_and_tag[pt.len()..]),
            "76fc6ece0f4e1768cddf8853bb2d551b"
        );
        let dec = aead_decrypt(AeadAlg::Aes256Gcm, &key, &iv, &ct_and_tag, &aad).unwrap();
        assert_eq!(dec.as_slice(), pt.as_slice());
    }

    #[test]
    fn keyfile_setup_unlock_rewrap_and_recovery_work() {
        let (kf, keys, rec, _info) =
            create_keyfile("correct-horse-battery-staple!", AeadAlg::Aes256Gcm).unwrap();
        let rec_str = crate::crypto::keys::format_recovery_key(&rec);

        let (keys_pw, _) =
            unwrap_keyfile_with_password(&kf, "correct-horse-battery-staple!").unwrap();
        assert_eq!(keys_pw.vault_key.expose(), keys.vault_key.expose());

        assert!(unwrap_keyfile_with_password(&kf, "wrong-password").is_err());

        let (keys_rec, _) = unwrap_keyfile_with_recovery(&kf, &rec_str).unwrap();
        assert_eq!(keys_rec.vault_key.expose(), keys.vault_key.expose());

        // Rewrap password in O(1), verify old password rejected, new password + recovery key still work
        let kf2 = rewrap_keyfile_password(&kf, &keys, "brand-new-passphrase-99").unwrap();
        assert!(unwrap_keyfile_with_password(&kf2, "correct-horse-battery-staple!").is_err());
        let (keys_pw2, _) = unwrap_keyfile_with_password(&kf2, "brand-new-passphrase-99").unwrap();
        assert_eq!(keys_pw2.vault_key.expose(), keys.vault_key.expose());
        let (keys_rec2, _) = unwrap_keyfile_with_recovery(&kf2, &rec_str).unwrap();
        assert_eq!(keys_rec2.vault_key.expose(), keys.vault_key.expose());
    }
}
