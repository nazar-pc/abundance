use crate::BlockVerificationError;
use crate::shared::{BlockSlots, derive_block_slots};
use ab_core_primitives::pot::SlotNumber;

const BLOCK_AUTHORING_DELAY: u64 = 4;

fn assert_invalid_slot(parent_slot: u64, slot: u64) {
    let result = derive_block_slots(
        SlotNumber::from(BLOCK_AUTHORING_DELAY),
        SlotNumber::from(parent_slot),
        SlotNumber::from(slot),
    );
    let Err(BlockVerificationError::InvalidSlot {
        slot: error_slot,
        parent_slot: error_parent_slot,
    }) = result
    else {
        panic!("{parent_slot} {slot}: {result:?}");
    };
    assert_eq!(
        (error_slot, error_parent_slot),
        (SlotNumber::from(slot), SlotNumber::from(parent_slot)),
        "{parent_slot} {slot}"
    );
}

#[test]
fn slot_above_parent_slot() {
    // Parent block's future slot is its slot plus the delay, except for the genesis block
    let cases = [(5, 6, 9), (5, 100, 9), (0, 1, 0), (0, 100, 0)];
    for (parent_slot, slot, parent_future_slot) in cases {
        assert_eq!(
            derive_block_slots(
                SlotNumber::from(BLOCK_AUTHORING_DELAY),
                SlotNumber::from(parent_slot),
                SlotNumber::from(slot),
            )
            .unwrap(),
            BlockSlots {
                future_slot: SlotNumber::from(slot + BLOCK_AUTHORING_DELAY),
                parent_future_slot: SlotNumber::from(parent_future_slot),
                slots_between_blocks: SlotNumber::from(slot - parent_slot),
            },
            "{parent_slot} {slot}"
        );
    }
}

#[test]
fn slot_not_above_parent_slot() {
    for (parent_slot, slot) in [(0, 0), (5, 5), (5, 4), (u64::MAX, 0), (u64::MAX, u64::MAX)] {
        assert_invalid_slot(parent_slot, slot);
    }
}

#[test]
fn future_slot_overflow() {
    // The largest slot accepted
    let slot = u64::MAX - BLOCK_AUTHORING_DELAY;
    let cases = [(0, 0), (slot - 1, slot - 1 + BLOCK_AUTHORING_DELAY)];
    for (parent_slot, parent_future_slot) in cases {
        assert_eq!(
            derive_block_slots(
                SlotNumber::from(BLOCK_AUTHORING_DELAY),
                SlotNumber::from(parent_slot),
                SlotNumber::from(slot),
            )
            .unwrap(),
            BlockSlots {
                future_slot: SlotNumber::MAX,
                parent_future_slot: SlotNumber::from(parent_future_slot),
                slots_between_blocks: SlotNumber::from(slot - parent_slot),
            },
            "{parent_slot}"
        );
    }

    // The smallest slot rejected and above
    for parent_slot in [0, 10, u64::MAX - BLOCK_AUTHORING_DELAY] {
        for slot in [u64::MAX - BLOCK_AUTHORING_DELAY + 1, u64::MAX] {
            assert_invalid_slot(parent_slot, slot);
        }
    }
}
