//! Proof of time implementation.

#![cfg_attr(target_arch = "aarch64", feature(array_chunks))]
#![feature(portable_simd)]
#![no_std]

mod aes;

use ab_core_primitives::pot::{PotCheckpoints, PotSeed};
use core::num::NonZeroU32;

/// Proof of time error
#[derive(Debug, thiserror::Error)]
pub enum PotError {
    /// Iterations are not multiple of number of checkpoints times two
    #[error(
        "Iterations {iterations} are not multiple of number of checkpoints {num_checkpoints} \
        times two"
    )]
    NotMultipleOfCheckpoints {
        /// Slot iterations provided
        iterations: NonZeroU32,
        /// Number of checkpoints
        num_checkpoints: u32,
    },
}

/// Run PoT proving and produce checkpoints.
///
/// Returns error if `iterations` is not a multiple of checkpoints times two.
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub fn prove(seed: PotSeed, iterations: NonZeroU32) -> Result<PotCheckpoints, PotError> {
    if !iterations
        .get()
        .is_multiple_of(u32::from(PotCheckpoints::NUM_CHECKPOINTS.get() * 2))
    {
        return Err(PotError::NotMultipleOfCheckpoints {
            iterations,
            num_checkpoints: u32::from(PotCheckpoints::NUM_CHECKPOINTS.get()),
        });
    }

    // TODO: Is there a point in having both values derived from the same source?
    Ok(aes::create(
        seed,
        seed.key(),
        iterations.get() / u32::from(PotCheckpoints::NUM_CHECKPOINTS.get()),
    ))
}

/// Verify checkpoint, number of iterations is set across uniformly distributed checkpoints.
///
/// Returns error if `iterations` is not a multiple of checkpoints times two.
// TODO: Figure out what is wrong with macOS here
#[cfg_attr(
    all(feature = "no-panic", not(target_os = "macos")),
    no_panic::no_panic
)]
pub fn verify(
    seed: PotSeed,
    iterations: NonZeroU32,
    checkpoints: &PotCheckpoints,
) -> Result<bool, PotError> {
    let num_checkpoints = checkpoints.len() as u32;
    if !iterations.get().is_multiple_of(num_checkpoints * 2) {
        return Err(PotError::NotMultipleOfCheckpoints {
            iterations,
            num_checkpoints,
        });
    }

    Ok(aes::verify_sequential(
        seed,
        seed.key(),
        checkpoints,
        iterations.get() / num_checkpoints,
    ))
}
