//! Client-side proof of time implementation.

pub mod source;
pub mod verifier;

use ab_core_primitives::pot::{PotOutput, PotParametersChange, PotSeed, SlotNumber};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use std::num::NonZeroU32;

/// Next slot input for proof of time evaluation
#[derive(Debug, Copy, Clone, PartialEq, Eq, Decode, Encode, MaxEncodedLen)]
pub struct PotNextSlotInput {
    /// Slot number
    pub slot: SlotNumber,
    /// Slot iterations for this slot
    pub slot_iterations: NonZeroU32,
    /// Seed for this slot
    pub seed: PotSeed,
}

impl PotNextSlotInput {
    /// Derive next slot input while taking parameters change into account.
    ///
    /// NOTE: `base_slot_iterations` doesn't have to be parent block, just something that is after
    /// prior parameters change (if any) took effect, in most cases this value corresponds to parent
    /// block's slot.
    pub fn derive(
        base_slot_iterations: NonZeroU32,
        parent_slot: SlotNumber,
        parent_output: PotOutput,
        pot_parameters_change: &Option<PotParametersChange>,
    ) -> Self {
        let next_slot = parent_slot + SlotNumber::ONE;
        let slot_iterations;
        let seed;

        // The change to number of iterations might have happened before `next_slot`
        if let Some(parameters_change) = pot_parameters_change
            && parameters_change.slot <= next_slot
        {
            slot_iterations = parameters_change.slot_iterations;
            // Only if entropy injection happens exactly on next slot we need to mix it in
            if parameters_change.slot == next_slot {
                seed = parent_output.seed_with_entropy(&parameters_change.entropy);
            } else {
                seed = parent_output.seed();
            }
        } else {
            slot_iterations = base_slot_iterations;
            seed = parent_output.seed();
        }

        Self {
            slot: next_slot,
            slot_iterations,
            seed,
        }
    }
}
