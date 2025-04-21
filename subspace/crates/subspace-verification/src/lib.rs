//! Verification primitives for Subspace.
#![forbid(unsafe_code)]
#![warn(rust_2018_idioms, missing_debug_implementations, missing_docs)]
#![no_std]

use schnorrkel::context::SigningContext;
use schnorrkel::SignatureError;
use subspace_core_primitives::solutions::{RewardSignature, SolutionRange};
use subspace_core_primitives::{BlockWeight, PublicKey};

/// Check the reward signature validity.
pub fn check_reward_signature(
    hash: &[u8],
    signature: &RewardSignature,
    public_key: &PublicKey,
    reward_signing_context: &SigningContext,
) -> Result<(), SignatureError> {
    let public_key = schnorrkel::PublicKey::from_bytes(public_key.as_ref())?;
    let signature = schnorrkel::Signature::from_bytes(signature.as_ref())?;
    public_key.verify(reward_signing_context.bytes(hash), &signature)
}

/// Calculate weight derived from provided solution range
pub fn calculate_block_weight(solution_range: SolutionRange) -> BlockWeight {
    BlockWeight::from(u64::from(SolutionRange::MAX - solution_range))
}
