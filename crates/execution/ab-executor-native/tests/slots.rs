//! A contract has a slot for each owner, which `#[slot]` reaches with or without the address of
//! the owner in the method signature, the caller chooses the owner either way

use ab_contracts_common::Contract;
use ab_contracts_common::env::MethodContext;
use ab_contracts_macros::contract;
use ab_core_primitives::address::Address;
use ab_core_primitives::shard::ShardIndex;
use ab_executor_native::NativeExecutor;
use ab_executor_slots::{SlotKey, Slots};
use ab_io_type::maybe_data::MaybeData;
use ab_io_type::trivial_type::TrivialType;
use ab_system_contract_code::CodeExt;

/// Slots of all methods hold an address, since a contract has one slot type and `write_owners()`
/// stores addresses
#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct SlotOwners {
    pub value: u8,
}

#[contract]
impl SlotOwners {
    /// Writes a slot without knowing its owner
    #[update]
    pub fn write(#[slot] slot: &mut MaybeData<'_, Address>, #[input] value: &Address) {
        slot.replace(*value);
    }

    /// Copies the owner of a slot and its contents, if any, to outputs
    #[view]
    pub fn read_with_owner(
        #[slot] (owner, slot): (&Address, &MaybeData<'_, Address>),
        #[output] received_owner: &mut MaybeData<'_, Address>,
        #[output] value: &mut MaybeData<'_, Address>,
    ) {
        received_owner.replace(*owner);
        if let Some(&slot) = slot.get() {
            value.replace(slot);
        }
    }

    /// Writes the owner of each slot into it
    #[update]
    pub fn write_owners(
        #[slot] (first_owner, first): (&Address, &mut MaybeData<'_, Address>),
        #[slot] (second_owner, second): (&Address, &mut MaybeData<'_, Address>),
    ) {
        first.replace(*first_owner);
        second.replace(*second_owner);
    }
}

fn setup() -> (NativeExecutor, Slots, Address) {
    let executor = NativeExecutor::builder(ShardIndex::new(1).unwrap())
        .with_contract::<SlotOwners>()
        .build()
        .unwrap();
    let mut slots = executor.new_storage_slots().unwrap();

    let contract = executor.transaction_emulate(Address::NULL, &mut slots, |env| {
        env.code_deploy(
            MethodContext::Keep,
            Address::SYSTEM_CODE,
            &SlotOwners::code(),
        )
        .unwrap()
    });

    (executor, slots, contract)
}

/// The owner and the value of the slot of `owner`, if they were written
fn read_with_owner(
    executor: &NativeExecutor,
    slots: &Slots,
    contract: Address,
    owner: Address,
) -> (Option<Address>, Option<Address>) {
    let mut received_owner = Address::NULL;
    let mut received_owner_size = 0;
    let mut value = Address::NULL;
    let mut value_size = 0;
    executor
        .with_env_ro(slots, |env| {
            env.slot_owners_read_with_owner(
                contract,
                &owner,
                &mut MaybeData::from_mut(&mut received_owner, &mut received_owner_size).unwrap(),
                &mut MaybeData::from_mut(&mut value, &mut value_size).unwrap(),
            )
        })
        .unwrap();

    (
        (received_owner_size != 0).then_some(received_owner),
        (value_size != 0).then_some(value),
    )
}

#[test]
fn slot_with_address_reads_slot_of_each_owner() {
    let (executor, mut slots, contract) = setup();
    let first_owner = Address::from(100);
    let second_owner = Address::from(101);
    let first_value = Address::from(200);
    let second_value = Address::from(201);

    executor.transaction_emulate(Address::NULL, &mut slots, |env| {
        env.slot_owners_write(MethodContext::Keep, contract, &first_owner, &first_value)
            .unwrap();
        env.slot_owners_write(MethodContext::Keep, contract, &second_owner, &second_value)
            .unwrap();
    });

    assert_eq!(
        read_with_owner(&executor, &slots, contract, first_owner),
        (Some(first_owner), Some(first_value))
    );
    assert_eq!(
        read_with_owner(&executor, &slots, contract, second_owner),
        (Some(second_owner), Some(second_value))
    );
    let never_written = Address::from(102);
    assert_eq!(
        read_with_owner(&executor, &slots, contract, never_written),
        (Some(never_written), None)
    );
}

#[test]
fn slot_with_address_binds_each_address_to_its_slot() {
    let (executor, mut slots, contract) = setup();
    let first_owner = Address::from(100);
    let second_owner = Address::from(101);

    executor.transaction_emulate(Address::NULL, &mut slots, |env| {
        env.slot_owners_write_owners(MethodContext::Keep, contract, &first_owner, &second_owner)
            .unwrap();
    });

    // Read from `Slots` directly, so that generated code can't hide swapped addresses by swapping
    // them back
    for owner in [first_owner, second_owner] {
        let slot_key = SlotKey { owner, contract };
        let (_slot_key, buffer) = slots
            .iter()
            .find(|(key, _buffer)| **key == slot_key)
            .unwrap();
        assert_eq!(buffer.as_slice(), owner.as_bytes());
    }
}
