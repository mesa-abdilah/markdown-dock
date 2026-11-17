# Dynamic Markdown Dock `.vault` Binary Format (v1)

All multi-byte integers are **little-endian**.

## 1. Container Header

The entire header (`bytes[0..header_len]`) is included in the AEAD Associated Authenticated Data (AAD), so any modification to header fields fails authentication.

| Offset | Size | Field | Description |
|---:|---:|---|---|
| 0 | 4 | `magic` | ASCII `"DMDV"` (`0x44 0x4D 0x44 0x56`) |
| 4 | 1 | `version` | `0x01` |
| 5 | 1 | `kind` | `0x01` Blob · `0x02` Manifest · `0x03` Keyfile |
| 6 | 1 | `aead_id` | `0x01` AES-256-GCM · `0x02` ChaCha20-Poly1305 |
| 7 | 1 | `compression` | `0x00` None · `0x01` Zstd (level 9) |
| 8 | 2 | `flags` | Bit `0`: `PADDED` (Padmé framing) · Bit `1`: `CHUNKED` (reserved) · Bit `2`: `ZSTD_DICT` |
| 10 | 2 | `header_len` | `60` for Blob/Manifest, `72` for Keyfile |
| 12 | 4 | `key_epoch` | Vault Key generation counter |
| 16 | 32 | `salt` | Blob/Manifest: HKDF per-file salt · Keyfile: Argon2id salt |
| 48 | 12 | `nonce` | 96-bit CSPRNG nonce |
| 60 | 12 | *(Keyfile only)* `kdf_block` | `kdf_id` (`0x01` Argon2id), `m_cost_kib` (`u32`), `t_cost` (`u32`), `p_cost` (`u8`), `reserved` (`u16`) |
| `header_len` | N | `payload` | AEAD ciphertext+tag (for Blob/Manifest), or CBOR `KeyfileBody` (for Keyfile) |

## 2. Key Hierarchy

1. **Master Password (NFKC)** → `Argon2id(salt, m >= 64 MiB, t >= 3, p >= 1)` → **KEK (256-bit)**.
2. **Recovery Key (256-bit random, 52 base32 chars)** → `HKDF-SHA256(info = "dmd/v1/recovery")` → **KEK_recovery**.
3. **Vault Key (256-bit CSPRNG)** wrapped inside `keyfile.vault` under both `KEK` and `KEK_recovery`.
4. Subkeys derived via `HKDF-SHA256(VaultKey)`:
   - `K_blob` (`info = "dmd/v1/blob"`) → per-blob key `HKDF(K_blob, salt = header.salt, info = "dmd/v1/file-key")`
   - `K_manifest` (`info = "dmd/v1/manifest"`) → per-manifest key `HKDF(K_manifest, salt = header.salt, info = "dmd/v1/manifest-key")`
   - `K_name` (`info = "dmd/v1/name"`) → HMAC-SHA256 for `file_id`, blob filenames, and manifest filenames.

## 3. Blob Framing & AAD

- Inner plaintext before AEAD:
  - When `FLAG_PADDED` is set: `u32_le(len(compressed)) || compressed || zero_padding` up to `padme_padded_len(4 + len(compressed))`.
  - When `FLAG_PADDED` is clear: `compressed`.
- `AAD = header_bytes || file_id[32] || plaintext_sha256[32]`.
