use crate::{NestedSlots, Slot, SlotKey, Slots};
use ab_aligned_buffer::SharedAlignedBuffer;
use ab_core_primitives::address::Address;

const CONTRACT: Address = Address::SYSTEM_CODE;
const NEW_CONTRACT: Address = Address::SYSTEM_SIMPLE_WALLET_BASE;
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

#[test]
fn reset_rolls_back_changes_of_nested_instances() {
    let mut slots = new_slots();

    {
        let mut parent = slots.new_nested_rw();

        {
            let mut nested = parent.new_nested_rw().unwrap();
            write(&mut nested, OWNERS[0], 10);

            {
                let mut nested_nested = nested.new_nested_rw().unwrap();
                write(&mut nested_nested, OWNERS[1], 20);
            }

            // Written again after the nested instance integrated its changes
            write(&mut nested, OWNERS[1], 21);
        }

        parent.reset();
    }

    assert_eq!(values(&slots), [1, 2, 3]);
    assert_eq!(slots.iter_modified().count(), 0);
}

#[test]
fn reset_restores_state_from_before_instance() {
    let mut slots = new_slots();

    {
        let mut parent = slots.new_nested_rw();

        {
            let mut nested = parent.new_nested_rw().unwrap();
            write(&mut nested, OWNERS[0], 10);
        }

        {
            let mut nested = parent.new_nested_rw().unwrap();

            {
                let mut nested_nested = nested.new_nested_rw().unwrap();
                write(&mut nested_nested, OWNERS[0], 20);
                write(&mut nested_nested, OWNERS[1], 20);
            }

            // Written again after the nested instance integrated its changes
            write(&mut nested, OWNERS[1], 21);
            nested.reset();
        }
    }

    assert_eq!(values(&slots), [10, 2, 3]);
    let mut modified = slots.iter_modified();
    assert_eq!(
        modified.next().map(|(slot_key, _buffer)| *slot_key),
        Some(slot_key_for(OWNERS[0]))
    );
    assert!(modified.next().is_none());
}

#[test]
fn reset_removes_new_slots_and_contracts() {
    let mut slots = new_slots();
    let new_slot_key = SlotKey {
        owner: OWNERS[0],
        contract: NEW_CONTRACT,
    };

    {
        let mut parent = slots.new_nested_rw();

        {
            let mut nested = parent.new_nested_rw().unwrap();
            assert!(nested.add_new_contract(NEW_CONTRACT));

            {
                let mut nested_nested = nested.new_nested_rw().unwrap();
                let (_slot_index, buffer) = nested_nested.use_rw(new_slot_key, 1).unwrap();
                buffer.copy_from_slice(&[10]);
            }

            nested.reset();

            // Neither the slot nor the contract exist anymore
            assert!(nested.use_ro(new_slot_key).is_none());
            assert!(nested.add_new_contract(NEW_CONTRACT));
        }
    }

    assert_eq!(slots.iter().len(), OWNERS.len());
}

#[test]
fn reset_rolls_back_repeated_changes_of_nested_instances() {
    let mut slots = new_slots();

    {
        let mut parent = slots.new_nested_rw();

        {
            let mut nested = parent.new_nested_rw().unwrap();

            for value in 10..20 {
                let mut nested_nested = nested.new_nested_rw().unwrap();
                write(&mut nested_nested, OWNERS[0], value);

                {
                    let mut nested_nested_nested = nested_nested.new_nested_rw().unwrap();
                    write(&mut nested_nested_nested, OWNERS[1], value);
                }
            }

            write(&mut nested, OWNERS[0], 30);
            nested.reset();
        }

        // Changes of the parent itself are kept
        write(&mut parent, OWNERS[2], 40);
    }

    assert_eq!(values(&slots), [1, 2, 40]);
}

#[test]
fn nested_instances_keep_oldest_state_of_slot_once() {
    let mut slots = new_slots();

    {
        let mut parent = slots.new_nested_rw();

        {
            let mut nested = parent.new_nested_rw().unwrap();

            for value in 10..20 {
                let mut nested_nested = nested.new_nested_rw().unwrap();
                write(&mut nested_nested, OWNERS[0], value);
                assert!(nested_nested.use_ro(slot_key_for(OWNERS[1])).is_some());
            }
            {
                let mut nested_nested = nested.new_nested_rw().unwrap();
                let mut nested_nested_nested = nested_nested.new_nested_rw().unwrap();
                write(&mut nested_nested_nested, OWNERS[0], 20);
            }
            write(&mut nested, OWNERS[0], 21);

            // The modification by nested instances and the active access of this instance
            assert_eq!(nested.inner_ro().slot_access.len(), 2);
        }

        assert_eq!(parent.inner_ro().slot_access.len(), 1);
        parent.reset();
    }

    assert_eq!(values(&slots), [1, 2, 3]);
}

#[test]
fn only_latest_access_of_slot_is_active() {
    let mut slots = new_slots();

    {
        let mut parent = slots.new_nested_rw();

        {
            let mut nested = parent.new_nested_rw().unwrap();
            write(&mut nested, OWNERS[0], 10);
        }

        // Accesses of nested instances that integrated their changes don't prevent new accesses
        let (slot_index, _buffer) = parent.use_rw(slot_key_for(OWNERS[0]), 1).unwrap();

        // An active access does
        assert!(parent.use_ro(slot_key_for(OWNERS[0])).is_none());
        {
            let mut nested = parent.new_nested_rw().unwrap();
            assert!(nested.use_ro(slot_key_for(OWNERS[0])).is_none());
            assert!(nested.use_rw(slot_key_for(OWNERS[0]), 1).is_none());
            assert!(nested.get_code(OWNERS[0]).is_none());
        }

        parent
            .access_used_rw(slot_index)
            .unwrap()
            .copy_from_slice(&[11]);
    }

    assert_eq!(values(&slots), [11, 2, 3]);
}
