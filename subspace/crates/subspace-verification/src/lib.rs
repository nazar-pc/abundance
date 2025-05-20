//! Verification primitives for Subspace.
#![forbid(unsafe_code)]
#![warn(rust_2018_idioms, missing_debug_implementations, missing_docs)]
#![no_std]

pub mod ed25519;

use crate::ed25519::RewardSignature;
use ab_core_primitives::hashes::Blake3Hash;
use ed25519_zebra::Error;

/// Check the reward signature validity.
pub fn check_reward_signature(
    hash: &Blake3Hash,
    signature: &RewardSignature,
    public_key_hash: &Blake3Hash,
) -> Result<(), Error> {
    if public_key_hash != &signature.public_key.hash() {
        return Err(Error::MalformedPublicKey);
    }

    signature
        .public_key
        .verify(&signature.signature, hash.as_ref())
}
