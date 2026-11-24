use crate::PotNextSlotInput;
use crate::source::state::PotState;
use crate::test_utils::{GENESIS_SEED, SLOT_ITERATIONS, first_slot_input, prove_slots};
use crate::verifier::PotVerifier;
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pot::{PotParametersChange, PotSeed, SlotNumber};

#[test]
fn is_on_current_chain() {
    // Slots 1..=3 of the chain and slots 2..=4 of the fork with entropy injected at slot 2
    let verifier = PotVerifier::new(PotSeed::from(GENESIS_SEED), 100);
    let chain = prove_slots(first_slot_input(), 3);
    let parameters_change = PotParametersChange {
        slot: SlotNumber::from(2),
        slot_iterations: SLOT_ITERATIONS,
        entropy: Blake3Hash::new([1; _]),
    };
    let fork = prove_slots(
        PotNextSlotInput::derive(
            SLOT_ITERATIONS,
            SlotNumber::ONE,
            chain[0].1.output(),
            &Some(parameters_change),
        ),
        3,
    );
    for (slot_input, checkpoints) in chain.iter().chain(&fork) {
        verifier.inject_verified_checkpoints(
            slot_input.seed,
            slot_input.slot_iterations,
            *checkpoints,
        );
    }

    let state = PotState::new(first_slot_input(), None, verifier);
    // Other slots of the chain are already known, so the tip moves to slot 4
    let next_slot_input = state
        .try_extend(
            first_slot_input(),
            SlotNumber::ONE,
            chain[0].1.output(),
            None,
        )
        .unwrap();
    assert_eq!(next_slot_input.slot, SlotNumber::from(4));

    for (slot_input, checkpoints) in &chain {
        assert!(state.is_on_current_chain(*slot_input, checkpoints.output()));
    }
    for (slot_input, checkpoints) in &fork {
        assert!(!state.is_on_current_chain(*slot_input, checkpoints.output()));
    }
    // Any output is fine for the next slot as long as the input matches
    assert!(state.is_on_current_chain(next_slot_input, fork[2].1.output()));

    // Reorg to the fork, whose tip is at slot 5
    state.set_known_good_output(
        SlotNumber::from(2),
        fork[0].1.output(),
        Some(parameters_change),
    );
    assert_eq!(state.next_slot_input().slot, SlotNumber::from(5));

    assert!(state.is_on_current_chain(chain[0].0, chain[0].1.output()));
    for (slot_input, checkpoints) in &chain[1..] {
        assert!(!state.is_on_current_chain(*slot_input, checkpoints.output()));
    }
    for (slot_input, checkpoints) in &fork {
        assert!(state.is_on_current_chain(*slot_input, checkpoints.output()));
    }
}
