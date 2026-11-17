use serde::{Deserialize, Serialize};
use specta::Type;

use crate::error::AppError;

pub const VAULT_MAGIC: [u8; 4] = *b"DMDV";
pub const VAULT_VERSION_1: u8 = 0x01;
pub const BASE_HEADER_LEN: usize = 60;
pub const KEYFILE_HEADER_LEN: usize = 72;
pub const AEAD_TAG_LEN: usize = 16;
pub const MAX_VAULT_PAYLOAD_BYTES: usize = 64 * 1024 * 1024 + 4096;

pub const FLAG_PADDED: u16 = 1 << 0;
pub const FLAG_CHUNKED: u16 = 1 << 1;
pub const FLAG_ZSTD_DICT: u16 = 1 << 2;
pub const KNOWN_FLAGS_MASK: u16 = FLAG_PADDED | FLAG_CHUNKED | FLAG_ZSTD_DICT;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[repr(u8)]
pub enum VaultKind {
    Blob = 0x01,
    Manifest = 0x02,
    Keyfile = 0x03,
}

impl VaultKind {
    pub fn from_u8(v: u8) -> Result<Self, AppError> {
        match v {
            0x01 => Ok(Self::Blob),
            0x02 => Ok(Self::Manifest),
            0x03 => Ok(Self::Keyfile),
            _ => Err(AppError::Crypto(format!(
                "Unsupported vault kind: {v:#04x}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[repr(u8)]
pub enum AeadAlg {
    Aes256Gcm = 0x01,
    ChaCha20Poly1305 = 0x02,
}

impl AeadAlg {
    pub fn from_u8(v: u8) -> Result<Self, AppError> {
        match v {
            0x01 => Ok(Self::Aes256Gcm),
            0x02 => Ok(Self::ChaCha20Poly1305),
            _ => Err(AppError::Crypto(format!(
                "Unsupported AEAD algorithm: {v:#04x}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[repr(u8)]
pub enum CompressionAlg {
    None = 0x00,
    Zstd = 0x01,
}

impl CompressionAlg {
    pub fn from_u8(v: u8) -> Result<Self, AppError> {
        match v {
            0x00 => Ok(Self::None),
            0x01 => Ok(Self::Zstd),
            _ => Err(AppError::Crypto(format!(
                "Unsupported compression: {v:#04x}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct KdfHeaderParams {
    pub kdf_id: u8,
    pub m_cost_kib: u32,
    pub t_cost: u32,
    pub p_cost: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultHeader {
    pub version: u8,
    pub kind: VaultKind,
    pub aead: AeadAlg,
    pub compression: CompressionAlg,
    pub flags: u16,
    pub header_len: u16,
    pub key_epoch: u32,
    pub salt: [u8; 32],
    pub nonce: [u8; 12],
    pub kdf: Option<KdfHeaderParams>,
}

impl VaultHeader {
    pub fn encode(&self) -> Result<Vec<u8>, AppError> {
        let expected_len = if self.kind == VaultKind::Keyfile {
            KEYFILE_HEADER_LEN as u16
        } else {
            BASE_HEADER_LEN as u16
        };
        if self.header_len < expected_len {
            return Err(AppError::Crypto("Invalid header_len for vault kind".into()));
        }

        let mut buf = Vec::with_capacity(usize::from(self.header_len));
        buf.extend_from_slice(&VAULT_MAGIC);
        buf.push(self.version);
        buf.push(self.kind as u8);
        buf.push(self.aead as u8);
        buf.push(self.compression as u8);
        buf.extend_from_slice(&self.flags.to_le_bytes());
        buf.extend_from_slice(&self.header_len.to_le_bytes());
        buf.extend_from_slice(&self.key_epoch.to_le_bytes());
        buf.extend_from_slice(&self.salt);
        buf.extend_from_slice(&self.nonce);

        if let Some(kdf) = self.kdf {
            buf.push(kdf.kdf_id);
            buf.extend_from_slice(&kdf.m_cost_kib.to_le_bytes());
            buf.extend_from_slice(&kdf.t_cost.to_le_bytes());
            buf.push(kdf.p_cost);
            buf.extend_from_slice(&0u16.to_le_bytes()); // reserved
        } else if self.kind == VaultKind::Keyfile {
            return Err(AppError::Crypto(
                "Keyfile header requires KDF params".into(),
            ));
        }

        while buf.len() < usize::from(self.header_len) {
            buf.push(0);
        }
        Ok(buf)
    }

    pub fn parse(bytes: &[u8]) -> Result<(Self, &[u8], &[u8]), AppError> {
        if bytes.len() < BASE_HEADER_LEN {
            return Err(AppError::Crypto(
                "Vault container too short for header".into(),
            ));
        }
        if bytes.len() > MAX_VAULT_PAYLOAD_BYTES + KEYFILE_HEADER_LEN + AEAD_TAG_LEN {
            return Err(AppError::Crypto(
                "Vault container exceeds maximum size".into(),
            ));
        }
        if bytes[0..4] != VAULT_MAGIC {
            return Err(AppError::Crypto("Invalid vault magic bytes".into()));
        }

        let version = bytes[4];
        if version != VAULT_VERSION_1 {
            return Err(AppError::Crypto(format!(
                "Unsupported vault version: {version}"
            )));
        }

        let kind = VaultKind::from_u8(bytes[5])?;
        let aead = AeadAlg::from_u8(bytes[6])?;
        let compression = CompressionAlg::from_u8(bytes[7])?;
        let flags = u16::from_le_bytes([bytes[8], bytes[9]]);
        if flags & !KNOWN_FLAGS_MASK != 0 {
            return Err(AppError::Crypto("Unsupported vault flags".into()));
        }
        if flags & FLAG_CHUNKED != 0 {
            return Err(AppError::Crypto(
                "Chunked vaults are not supported in v1".into(),
            ));
        }

        let header_len = u16::from_le_bytes([bytes[10], bytes[11]]);
        let hlen = usize::from(header_len);
        let min_len = if kind == VaultKind::Keyfile {
            KEYFILE_HEADER_LEN
        } else {
            BASE_HEADER_LEN
        };
        if hlen < min_len || hlen > 4096 || bytes.len() < hlen {
            return Err(AppError::Crypto(
                "Invalid or truncated vault header_len".into(),
            ));
        }

        let key_epoch = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
        let mut salt = [0u8; 32];
        salt.copy_from_slice(&bytes[16..48]);
        let mut nonce = [0u8; 12];
        nonce.copy_from_slice(&bytes[48..60]);

        let kdf = if kind == VaultKind::Keyfile {
            let kdf_id = bytes[60];
            if kdf_id != 0x01 {
                return Err(AppError::Crypto(format!(
                    "Unsupported KDF id: {kdf_id:#04x}"
                )));
            }
            let m_cost_kib = u32::from_le_bytes([bytes[61], bytes[62], bytes[63], bytes[64]]);
            let t_cost = u32::from_le_bytes([bytes[65], bytes[66], bytes[67], bytes[68]]);
            let p_cost = bytes[69];
            Some(KdfHeaderParams {
                kdf_id,
                m_cost_kib,
                t_cost,
                p_cost,
            })
        } else {
            None
        };

        let header_bytes = &bytes[..hlen];
        let body = &bytes[hlen..];
        if kind != VaultKind::Keyfile && body.len() < AEAD_TAG_LEN {
            return Err(AppError::Crypto(
                "Vault body truncated before AEAD tag".into(),
            ));
        }

        Ok((
            Self {
                version,
                kind,
                aead,
                compression,
                flags,
                header_len,
                key_epoch,
                salt,
                nonce,
                kdf,
            },
            header_bytes,
            body,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_blob_and_keyfile_headers() {
        let h = VaultHeader {
            version: VAULT_VERSION_1,
            kind: VaultKind::Blob,
            aead: AeadAlg::Aes256Gcm,
            compression: CompressionAlg::Zstd,
            flags: FLAG_PADDED,
            header_len: BASE_HEADER_LEN as u16,
            key_epoch: 1,
            salt: [0x11; 32],
            nonce: [0x22; 12],
            kdf: None,
        };
        let mut raw = h.encode().unwrap();
        raw.extend_from_slice(&[0u8; 32]);
        let (parsed, hbytes, body) = VaultHeader::parse(&raw).unwrap();
        assert_eq!(parsed, h);
        assert_eq!(hbytes.len(), BASE_HEADER_LEN);
        assert_eq!(body.len(), 32);
    }

    #[test]
    fn rejects_invalid_magic_and_unknown_flags() {
        let mut raw = vec![0u8; 80];
        raw[0..4].copy_from_slice(b"BAD!");
        assert!(VaultHeader::parse(&raw).is_err());

        let mut valid = VaultHeader {
            version: VAULT_VERSION_1,
            kind: VaultKind::Blob,
            aead: AeadAlg::Aes256Gcm,
            compression: CompressionAlg::Zstd,
            flags: 0,
            header_len: BASE_HEADER_LEN as u16,
            key_epoch: 1,
            salt: [1; 32],
            nonce: [2; 12],
            kdf: None,
        }
        .encode()
        .unwrap();
        valid.extend_from_slice(&[0u8; 16]);
        valid[8] = 0x80; // unknown flag
        assert!(VaultHeader::parse(&valid).is_err());
    }
}
