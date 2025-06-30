use crate::{BlockBytes, BlockWords};
use core::mem;

pub(crate) const MAX_SIMD_DEGREE: usize = 1;

// There are some places where we want a static size that's equal to the
// MAX_SIMD_DEGREE, but also at least 2. Constant contexts aren't currently
// allowed to use cmp::max, so we have to hardcode this additional constant
// value. Get rid of this once cmp::max is a const fn.
pub(crate) const MAX_SIMD_DEGREE_OR_2: usize = 2;

macro_rules! extract_u32_from_byte_chunks {
    ($src:ident, $chunk_index:literal) => {
        u32::from_le_bytes([
            $src[$chunk_index * 4 + 0],
            $src[$chunk_index * 4 + 1],
            $src[$chunk_index * 4 + 2],
            $src[$chunk_index * 4 + 3],
        ])
    };
}

/// Converts bytes into `u32` words, the size matches BLAKE3 hash
#[inline(always)]
pub const fn words_from_le_bytes_32(bytes: &[u8; 32]) -> [u32; 8] {
    let mut out = [0; 8];
    out[0] = extract_u32_from_byte_chunks!(bytes, 0);
    out[1] = extract_u32_from_byte_chunks!(bytes, 1);
    out[2] = extract_u32_from_byte_chunks!(bytes, 2);
    out[3] = extract_u32_from_byte_chunks!(bytes, 3);
    out[4] = extract_u32_from_byte_chunks!(bytes, 4);
    out[5] = extract_u32_from_byte_chunks!(bytes, 5);
    out[6] = extract_u32_from_byte_chunks!(bytes, 6);
    out[7] = extract_u32_from_byte_chunks!(bytes, 7);
    out
}

/// Converts bytes into `u32` words, the size matches BLAKE3 block
#[inline(always)]
pub const fn words_from_le_bytes_64(bytes: &BlockBytes) -> BlockWords {
    let mut out = [0; 16];
    out[0] = extract_u32_from_byte_chunks!(bytes, 0);
    out[1] = extract_u32_from_byte_chunks!(bytes, 1);
    out[2] = extract_u32_from_byte_chunks!(bytes, 2);
    out[3] = extract_u32_from_byte_chunks!(bytes, 3);
    out[4] = extract_u32_from_byte_chunks!(bytes, 4);
    out[5] = extract_u32_from_byte_chunks!(bytes, 5);
    out[6] = extract_u32_from_byte_chunks!(bytes, 6);
    out[7] = extract_u32_from_byte_chunks!(bytes, 7);
    out[8] = extract_u32_from_byte_chunks!(bytes, 8);
    out[9] = extract_u32_from_byte_chunks!(bytes, 9);
    out[10] = extract_u32_from_byte_chunks!(bytes, 10);
    out[11] = extract_u32_from_byte_chunks!(bytes, 11);
    out[12] = extract_u32_from_byte_chunks!(bytes, 12);
    out[13] = extract_u32_from_byte_chunks!(bytes, 13);
    out[14] = extract_u32_from_byte_chunks!(bytes, 14);
    out[15] = extract_u32_from_byte_chunks!(bytes, 15);
    out
}

/// Converts `u32` words into bytes, the size matches BLAKE3 hash
#[inline(always)]
pub const fn le_bytes_from_words_32(words: &[u32; 8]) -> &[u8; 32] {
    // SAFETY: All bit patterns are valid, output alignment is smaller (1 byte) than input
    unsafe { mem::transmute::<&[u32; 8], &[u8; 32]>(words) }
}
