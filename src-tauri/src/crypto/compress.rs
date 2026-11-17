use std::io::Read;

use crate::error::AppError;

pub const ZSTD_LEVEL: i32 = 9;
pub const MAX_PLAINTEXT_BYTES: usize = 64 * 1024 * 1024; // 64 MiB

/// Computes the Padmé padded length for `len` bytes (bounds size leakage to O(log log N) bits).
pub fn padme_padded_len(len: usize) -> usize {
    if len <= 64 {
        return 64;
    }
    let e = (usize::BITS - 1 - len.leading_zeros()) as usize;
    let s = (usize::BITS - 1 - e.leading_zeros()) as usize + 1;
    if e <= s {
        return len;
    }
    let bit_mask = (1usize << (e - s)) - 1;
    (len + bit_mask) & !bit_mask
}

pub fn compress_zstd(plaintext: &[u8]) -> Result<Vec<u8>, AppError> {
    if plaintext.len() > MAX_PLAINTEXT_BYTES {
        return Err(AppError::Crypto("Plaintext exceeds 64 MiB v1 limit".into()));
    }
    zstd::bulk::compress(plaintext, ZSTD_LEVEL)
        .map_err(|e| AppError::Crypto(format!("zstd compression failed: {e}")))
}

pub fn decompress_zstd(compressed: &[u8]) -> Result<Vec<u8>, AppError> {
    let mut decoder = zstd::stream::read::Decoder::new(compressed)
        .map_err(|e| AppError::Crypto(format!("zstd decoder init failed: {e}")))?;
    let mut out = Vec::new();
    let mut limited = (&mut decoder).take((MAX_PLAINTEXT_BYTES + 1) as u64);
    limited
        .read_to_end(&mut out)
        .map_err(|e| AppError::Crypto(format!("zstd decompression failed: {e}")))?;
    if out.len() > MAX_PLAINTEXT_BYTES {
        return Err(AppError::Crypto(
            "Decompressed payload exceeds 64 MiB safety limit".into(),
        ));
    }
    Ok(out)
}

pub fn frame_padded_inner(payload: &[u8]) -> Result<Vec<u8>, AppError> {
    let len_u32 = u32::try_from(payload.len())
        .map_err(|_| AppError::Crypto("Payload too large to frame".into()))?;
    let total = padme_padded_len(4 + payload.len());
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&len_u32.to_le_bytes());
    out.extend_from_slice(payload);
    out.resize(total, 0u8);
    Ok(out)
}

pub fn unframe_padded_inner(inner: &[u8]) -> Result<&[u8], AppError> {
    if inner.len() < 4 {
        return Err(AppError::Crypto("Padded frame too short".into()));
    }
    let len = u32::from_le_bytes([inner[0], inner[1], inner[2], inner[3]]) as usize;
    if 4 + len > inner.len() {
        return Err(AppError::Crypto(
            "Invalid framed length in padded payload".into(),
        ));
    }
    Ok(&inner[4..4 + len])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zstd_and_padme_round_trip() {
        let data = b"# Markdown Note\n\nRepeated content Repeated content Repeated content\n";
        let compressed = compress_zstd(data).unwrap();
        let framed = frame_padded_inner(&compressed).unwrap();
        assert!(framed.len() >= 4 + compressed.len());
        let unframed = unframe_padded_inner(&framed).unwrap();
        let restored = decompress_zstd(unframed).unwrap();
        assert_eq!(restored, data);
    }

    #[test]
    fn padme_bounds_are_monotonic_and_ge_input() {
        let mut prev = 0;
        for sz in [0, 1, 63, 64, 65, 255, 1024, 4096, 65535, 1_000_000] {
            let padded = padme_padded_len(sz);
            assert!(padded >= sz);
            assert!(padded >= prev);
            prev = padded;
        }
    }
}
