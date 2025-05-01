//! Verification primitives for Subspace.
#![forbid(unsafe_code)]
#![warn(rust_2018_idioms, missing_debug_implementations, missing_docs)]
#![no_std]

pub mod sr25519;

use ab_core_primitives::block::BlockWeight;
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::solutions::SolutionRange;
use schnorrkel::SignatureError;
use schnorrkel::context::SigningContext;
use sr25519::RewardSignature;

/// Check the reward signature validity.
pub fn check_reward_signature(
    hash: &[u8],
    signature: &RewardSignature,
    public_key_hash: &Blake3Hash,
    reward_signing_context: &SigningContext,
) -> Result<(), SignatureError> {
    if public_key_hash != &signature.public_key.hash() {
        return Err(SignatureError::InvalidKey);
    }
    let public_key = schnorrkel::PublicKey::from_bytes(signature.public_key.as_ref())?;
    let signature = schnorrkel::Signature::from_bytes(signature.signature.as_ref())?;
    public_key.verify(reward_signing_context.bytes(hash), &signature)
}

/// Calculate weight derived from provided solution range
pub fn calculate_block_weight(solution_range: SolutionRange) -> BlockWeight {
    BlockWeight::new(u128::from(u64::from(SolutionRange::MAX - solution_range)))
}
