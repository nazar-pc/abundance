//! BLAKE3 functions that process at most a single block.
//!
//! This module and submodules are copied with modifications from the official [`blake3`] crate, but
//! is unlikely to be upstreamed.

#[cfg(test)]
mod tests;

#[cfg(not(target_arch = "spirv"))]
use crate::platform::{le_bytes_from_words_32, words_from_le_bytes_32};
#[cfg(not(target_arch = "spirv"))]
use crate::{BLOCK_LEN, DERIVE_KEY_CONTEXT, DERIVE_KEY_MATERIAL, KEY_LEN, KEYED_HASH, OUT_LEN};
use crate::{BlockWords, CHUNK_END, CHUNK_START, CVWords, IV, ROOT, portable};
// TODO: Workaround for https://github.com/Rust-GPU/rust-gpu/issues/312
#[cfg(not(target_arch = "spirv"))]
use blake3::platform::Platform;

// Hash a single block worth of values
// TODO: Workaround for https://github.com/Rust-GPU/rust-gpu/issues/312
#[cfg(not(target_arch = "spirv"))]
#[inline(always)]
fn hash_block(input: &[u8], key: CVWords, flags: u8) -> Option<[u8; OUT_LEN]> {
    // If the whole subtree is one block, hash it directly with a ChunkState.
    if input.len() > BLOCK_LEN {
        return None;
    }

    let mut cv = key;

    let mut block = [0; BLOCK_LEN];
    block[..input.len()].copy_from_slice(input);
    Platform::detect().compress_in_place(
        &mut cv,
        &block,
        input.len() as u8,
        0,
        flags | CHUNK_START | CHUNK_END | ROOT,
    );

    Some(*le_bytes_from_words_32(&cv))
}

/// Hashing function for at most a single block worth of bytes.
///
/// Returns `None` if input length exceeds one block.
// TODO: Workaround for https://github.com/Rust-GPU/rust-gpu/issues/312
#[cfg(not(target_arch = "spirv"))]
#[inline]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub fn single_block_hash(input: &[u8]) -> Option<[u8; OUT_LEN]> {
    hash_block(input, *IV, 0)
}

/// The keyed hash function for at most a single block worth of bytes.
///
/// Returns `None` if input length exceeds one block.
// TODO: Workaround for https://github.com/Rust-GPU/rust-gpu/issues/312
#[cfg(not(target_arch = "spirv"))]
#[inline]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub fn single_block_keyed_hash(key: &[u8; KEY_LEN], input: &[u8]) -> Option<[u8; OUT_LEN]> {
    let key_words = words_from_le_bytes_32(key);
    hash_block(input, key_words, KEYED_HASH)
}

// The key derivation function for at most a single block worth of bytes.
//
// Returns `None` if either context or key material length exceed one block.
// TODO: Workaround for https://github.com/Rust-GPU/rust-gpu/issues/312
#[cfg(not(target_arch = "spirv"))]
#[inline]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub fn single_block_derive_key(context: &str, key_material: &[u8]) -> Option<[u8; OUT_LEN]> {
    let context_key = hash_block(context.as_bytes(), *IV, DERIVE_KEY_CONTEXT)?;
    let context_key_words = words_from_le_bytes_32(&context_key);
    hash_block(key_material, context_key_words, DERIVE_KEY_MATERIAL)
}

/// Hashing function for at most a single block worth of words using portable implementation.
///
/// This API operates on words and is GPU-friendly.
///
/// `num_bytes` specifies how many actual bytes are occupied by useful value in `input`. Bytes
/// outside that must be set to `0`.
///
/// NOTE: If unused bytes are not set to `0` or invalid number of bytes is specified, it'll simply
/// result in invalid hash.
///
/// [`words_from_le_bytes_32()`], [`words_from_le_bytes_64()`] and [`le_bytes_from_words_32()`] can
/// be used to convert bytes to words and back if necessary.
///
/// [`words_from_le_bytes_64()`]: crate::words_from_le_bytes_64
#[inline]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub fn single_block_hash_portable_words(input: &BlockWords, num_bytes: u32) -> CVWords {
    let mut cv = *IV;

    portable::compress_in_place_u32(
        &mut cv,
        input,
        num_bytes,
        0,
        (CHUNK_START | CHUNK_END | ROOT) as u32,
    );

    cv
}
