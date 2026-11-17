# Threat Model — Dynamic Markdown Dock

## 1. Security Goals
1. **Zero-Knowledge Cloud Backup:** Google Drive stores only compressed, authenticated-encrypted blobs (`.vault`) and an encrypted manifest (`manifest.vault`). Neither Google nor any party with read/write access to the user's Google Drive can view plaintext Markdown notes, directory hierarchies, tags, or original file names.
2. **Anti-Preview Enforcement:** Uploaded files use opaque HMAC-SHA256 hex names (`<64-hex>.vault`) with `application/octet-stream` MIME type and binary `DMDV` container headers, preventing Google Drive previewers or search indexers from parsing contents.
3. **Tamper & Rollback Detection:** Authenticated encryption (`AES-256-GCM` or `ChaCha20-Poly1305`) binds the entire header (version, kind, algorithm, flags, key epoch, salt, nonce) as Additional Authenticated Data (`AAD`) alongside an AEAD-encrypted manifest tracking epochs and generation sequence numbers.

## 2. Cryptographic Architecture
- **KDF:** `Argon2id` (`m = 65536 KiB`, `t = 3`, `p = 4`) derives the 256-bit `MasterKey` from the user's passphrase (enforced minimum `zxcvbn` score `>= 3`).
- **Key Hierarchy (HKDF-SHA256):**
  - `k_blob`: blob encryption subkey (used with unique per-revision random 96-bit nonces)
  - `k_manifest`: manifest encryption subkey
  - `k_name`: HMAC-SHA256 subkey for deterministic opaque remote names (`<hex>.vault`)
- **Compression & Traffic-Analysis Mitigation:** Plaintext notes are compressed with `zstd` (level 9) and padded to Padmé bucket boundaries (`FLAG_PADDED`) before encryption to obscure exact file sizes.
- **Recovery Key:** Setup mandates generating and verifying a 52-character Base32 Recovery Key capable of re-wrapping the Master Key inside `keyfile.vault` if the user forgets their passphrase.
- **Memory Hygiene:** Sensitive keys implement `Zeroize` / `ZeroizeOnDrop` (`SecretKey32`). Auto-lock and explicit lock zeroize in-memory keys and optionally purge the OS Keyring entry.

## 3. Trust Boundaries & Assumptions
| Boundary | Trusted? | Notes |
| --- | --- | --- |
| Local OS user session & memory | Trusted while unlocked | Vault must be explicitly locked or Forget Keyring triggered before sharing an unlocked OS session. |
| OS Credential Store (`keyring`) | Optional opt-in | Stores encoded key material when "Remember in OS Keyring" is chosen at unlock/setup. |
| Google Drive (`drive.file` scope) | Untrusted for confidentiality & integrity | Treated as untrusted remote blob storage; any modified header, truncated ciphertext, swapped AAD, or unknown key epoch fails AEAD tag verification (`vault_tamper.rs`). |
| Local Markdown files & editor | User-controlled | Markdown rendering uses `comrak` + `ammonia` HTML sanitization (`CSP: default-src 'self'`). |

## 4. Non-Goals / Out-of-Scope Threats
- Compromised local OS kernel, keyloggers, or malicious privileged processes reading unlocked RAM.
- File-system metadata leakage on the user's own local disk (local `.md` notes remain plaintext on disk for external editors).
- Traffic-correlation timing against Google Drive API endpoints while actively syncing.

## 5. Verification Coverage
- **Tamper & Crypto Tests (`src-tauri/tests/vault_tamper.rs`):** Bit-flip ciphertext, modified header AAD, swapped blob substitution, key epoch mismatch, and Padmé padding bounds.
- **Fuzzing (`src-tauri/fuzz/fuzz_targets/parse_vault_header.rs`):** Untrusted binary container parsing invariants.
- **Offline Recovery CLI (`tools/dmd-vault`):** Inspect and decrypt `.vault` containers independently of the desktop GUI.
