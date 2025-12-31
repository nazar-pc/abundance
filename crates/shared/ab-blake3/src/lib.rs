#![feature(slice_as_array)]
//! Optimized and more exotic APIs around BLAKE3

#![no_std]

// TODO: Workaround for https://github.com/Rust-GPU/rust-gpu/issues/312
#[cfg(not(target_arch = "spirv"))]
mod const_fn;
// TODO: Workaround for https://github.com/Rust-GPU/rust-gpu/issues/312
#[cfg(not(target_arch = "spirv"))]
mod platform;
mod portable;
mod single_block;
// TODO: Workaround for https://github.com/Rust-GPU/rust-gpu/issues/312
#[cfg(not(target_arch = "spirv"))]
mod single_chunk;

// TODO: Workaround for https://github.com/Rust-GPU/rust-gpu/issues/312
#[cfg(not(target_arch = "spirv"))]
pub use const_fn::{const_derive_key, const_hash, const_keyed_hash};
// TODO: Workaround for https://github.com/Rust-GPU/rust-gpu/issues/312
#[cfg(not(target_arch = "spirv"))]
pub use platform::{le_bytes_from_words_32, words_from_le_bytes_32, words_from_le_bytes_64};
pub use single_block::single_block_hash_portable_words;
// TODO: Workaround for https://github.com/Rust-GPU/rust-gpu/issues/312
#[cfg(not(target_arch = "spirv"))]
pub use single_block::{
    single_block_derive_key, single_block_hash, single_block_hash_many_exact,
    single_block_keyed_hash, single_block_keyed_hash_many_exact,
};
// TODO: Workaround for https://github.com/Rust-GPU/rust-gpu/issues/312
#[cfg(not(target_arch = "spirv"))]
pub use single_chunk::{single_chunk_derive_key, single_chunk_hash, single_chunk_keyed_hash};

/// The number of bytes in a hash
pub const OUT_LEN: usize = 32;
/// The number of bytes in a key
pub const KEY_LEN: usize = 32;
/// The number of bytes in a block
pub const BLOCK_LEN: usize = 64;

/// The number of bytes in a chunk, 1024.
///
/// You don't usually need to think about this number, but it often comes up in benchmarks, because
/// the maximum degree of parallelism used by the implementation equals the number of chunks.
// TODO: Workaround for https://github.com/Rust-GPU/rust-gpu/issues/312
#[cfg(not(target_arch = "spirv"))]
pub const CHUNK_LEN: usize = 1024;

// While iterating the compression function within a chunk, the CV is
// represented as words, to avoid doing two extra endianness conversions for
// each compression in the portable implementation. But the hash_many interface
// needs to hash both input bytes and parent nodes, so its better for its
// output CVs to be represented as bytes.
type CVWords = [u32; 8];
// TODO: Workaround for https://github.com/Rust-GPU/rust-gpu/issues/312
#[cfg(not(target_arch = "spirv"))]
type CVBytes = [u8; 32]; // little-endian

// TODO: Workaround for https://github.com/Rust-GPU/rust-gpu/issues/312
#[cfg(not(target_arch = "spirv"))]
type BlockBytes = [u8; BLOCK_LEN];
type BlockWords = [u32; 16];

const IV: &CVWords = &[
    0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A, 0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19,
];

const MSG_SCHEDULE: [[usize; 16]; 7] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8],
    [3, 4, 10, 12, 13, 2, 7, 14, 6, 5, 9, 0, 11, 15, 8, 1],
    [10, 7, 12, 9, 14, 3, 13, 15, 4, 0, 11, 2, 5, 8, 1, 6],
    [12, 13, 9, 11, 15, 10, 14, 8, 7, 2, 5, 3, 0, 1, 6, 4],
    [9, 14, 11, 5, 8, 12, 15, 1, 13, 3, 0, 10, 2, 6, 4, 7],
    [11, 15, 5, 0, 1, 9, 8, 6, 14, 10, 2, 12, 3, 4, 7, 13],
];

// These are the internal flags that we use to domain separate root/non-root,
// chunk/parent, and chunk beginning/middle/end. These get set at the high end
// of the block flags word in the compression function, so their values start
// high and go down.
const CHUNK_START: u8 = 1 << 0;
const CHUNK_END: u8 = 1 << 1;
// TODO: Workaround for https://github.com/Rust-GPU/rust-gpu/issues/312
#[cfg(not(target_arch = "spirv"))]
const PARENT: u8 = 1 << 2;
const ROOT: u8 = 1 << 3;
// TODO: Workaround for https://github.com/Rust-GPU/rust-gpu/issues/312
#[cfg(not(target_arch = "spirv"))]
const KEYED_HASH: u8 = 1 << 4;
// TODO: Workaround for https://github.com/Rust-GPU/rust-gpu/issues/312
#[cfg(not(target_arch = "spirv"))]
const DERIVE_KEY_CONTEXT: u8 = 1 << 5;
// TODO: Workaround for https://github.com/Rust-GPU/rust-gpu/issues/312
#[cfg(not(target_arch = "spirv"))]
const DERIVE_KEY_MATERIAL: u8 = 1 << 6;
