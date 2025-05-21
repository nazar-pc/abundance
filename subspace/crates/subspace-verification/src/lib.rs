//! Verification primitives for Subspace.
#![forbid(unsafe_code)]
#![warn(rust_2018_idioms, missing_debug_implementations, missing_docs)]
#![no_std]

pub mod ed25519;

use crate::ed25519::RewardSignature;
use ab_core_primitives::hashes::Blake3Hash;

/// Check the reward signature validity.
pub fn is_reward_signature_valid(
    hash: &Blake3Hash,
    signature: &RewardSignature,
    public_key_hash: &Blake3Hash,
) -> bool {
    if public_key_hash != &signature.public_key.hash() {
        return false;
    }

    signature
        .public_key
        .verify(&signature.signature, hash.as_ref())
        .is_ok()
}
