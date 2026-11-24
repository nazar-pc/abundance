use crate::PotNextSlotInput;
use ab_core_primitives::pot::{PotCheckpoints, PotSeed, SlotNumber};
use std::iter;
use std::num::NonZeroU32;

pub(crate) const GENESIS_SEED: [u8; PotSeed::SIZE] = [
    0xd6, 0x66, 0xcc, 0xd8, 0xd5, 0x93, 0xc2, 0x3d, 0xa8, 0xdb, 0x6b, 0x5b, 0x14, 0x13, 0xb1, 0x3a,
];
pub(crate) const SLOT_ITERATIONS: NonZeroU32 = NonZeroU32::new(16).unwrap();

/// Input of the first slot after genesis
pub(crate) fn first_slot_input() -> PotNextSlotInput {
    PotNextSlotInput {
        slot: SlotNumber::ONE,
        slot_iterations: SLOT_ITERATIONS,
        seed: PotSeed::from(GENESIS_SEED),
    }
}

/// Prove `num_slots` consecutive slots starting with `slot_input`, returns input and checkpoints of
/// each slot
pub(crate) fn prove_slots(
    mut slot_input: PotNextSlotInput,
    num_slots: usize,
) -> Vec<(PotNextSlotInput, PotCheckpoints)> {
    iter::repeat_with(|| {
        let checkpoints =
            ab_proof_of_time::prove(slot_input.seed, slot_input.slot_iterations).unwrap();
        let proof = (slot_input, checkpoints);
        slot_input = PotNextSlotInput::derive(
            slot_input.slot_iterations,
            slot_input.slot,
            checkpoints.output(),
            &None,
        );
        proof
    })
    .take(num_slots)
    .collect()
}
