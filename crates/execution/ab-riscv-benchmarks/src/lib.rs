#![no_std]

#[cfg(not(target_env = "abundance"))]
pub mod host_utils;

use ab_blake3::{CHUNK_LEN, OUT_LEN, single_chunk_hash};
use ab_contracts_macros::contract;
use ab_core_primitives::ed25519::{Ed25519PublicKey, Ed25519Signature};
use ab_io_type::bool::Bool;
use ab_io_type::trivial_type::TrivialType;

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct Benchmarks;

#[contract]
impl Benchmarks {
    /// Hash a single chunk worth of bytes
    #[view]
    pub fn blake3_hash_chunk(#[input] chunk: &[u8; CHUNK_LEN]) -> [u8; OUT_LEN] {
        single_chunk_hash(chunk).expect("Exactly one chunk; qed")
    }

    /// Verify a single Ed25519 signature
    #[view]
    pub fn ed25519_verify(
        #[input] public_key: &Ed25519PublicKey,
        #[input] signature: &Ed25519Signature,
        #[input] message: &[u8; OUT_LEN],
    ) -> Bool {
        Bool::new(public_key.verify(signature, message).is_ok())
    }
}
