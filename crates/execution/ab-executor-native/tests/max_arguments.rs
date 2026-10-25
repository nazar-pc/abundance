#![feature(const_convert, const_trait_impl)]

use ab_contracts_common::env::MethodContext;
use ab_contracts_common::{Contract, MAX_TOTAL_METHOD_ARGS};
use ab_contracts_macros::contract;
use ab_core_primitives::address::Address;
use ab_core_primitives::shard::ShardIndex;
use ab_executor_native::NativeExecutor;
use ab_executor_slots::Slots;
use ab_io_type::IoType;
use ab_io_type::maybe_data::MaybeData;
use ab_io_type::trivial_type::TrivialType;
use ab_system_contract_code::CodeExt;

const OWNERS: [Address; usize::from(MAX_TOTAL_METHOD_ARGS)] = [
    Address::from(100),
    Address::from(101),
    Address::from(102),
    Address::from(103),
    Address::from(104),
    Address::from(105),
    Address::from(106),
    Address::from(107),
];

#[derive(Debug, Copy, Clone, PartialEq, Eq, TrivialType)]
#[repr(C)]
pub struct Value {
    pub value: u64,
}

fn increment(slot: &mut MaybeData<'_, Value>) {
    let value = slot.get().map_or(0, |slot| slot.value);
    slot.replace(Value { value: value + 1 });
}

/// Contract whose methods have as many arguments as `MAX_TOTAL_METHOD_ARGS` allows.
///
/// The arguments are `#[slot]`s with write access, which take the most space in buffers of the
/// executor.
#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct MaxArguments {
    pub value: u64,
}

#[contract]
impl MaxArguments {
    #[init]
    pub fn init() -> Self {
        Self { value: 0 }
    }

    /// Adds one to each slot.
    ///
    /// A method without `self` can use all of `MAX_TOTAL_METHOD_ARGS` for its arguments.
    #[update]
    #[expect(
        clippy::too_many_arguments,
        reason = "The max number of arguments is tested"
    )]
    pub fn increment_slots(
        #[slot] first: &mut MaybeData<'_, Value>,
        #[slot] second: &mut MaybeData<'_, Value>,
        #[slot] third: &mut MaybeData<'_, Value>,
        #[slot] fourth: &mut MaybeData<'_, Value>,
        #[slot] fifth: &mut MaybeData<'_, Value>,
        #[slot] sixth: &mut MaybeData<'_, Value>,
        #[slot] seventh: &mut MaybeData<'_, Value>,
        #[slot] eighth: &mut MaybeData<'_, Value>,
    ) {
        // Each slot has its own lifetime, so the slots can't be put into one array as they are.
        // `reborrow_mut()` returns views whose lifetimes can be shortened to a common one.
        for mut slot in [
            first.reborrow_mut(),
            second.reborrow_mut(),
            third.reborrow_mut(),
            fourth.reborrow_mut(),
            fifth.reborrow_mut(),
            sixth.reborrow_mut(),
            seventh.reborrow_mut(),
            eighth.reborrow_mut(),
        ] {
            increment(&mut slot);
        }
    }

    /// Adds one to the state and each slot.
    ///
    /// `&mut self` counts towards `MAX_TOTAL_METHOD_ARGS`, so there is one slot less.
    #[update]
    #[expect(
        clippy::too_many_arguments,
        reason = "The max number of arguments is tested"
    )]
    pub fn increment_state_and_slots(
        &mut self,
        #[slot] first: &mut MaybeData<'_, Value>,
        #[slot] second: &mut MaybeData<'_, Value>,
        #[slot] third: &mut MaybeData<'_, Value>,
        #[slot] fourth: &mut MaybeData<'_, Value>,
        #[slot] fifth: &mut MaybeData<'_, Value>,
        #[slot] sixth: &mut MaybeData<'_, Value>,
        #[slot] seventh: &mut MaybeData<'_, Value>,
    ) {
        self.value += 1;
        increment(first);
        increment(second);
        increment(third);
        increment(fourth);
        increment(fifth);
        increment(sixth);
        increment(seventh);
    }

    #[view]
    pub fn value(&self) -> u64 {
        self.value
    }

    /// Value of a slot, `0` if it is empty
    #[view]
    pub fn read(#[slot] slot: &MaybeData<'_, Value>) -> u64 {
        slot.get().map_or(0, |slot| slot.value)
    }
}

fn setup() -> (NativeExecutor, Slots, Address) {
    let executor = NativeExecutor::builder(ShardIndex::new(1).unwrap())
        .with_contract::<MaxArguments>()
        .build()
        .unwrap();
    let mut slots = executor.new_storage_slots().unwrap();

    let contract = executor.transaction_emulate(Address::NULL, &mut slots, |env| {
        let contract = env
            .code_deploy(
                MethodContext::Keep,
                Address::SYSTEM_CODE,
                &MaxArguments::code(),
            )
            .unwrap();
        env.max_arguments_init(MethodContext::Keep, contract)
            .unwrap();
        contract
    });

    (executor, slots, contract)
}

fn read_all(
    executor: &NativeExecutor,
    slots: &Slots,
    contract: Address,
) -> [u64; usize::from(MAX_TOTAL_METHOD_ARGS)] {
    executor.with_env_ro(slots, |env| {
        OWNERS.map(|owner| env.max_arguments_read(contract, &owner).unwrap())
    })
}

#[test]
fn method_without_self_has_max_arguments() {
    let (executor, mut slots, contract) = setup();

    executor.transaction_emulate(Address::NULL, &mut slots, |env| {
        env.max_arguments_increment_slots(
            MethodContext::Keep,
            contract,
            &OWNERS[0],
            &OWNERS[1],
            &OWNERS[2],
            &OWNERS[3],
            &OWNERS[4],
            &OWNERS[5],
            &OWNERS[6],
            &OWNERS[7],
        )
        .unwrap();
    });

    assert_eq!(
        read_all(&executor, &slots, contract),
        [1; usize::from(MAX_TOTAL_METHOD_ARGS)]
    );
}

#[test]
fn method_with_self_has_one_argument_less() {
    let (executor, mut slots, contract) = setup();

    executor.transaction_emulate(Address::NULL, &mut slots, |env| {
        env.max_arguments_increment_state_and_slots(
            MethodContext::Keep,
            contract,
            &OWNERS[0],
            &OWNERS[1],
            &OWNERS[2],
            &OWNERS[3],
            &OWNERS[4],
            &OWNERS[5],
            &OWNERS[6],
        )
        .unwrap();
    });

    assert_eq!(
        read_all(&executor, &slots, contract),
        [1, 1, 1, 1, 1, 1, 1, 0]
    );
    let value = executor.with_env_ro(&slots, |env| env.max_arguments_value(contract).unwrap());
    assert_eq!(value, 1);
}
