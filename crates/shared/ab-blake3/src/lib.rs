//! Optimized and more exotic APIs around BLAKE3

#![no_std]

mod const_fn;

/// The number of bytes in a hash
pub const OUT_LEN: usize = 32;
/// The number of bytes in a key
pub const KEY_LEN: usize = 32;
/// The number of bytes in a block
pub const BLOCK_LEN: usize = 64;

type BlockBytes = [u8; BLOCK_LEN];
type BlockWords = [u32; 16];

pub use const_fn::{const_derive_key, const_hash, const_keyed_hash};
