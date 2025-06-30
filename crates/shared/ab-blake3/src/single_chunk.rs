//! BLAKE3 functions that process at most a single chunk.
//!
//! This module and submodules are copied with modifications from the official [`blake3`] crate, but
//! is unlikely to be upstreamed.
//!
//! [`blake3`]: https://github.com/BLAKE3-team/BLAKE3

#[cfg(test)]
mod tests;

use crate::platform::{le_bytes_from_words_32, words_from_le_bytes_32};
use crate::{
    CVWords, BLOCK_LEN, CHUNK_END, CHUNK_LEN, CHUNK_START, DERIVE_KEY_CONTEXT, DERIVE_KEY_MATERIAL,
    IV, KEYED_HASH, KEY_LEN, OUT_LEN, ROOT,
};
use blake3::platform::Platform;

// Hash a single chunk worth of values
#[inline(always)]
fn hash_chunk(input: &[u8], key: CVWords, flags: u8) -> Option<[u8; OUT_LEN]> {
    // If the whole subtree is one chunk, hash it directly with a ChunkState.
    if input.len() > CHUNK_LEN {
        return None;
    }

    let mut cv = key;
    let platform = Platform::detect();
    let blocks = input.array_chunks();
    let remainder = blocks.remainder();
    let num_blocks = blocks.len() + (!remainder.is_empty()) as usize;

    for (block_index, block) in blocks.enumerate() {
        let mut block_flags = flags;
        if block_index == 0 {
            block_flags |= CHUNK_START;
        }
        if block_index + 1 == num_blocks {
            block_flags |= CHUNK_END | ROOT;
        }

        platform.compress_in_place(&mut cv, block, BLOCK_LEN as u8, 0, block_flags);
    }

    // `num_blocks == 0` means zero bytes input length
    if !remainder.is_empty() || num_blocks == 0 {
        let mut block_flags = flags | CHUNK_END | ROOT;
        if num_blocks <= 1 {
            block_flags |= CHUNK_START;
        }

        let mut buf = [0; BLOCK_LEN];
        buf[..remainder.len()].copy_from_slice(remainder);
        platform.compress_in_place(&mut cv, &buf, remainder.len() as u8, 0, block_flags);
    }

    Some(le_bytes_from_words_32(&cv))
}

/// Hashing function for at most a single chunk worth of bytes.
///
/// Returns `None` if input length exceeds one chunk.
#[inline]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub fn single_chunk_hash(input: &[u8]) -> Option<[u8; OUT_LEN]> {
    hash_chunk(input, *IV, 0)
}

/// The keyed hash function for at most a single chunk worth of bytes.
///
/// Returns `None` if input length exceeds one chunk.
#[inline]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub fn single_chunk_keyed_hash(key: &[u8; KEY_LEN], input: &[u8]) -> Option<[u8; OUT_LEN]> {
    let key_words = words_from_le_bytes_32(key);
    hash_chunk(input, key_words, KEYED_HASH)
}

// The key derivation function for at most a single chunk worth of bytes.
//
// Returns `None` if either context or key material length exceed one chunk.
#[inline]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub fn single_chunk_derive_key(context: &str, key_material: &[u8]) -> Option<[u8; OUT_LEN]> {
    let context_key = hash_chunk(context.as_bytes(), *IV, DERIVE_KEY_CONTEXT)?;
    let context_key_words = words_from_le_bytes_32(&context_key);
    hash_chunk(key_material, context_key_words, DERIVE_KEY_MATERIAL)
}
