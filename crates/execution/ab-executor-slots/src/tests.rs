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

/// Same as [`new_slots()`], but the slot of the first owner is read-only
fn new_slots_with_read_only() -> Slots {
    Slots::new(OWNERS.iter().zip(1..).map(|(&owner, value)| {
        let key = slot_key_for(owner);
        let buffer = SharedAlignedBuffer::from_bytes(&[value]);

        if owner == OWNERS[0] {
            Slot::ReadOnly { key, buffer }
        } else {
            Slot::ReadWrite { key, buffer }
        }
    }))
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
fn drop_resets_changes() {
    let mut slots = new_slots();
    let new_slot_key = SlotKey {
        owner: OWNERS[0],
        contract: NEW_CONTRACT,
    };

    {
        let mut nested = slots.new_nested_rw();
        write(&mut nested, OWNERS[0], 10);
        assert!(nested.add_new_contract(NEW_CONTRACT));
        let (_slot_index, buffer) = nested.use_rw(new_slot_key, 1).unwrap();
        buffer.copy_from_slice(&[10]);
    }

    assert_eq!(values(&slots), [1, 2, 3]);
    assert_eq!(slots.iter().len(), OWNERS.len());
    assert_eq!(slots.iter_modified().count(), 0);
    // The new contract doesn't exist anymore
    assert!(slots.add_new_contract(NEW_CONTRACT));
}

#[test]
fn commit_integrates_changes() {
    let mut slots = new_slots();
    let new_slot_key = SlotKey {
        owner: OWNERS[0],
        contract: NEW_CONTRACT,
    };
    let tmp_slot_key = SlotKey {
        owner: OWNERS[0],
        contract: Address::NULL,
    };

    {
        let mut nested = slots.new_nested_rw();
        write(&mut nested, OWNERS[0], 10);
        assert!(nested.add_new_contract(NEW_CONTRACT));
        let (_slot_index, buffer) = nested.use_rw(new_slot_key, 1).unwrap();
        buffer.copy_from_slice(&[10]);
        let (_slot_index, buffer) = nested.use_rw(tmp_slot_key, 1).unwrap();
        buffer.copy_from_slice(&[10]);
        nested.commit();
    }

    assert_eq!(values(&slots), [10, 2, 3]);
    // The new slot is kept, while the `#[tmp]` one is removed
    assert_eq!(slots.iter().len(), OWNERS.len() + 1);
    assert!(
        slots
            .iter_modified()
            .any(|(slot_key, _buffer)| *slot_key == new_slot_key)
    );
    // The new contract still exists
    assert!(!slots.add_new_contract(NEW_CONTRACT));
}

#[test]
fn commit_integrates_changes_of_nested_instances() {
    let mut slots = new_slots();

    {
        let mut parent = slots.new_nested_rw();

        {
            let mut nested = parent.new_nested_rw().unwrap();
            write(&mut nested, OWNERS[0], 10);

            {
                let mut nested_nested = nested.new_nested_rw().unwrap();
                write(&mut nested_nested, OWNERS[1], 20);
                nested_nested.commit();
            }

            nested.commit();
        }

        parent.commit();
    }

    assert_eq!(values(&slots), [10, 20, 3]);
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
        }

        // The parent still has the slot open for writing after the nested instance was reset
        assert!(parent.use_ro(slot_key_for(OWNERS[0])).is_none());
        parent
            .access_used_rw(slot_index)
            .unwrap()
            .copy_from_slice(&[11]);
        parent.commit();
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
                nested_nested.commit();
            }

            // Written again after the nested instance integrated its changes
            write(&mut nested, OWNERS[1], 21);
            nested.commit();
        }
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
            nested.commit();
        }

        {
            let mut nested = parent.new_nested_rw().unwrap();

            {
                let mut nested_nested = nested.new_nested_rw().unwrap();
                write(&mut nested_nested, OWNERS[0], 20);
                write(&mut nested_nested, OWNERS[1], 20);
                nested_nested.commit();
            }

            // Written again after the nested instance integrated its changes
            write(&mut nested, OWNERS[1], 21);
        }

        parent.commit();
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
                nested_nested.commit();
            }
        }

        // Neither the slot nor the contract exist anymore
        assert!(parent.use_ro(new_slot_key).is_none());
        assert!(parent.add_new_contract(NEW_CONTRACT));
        parent.commit();
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
                    nested_nested_nested.commit();
                }

                nested_nested.commit();
            }

            write(&mut nested, OWNERS[0], 30);
        }

        // Changes of the parent itself are kept
        write(&mut parent, OWNERS[2], 40);
        parent.commit();
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
                nested_nested.commit();
            }
            {
                let mut nested_nested = nested.new_nested_rw().unwrap();
                let mut nested_nested_nested = nested_nested.new_nested_rw().unwrap();
                write(&mut nested_nested_nested, OWNERS[0], 20);
                nested_nested_nested.commit();
                nested_nested.commit();
            }
            write(&mut nested, OWNERS[0], 21);

            // The modification by nested instances and the active access of this instance
            assert_eq!(nested.inner_ro().slot_access.len(), 2);
            nested.commit();
        }

        assert_eq!(parent.inner_ro().slot_access.len(), 1);
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
            nested.commit();
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
        parent.commit();
    }

    assert_eq!(values(&slots), [11, 2, 3]);
}

#[test]
fn read_only_slot_stays_read_only_after_read() {
    let mut slots = new_slots_with_read_only();
    let read_only_slot_key = slot_key_for(OWNERS[0]);

    {
        let mut nested = slots.new_nested_rw();
        assert!(nested.use_ro(read_only_slot_key).is_some());
    }
    {
        let mut nested = slots.new_nested_rw();
        assert!(nested.use_ro(read_only_slot_key).is_some());
        nested.commit();
    }

    let mut nested = slots.new_nested_rw();
    assert!(nested.use_rw(read_only_slot_key, 1).is_none());
}

#[test]
fn read_only_slot_stays_read_only_after_refused_write() {
    let mut slots = new_slots_with_read_only();
    let read_only_slot_key = slot_key_for(OWNERS[0]);

    {
        let mut nested = slots.new_nested_rw();
        assert!(nested.use_rw(read_only_slot_key, 1).is_none());
    }
    {
        let mut nested = slots.new_nested_rw();
        assert!(nested.use_rw(read_only_slot_key, 1).is_none());
        nested.commit();
    }

    let mut nested = slots.new_nested_rw();
    assert!(nested.use_rw(read_only_slot_key, 1).is_none());
    // It can still be read
    assert!(nested.use_ro(read_only_slot_key).is_some());
}

#[test]
fn read_only_slots_are_iterated_as_unmodified() {
    let mut slots = new_slots_with_read_only();

    {
        let mut nested = slots.new_nested_rw();
        write(&mut nested, OWNERS[1], 20);
        nested.commit();
    }

    assert_eq!(values(&slots), [1, 20, 3]);
    assert!(
        slots
            .iter_modified()
            .map(|(slot_key, _buffer)| *slot_key)
            .eq([slot_key_for(OWNERS[1])])
    );
    assert!(
        slots
            .into_slots()
            .any(|(slot_key, buffer)| slot_key == slot_key_for(OWNERS[0])
                && buffer.as_slice() == [1])
    );
}
