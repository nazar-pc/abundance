//! Verification primitives for Subspace.
#![forbid(unsafe_code)]
#![warn(rust_2018_idioms, missing_debug_implementations, missing_docs)]
#![no_std]

pub mod ed25519;

use ab_core_primitives::block::BlockWeight;
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::solutions::SolutionRange;
use ed25519::RewardSignature;
use ed25519_zebra::{Error, Signature, VerificationKey};

/// Check the reward signature validity.
pub fn check_reward_signature(
    hash: &Blake3Hash,
    signature: &RewardSignature,
    public_key_hash: &Blake3Hash,
) -> Result<(), Error> {
    if public_key_hash != &signature.public_key.hash() {
        return Err(Error::MalformedPublicKey);
    }

    VerificationKey::try_from(*signature.public_key)?
        .verify(&Signature::from_bytes(&signature.signature), hash.as_ref())
}

/// Calculate weight derived from provided solution range
pub fn calculate_block_weight(solution_range: SolutionRange) -> BlockWeight {
    BlockWeight::new(u128::from(u64::from(SolutionRange::MAX - solution_range)))
}
