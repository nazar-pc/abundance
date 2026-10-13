use crate::{NestedSlots, Slot, SlotKey, Slots};
use ab_aligned_buffer::SharedAlignedBuffer;
use ab_core_primitives::address::Address;

const CONTRACT: Address = Address::SYSTEM_CODE;
const OWNERS: [Address; 3] = [
    Address::SYSTEM_STATE,
    Address::SYSTEM_BLOCK,
    Address::SYSTEM_NATIVE_TOKEN,
];

fn slot_key_for(owner: Address) -> SlotKey {
    SlotKey {
        owner,
        contract: CONTRACT,
    }
}

/// Slots of [`OWNERS`] with values `1`, `2` and `3`
fn new_slots() -> Slots {
    Slots::new(
        OWNERS
            .iter()
            .zip(1..)
            .map(|(&owner, value)| Slot::ReadWrite {
                key: slot_key_for(owner),
                buffer: SharedAlignedBuffer::from_bytes(&[value]),
            }),
    )
}

fn write(slots: &mut NestedSlots<'_>, owner: Address, value: u8) {
    let (_slot_index, buffer) = slots.use_rw(slot_key_for(owner), 1).unwrap();
    buffer.copy_from_slice(&[value]);
}

/// Values of slots of [`OWNERS`]
fn values(slots: &Slots) -> [u8; 3] {
    OWNERS.map(|owner| {
        let (_slot_key, buffer) = slots
            .iter()
            .find(|(slot_key, _buffer)| **slot_key == slot_key_for(owner))
            .unwrap();
        *buffer.as_slice().first().unwrap()
    })
}

#[test]
fn reset_keeps_accesses_of_parent() {
    let mut slots = new_slots();

    {
        let mut parent = slots.new_nested_rw();
        let (slot_index, buffer) = parent.use_rw(slot_key_for(OWNERS[0]), 1).unwrap();
        buffer.copy_from_slice(&[10]);

        {
            let mut nested = parent.new_nested_rw().unwrap();
            write(&mut nested, OWNERS[1], 20);
            nested.reset();
        }

        // The parent still has the slot open for writing after the nested instance was reset
        assert!(parent.use_ro(slot_key_for(OWNERS[0])).is_none());
        parent
            .access_used_rw(slot_index)
            .unwrap()
            .copy_from_slice(&[11]);
    }

    assert_eq!(values(&slots), [11, 2, 3]);
}
