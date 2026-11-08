use crate::BlockVerificationError;
use crate::beacon_chain::{check_proof_of_time, is_below_archiving_point};
#[cfg(not(miri))]
use ab_client_proof_of_time::PotNextSlotInput;
use ab_client_proof_of_time::verifier::PotVerifier;
use ab_core_primitives::block::BlockNumber;
use ab_core_primitives::block::header::{
    BlockHeaderFixedConsensusParameters, OwnedBlockHeaderConsensusParameters,
};
use ab_core_primitives::pot::{PotCheckpoints, PotOutput, PotSeed, SlotNumber};
use ab_core_primitives::shard::NumShards;
use ab_core_primitives::solutions::SolutionRange;
use core::assert_matches;
#[cfg(not(miri))]
use core::iter;
use core::num::{NonZeroU16, NonZeroU32};

const BLOCK_AUTHORING_DELAY: u64 = 4;
const SLOT_ITERATIONS: NonZeroU32 = NonZeroU32::new(16).unwrap();
const PARENT_SLOT: u64 = 10;

#[test]
fn below_archiving_point_matches_fork_pruning_block_offset() {
    for block_confirmation_depth in [2, 5, 100] {
        let best_number = 1_000;
        let is_below = |block_offset| {
            is_below_archiving_point(
                BlockNumber::from(best_number - block_offset),
                BlockNumber::from(best_number),
                BlockNumber::from(block_confirmation_depth),
            )
        };

        // The next best block and forks down to block offset `block_confirmation_depth - 2`
        assert!(
            !is_below_archiving_point(
                BlockNumber::from(best_number + 1),
                BlockNumber::from(best_number),
                BlockNumber::from(block_confirmation_depth),
            ),
            "{block_confirmation_depth}"
        );
        assert!(!is_below(0), "{block_confirmation_depth}");
        assert!(
            !is_below(block_confirmation_depth - 2),
            "{block_confirmation_depth}"
        );
        // Block offset `block_confirmation_depth - 1` and deeper
        assert!(
            is_below(block_confirmation_depth - 1),
            "{block_confirmation_depth}"
        );
        assert!(
            is_below(block_confirmation_depth),
            "{block_confirmation_depth}"
        );
    }
}

#[test]
fn below_archiving_point_does_not_overflow() {
    for block_confirmation_depth in [2, 5, 100] {
        for block_number in [BlockNumber::MAX - BlockNumber::ONE, BlockNumber::MAX] {
            assert!(
                !is_below_archiving_point(
                    block_number,
                    BlockNumber::from(1_000),
                    BlockNumber::from(block_confirmation_depth),
                ),
                "{block_confirmation_depth} {block_number}"
            );
        }
    }
}

fn consensus_parameters() -> OwnedBlockHeaderConsensusParameters {
    OwnedBlockHeaderConsensusParameters {
        fixed_parameters: BlockHeaderFixedConsensusParameters {
            solution_range: SolutionRange::MAX,
            slot_iterations: SLOT_ITERATIONS,
            num_shards: NumShards::new(NonZeroU16::MIN, NonZeroU16::MIN).unwrap(),
        },
        super_segment_root: None,
        next_solution_range: None,
        pot_parameters_change: None,
    }
}

/// Checkpoints of slots `1..=num_slots`, starting from the genesis seed
#[cfg(not(miri))]
fn checkpoints_chain(num_slots: usize) -> Vec<PotCheckpoints> {
    let pot_verifier = PotVerifier::new(PotSeed::default(), 100);
    let mut pot_input = PotNextSlotInput {
        slot: SlotNumber::ONE,
        slot_iterations: SLOT_ITERATIONS,
        seed: pot_verifier.genesis_seed(),
    };

    iter::repeat_with(|| {
        let checkpoints = pot_verifier
            .get_checkpoints(pot_input.slot_iterations, pot_input.seed)
            .unwrap();
        pot_input = PotNextSlotInput::derive(
            pot_input.slot_iterations,
            pot_input.slot,
            checkpoints.output(),
            &None,
        );
        checkpoints
    })
    .take(num_slots)
    .collect()
}

