use proptest::prelude::*;
use tempfile::TempDir;

use dynamic_markdown_dock_lib::crypto::aead::{decrypt_blob, encrypt_blob};
use dynamic_markdown_dock_lib::crypto::keys::{SecretKey32, VaultKeys};
use dynamic_markdown_dock_lib::crypto::keystore::VaultManager;
use dynamic_markdown_dock_lib::crypto::names::derive_file_id;
use dynamic_markdown_dock_lib::crypto::vault_format::{AeadAlg, VaultHeader};
use dynamic_markdown_dock_lib::events::VaultLockState;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    #[test]
    fn blob_round_trip_for_arbitrary_payloads(
        data in proptest::collection::vec(any::<u8>(), 0..65536),
        use_chacha in any::<bool>(),
        pad in any::<bool>(),
    ) {
        let vk = SecretKey32::from_bytes([0x42; 32]);
        let keys = VaultKeys::derive_from_vault_key(vk).unwrap();
        let fid = derive_file_id(&keys.k_name, "root-uuid", "notes/sample.md");
        let aead = if use_chacha {
            AeadAlg::ChaCha20Poly1305
        } else {
            AeadAlg::Aes256Gcm
        };

        let (blob, sha) = encrypt_blob(&keys, 1, aead, &fid, &data, pad).unwrap();
        let decrypted = decrypt_blob(&keys, &fid, &sha, &blob).unwrap();
        prop_assert_eq!(decrypted, data);
    }
}

#[test]
fn tamper_matrix_rejects_every_mutated_or_truncated_byte() {
    let vk = SecretKey32::from_bytes([0x77; 32]);
    let keys = VaultKeys::derive_from_vault_key(vk).unwrap();
    let fid = derive_file_id(&keys.k_name, "root-uuid", "secret.md");
    let plaintext = b"# Secret Note\n\nZero-knowledge encryption test payload.";

    let (blob, sha) = encrypt_blob(&keys, 1, AeadAlg::Aes256Gcm, &fid, plaintext, true).unwrap();

    // Every single byte flip anywhere in header, ciphertext, or auth tag must fail.
    for i in 0..blob.len() {
        let mut mutated = blob.clone();
        mutated[i] ^= 0x01;
        assert!(
            decrypt_blob(&keys, &fid, &sha, &mutated).is_err(),
            "tampered byte at index {i} was unexpectedly accepted"
        );
    }

    // Truncations and trailing bytes must fail.
    for len in [0, 1, 59, 60, blob.len() - 1] {
        assert!(decrypt_blob(&keys, &fid, &sha, &blob[..len]).is_err());
    }
    let mut extended = blob.clone();
    extended.push(0x00);
    assert!(decrypt_blob(&keys, &fid, &sha, &extended).is_err());
}

#[test]
fn rejects_blob_substitution_across_file_ids_or_hashes() {
    let vk = SecretKey32::from_bytes([0x99; 32]);
    let keys = VaultKeys::derive_from_vault_key(vk).unwrap();
    let fid_a = derive_file_id(&keys.k_name, "root-1", "a.md");
    let fid_b = derive_file_id(&keys.k_name, "root-1", "b.md");

    let (blob_a, sha_a) = encrypt_blob(
        &keys,
        1,
        AeadAlg::ChaCha20Poly1305,
        &fid_a,
        b"content A",
        true,
    )
    .unwrap();

    assert!(decrypt_blob(&keys, &fid_b, &sha_a, &blob_a).is_err());

    let mut wrong_sha = sha_a;
    wrong_sha[0] ^= 0xff;
    assert!(decrypt_blob(&keys, &fid_a, &wrong_sha, &blob_a).is_err());
}

#[test]
fn mandatory_recovery_key_blocks_activation_until_verified() {
    let tmp = TempDir::new().unwrap();
    let kf_path = tmp.path().join("keyfile.vault");
    let mgr = VaultManager::init(kf_path.clone());

    assert_eq!(mgr.status().state, VaultLockState::Uninitialized);

    let pending = mgr
        .begin_setup(
            "correct horse battery staple 2026!",
            AeadAlg::Aes256Gcm,
            false,
        )
        .unwrap();
    // Before confirmation, keyfile must NOT exist on disk and state remains Uninitialized.
    assert!(!kf_path.exists());
    assert_eq!(mgr.status().state, VaultLockState::Uninitialized);

    // Wrong suffix must fail and keep state Uninitialized.
    assert!(mgr.confirm_setup(None, "WRONG1").is_err());
    assert!(!kf_path.exists());

    // Correct 6-char suffix writes keyfile and unlocks session.
    let st = mgr.confirm_setup(None, &pending.recovery_suffix).unwrap();
    assert_eq!(st.state, VaultLockState::Unlocked);
    assert!(kf_path.exists());

    // Confirming with the full formatted recovery key (even lowercase) also succeeds cleanly.
    let mgr_full = VaultManager::init(tmp.path().join("keyfile_full.vault"));
    let pending_full = mgr_full
        .begin_setup(
            "correct horse battery staple 2026!",
            AeadAlg::Aes256Gcm,
            false,
        )
        .unwrap();
    let st_full = mgr_full
        .confirm_setup(None, &pending_full.recovery_key.to_ascii_lowercase())
        .unwrap();
    assert_eq!(st_full.state, VaultLockState::Unlocked);

    // Locking clears keys; unlocking via password or recovery key works.
    let locked = mgr.lock(None, false);
    assert_eq!(locked.state, VaultLockState::Locked);
    assert!(mgr.with_unlocked(|_, _, _, _| Ok(())).is_err());

    let unlocked = mgr
        .unlock_with_recovery(None, &pending.recovery_key, None, false)
        .unwrap();
    assert_eq!(unlocked.state, VaultLockState::Unlocked);

    // Verify parsed header on disk matches v1 keyfile format.
    let raw = std::fs::read(&kf_path).unwrap();
    let (hdr, _, _) = VaultHeader::parse(&raw).unwrap();
    assert_eq!(hdr.header_len, 72);
}
