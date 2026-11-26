use crate::source::state::PotState;
use crate::source::timekeeper::Timekeeper;
use crate::test_utils::{GENESIS_SEED, first_slot_input};
use crate::verifier::PotVerifier;
use ab_core_primitives::pot::{PotSeed, SlotNumber};
use futures::channel::mpsc;
use std::assert_matches;

#[test]
fn proof_is_sent_only_if_chain_was_extended() {
    let verifier = PotVerifier::new(PotSeed::from(GENESIS_SEED), 100);
    let state = PotState::new(first_slot_input(), None, verifier.clone());
    let (mut proof_sender, mut proof_receiver) = mpsc::channel(1);

    let next_slot_input =
        Timekeeper::prove_slot(&state, &verifier, &mut proof_sender, first_slot_input())
            .unwrap()
            .unwrap();
    assert_eq!(next_slot_input.slot, SlotNumber::from(2));
    assert_eq!(state.next_slot_input(), next_slot_input);
    assert_eq!(proof_receiver.try_recv().unwrap().slot, SlotNumber::ONE);

    // The chain was extended in the meantime, so the input of slot 1 is stale
    assert_eq!(
        Timekeeper::prove_slot(&state, &verifier, &mut proof_sender, first_slot_input())
            .unwrap()
            .unwrap(),
        next_slot_input
    );
    assert_eq!(state.next_slot_input(), next_slot_input);
    assert_matches!(proof_receiver.try_recv(), Err(mpsc::TryRecvError::Empty));
}