/// Check a block with `num_checkpoints` checkpoints and proofs of time that are not valid, only
/// slot checks can be passed this way
fn check_with_bogus_checkpoints(
    parent_slot: u64,
    slot: u64,
    num_checkpoints: usize,
) -> Result<(), BlockVerificationError> {
    let checkpoints = vec![PotCheckpoints::default(); num_checkpoints];

    check_proof_of_time(
        &PotVerifier::new(PotSeed::default(), 100),
        SlotNumber::from(BLOCK_AUTHORING_DELAY),
        SlotNumber::from(parent_slot),
        PotOutput::default(),
        PotOutput::default(),
        &consensus_parameters().as_ref(),
        SlotNumber::from(slot),
        PotOutput::default(),
        checkpoints.last().unwrap().output(),
        &checkpoints,
        false,
    )
}

fn assert_invalid_slot(result: Result<(), BlockVerificationError>, parent_slot: u64, slot: u64) {
    let Err(BlockVerificationError::InvalidSlot {
        slot: error_slot,
        parent_slot: error_parent_slot,
    }) = result
    else {
        panic!("{parent_slot} {slot}: {result:?}");
    };
    assert_eq!(
        (error_slot, error_parent_slot),
        (SlotNumber::from(slot), SlotNumber::from(parent_slot))
    );
}

// TODO: Compile under Miri once a `crossbeam-epoch` release passes Stacked Borrows, until then
//  the test fails there because Rayon's thread pool uses `crossbeam-epoch`:
//  https://github.com/crossbeam-rs/crossbeam/issues/1181
#[test]
#[cfg(not(miri))]
fn valid_after_genesis() {
    let checkpoints = checkpoints_chain(5);

    // Checkpoints from slot 1 up to the future slot 5
    let result = check_proof_of_time(
        &PotVerifier::new(PotSeed::default(), 100),
        SlotNumber::from(BLOCK_AUTHORING_DELAY),
        SlotNumber::ZERO,
        PotOutput::default(),
        PotOutput::default(),
        &consensus_parameters().as_ref(),
        SlotNumber::ONE,
        checkpoints[0].output(),
        checkpoints[4].output(),
        &checkpoints,
        true,
    );
    assert_matches!(result, Ok(()));
}

// TODO: Compile under Miri once a `crossbeam-epoch` release passes Stacked Borrows, until then
//  the test fails there because Rayon's thread pool uses `crossbeam-epoch`:
//  https://github.com/crossbeam-rs/crossbeam/issues/1181
#[test]
#[cfg(not(miri))]
fn valid_after_non_genesis() {
    let checkpoints = checkpoints_chain(7);

    // Parent at slot 1 with future slot 5, block at slot 3 with future slot 7
    let result = check_proof_of_time(
        &PotVerifier::new(PotSeed::default(), 100),
        SlotNumber::from(BLOCK_AUTHORING_DELAY),
        SlotNumber::ONE,
        checkpoints[0].output(),
        checkpoints[4].output(),
        &consensus_parameters().as_ref(),
        SlotNumber::from(3),
        checkpoints[2].output(),
        checkpoints[6].output(),
        &checkpoints[5..],
        true,
    );
    assert_matches!(result, Ok(()));
}

#[test]
fn slot_not_above_parent_slot() {
    for slot in [PARENT_SLOT, PARENT_SLOT - 1] {
        assert_invalid_slot(
            check_with_bogus_checkpoints(PARENT_SLOT, slot, 1),
            PARENT_SLOT,
            slot,
        );
    }
}

#[test]
fn slot_with_overflowing_future_slot() {
    let slot = u64::MAX - 1;
    assert_invalid_slot(
        check_with_bogus_checkpoints(PARENT_SLOT, slot, 3),
        PARENT_SLOT,
        slot,
    );
}

#[test]
fn slot_with_overflowing_future_slot_after_genesis() {
    // Future slot wraps around to the number of checkpoints
    let slot = u64::MAX - 1;
    assert_invalid_slot(check_with_bogus_checkpoints(0, slot, 2), 0, slot);
}

#[test]
fn slot_zero_after_genesis() {
    // Future slot matches the number of checkpoints, while there are no slots between blocks
    assert_invalid_slot(check_with_bogus_checkpoints(0, 0, 4), 0, 0);
}
