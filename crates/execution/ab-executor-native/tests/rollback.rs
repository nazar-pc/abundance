use ab_contracts_common::env::{Env, MethodContext};
use ab_contracts_common::{Contract, ContractError, MAX_CODE_SIZE};
use ab_contracts_macros::contract;
use ab_core_primitives::address::Address;
use ab_core_primitives::shard::ShardIndex;
use ab_executor_native::NativeExecutor;
use ab_executor_slots::Slots;
use ab_io_type::maybe_data::MaybeData;
use ab_io_type::trivial_type::TrivialType;
use ab_io_type::variable_bytes::VariableBytes;
use ab_system_contract_code::CodeExt;

const OWNERS: [Address; 4] = [
    Address::SYSTEM_STATE,
    Address::SYSTEM_BLOCK,
    Address::SYSTEM_NATIVE_TOKEN,
    Address::SYSTEM_SIMPLE_WALLET_BASE,
];

#[derive(Debug, Copy, Clone, PartialEq, Eq, TrivialType)]
#[repr(C)]
pub struct Value {
    pub value: u64,
}

/// Contract whose methods write slots and call each other
#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct Rollback {
    pub value: u64,
}

#[contract]
impl Rollback {
    /// Writes a slot and calls [`Self::write()`] successfully, but leaves the state empty, which
    /// the host rejects after the method returned
    #[init]
    pub fn new_without_state(
        #[env] env: &mut Env<'_>,
        #[slot] slot: &mut MaybeData<Value>,
        #[input] value: &u64,
        #[input] nested_owner: &Address,
        #[output] _state: &mut MaybeData<Self>,
    ) -> Result<(), ContractError> {
        slot.replace(Value { value: *value });
        env.rollback_write(MethodContext::Keep, env.own_address(), nested_owner, value)
    }

    /// Writes a slot
    #[update]
    pub fn write(#[slot] slot: &mut MaybeData<Value>, #[input] &value: &u64) {
        slot.replace(Value { value });
    }

    /// Writes a slot, then fails
    #[update]
    pub fn write_and_fail(
        #[slot] slot: &mut MaybeData<Value>,
        #[input] &value: &u64,
    ) -> Result<(), ContractError> {
        slot.replace(Value { value });
        Err(ContractError::Forbidden)
    }

    /// Calls [`Self::write()`], then fails
    #[update]
    pub fn call_write_and_fail(
        #[env] env: &mut Env<'_>,
        #[input] value: &u64,
        #[input] nested_owner: &Address,
    ) -> Result<(), ContractError> {
        env.rollback_write(MethodContext::Keep, env.own_address(), nested_owner, value)?;
        Err(ContractError::Forbidden)
    }

    /// Writes a slot and calls [`Self::write()`]
    #[update]
    pub fn write_and_call_write(
        #[env] env: &mut Env<'_>,
        #[slot] slot: &mut MaybeData<Value>,
        #[input] value: &u64,
        #[input] nested_owner: &Address,
    ) -> Result<(), ContractError> {
        slot.replace(Value { value: *value });
        env.rollback_write(MethodContext::Keep, env.own_address(), nested_owner, value)
    }

    /// Writes a slot and calls [`Self::write_and_call_write()`], then fails
    #[update]
    pub fn write_call_nested_and_fail(
        #[env] env: &mut Env<'_>,
        #[slot] slot: &mut MaybeData<Value>,
        #[input] value: &u64,
        #[input] nested_owner: &Address,
        #[input] nested_nested_owner: &Address,
    ) -> Result<(), ContractError> {
        slot.replace(Value { value: *value });
        env.rollback_write_and_call_write(
            MethodContext::Keep,
            env.own_address(),
            nested_owner,
            value,
            nested_nested_owner,
        )?;
        Err(ContractError::Forbidden)
    }

    /// Writes `value` to a slot, calls [`Self::write()`] with `value + 1` and then
    /// [`Self::write_call_nested_and_fail()`] with `value + 2` for the same nested slot, whose
    /// error is ignored, and writes `value + 3` to the slot in the end
    #[update]
    pub fn survive_nested_failure(
        #[env] env: &mut Env<'_>,
        #[slot] slot: &mut MaybeData<Value>,
        #[input] &value: &u64,
        #[input] nested_owner: &Address,
        #[input] nested_nested_owner: &Address,
        #[input] nested_nested_nested_owner: &Address,
    ) -> Result<(), ContractError> {
        slot.replace(Value { value });
        env.rollback_write(
            MethodContext::Keep,
            env.own_address(),
            nested_owner,
            &(value + 1),
        )?;
        let result = env.rollback_write_call_nested_and_fail(
            MethodContext::Keep,
            env.own_address(),
            nested_owner,
            &(value + 2),
            nested_nested_owner,
            nested_nested_nested_owner,
        );
        if result != Err(ContractError::Forbidden) {
            return Err(ContractError::BadOutput);
        }
        slot.replace(Value { value: value + 3 });
        Ok(())
    }

    /// Deploys a contract, then fails
    #[update]
    pub fn deploy_and_fail(
        #[env] env: &mut Env<'_>,
        #[input] code: &VariableBytes<MAX_CODE_SIZE>,
    ) -> Result<(), ContractError> {
        env.code_deploy(MethodContext::Keep, Address::SYSTEM_CODE, code)?;
        Err(ContractError::Forbidden)
    }

    /// Value of a slot, `0` if it is empty
    #[view]
    pub fn read(#[slot] slot: &MaybeData<Value>) -> u64 {
        slot.get().map_or(0, |slot| slot.value)
    }
}

fn setup() -> (NativeExecutor, Slots, Address) {
    let executor = NativeExecutor::builder(ShardIndex::new(1).unwrap())
        .with_contract::<Rollback>()
        .build()
        .unwrap();
    let mut slots = executor.new_storage_slots().unwrap();

    let contract = executor.transaction_emulate(Address::NULL, &mut slots, |env| {
        let contract = env
            .code_deploy(MethodContext::Keep, Address::SYSTEM_CODE, &Rollback::code())
            .unwrap();
        // Initial values, so that rollback has something to restore
        for (owner, value) in OWNERS.iter().zip(1..) {
            env.rollback_write(MethodContext::Keep, contract, owner, &value)
                .unwrap();
        }
        contract
    });

    (executor, slots, contract)
}

fn read_all(executor: &NativeExecutor, slots: &Slots, contract: Address) -> [u64; 4] {
    executor.with_env_ro(slots, |env| {
        OWNERS.map(|owner| env.rollback_read(contract, &owner).unwrap())
    })
}

#[test]
fn failed_call_rolls_back_successful_nested_call() {
    let (executor, mut slots, contract) = setup();

    let (write_and_fail_result, call_write_and_fail_result) =
        executor.transaction_emulate(Address::NULL, &mut slots, |env| {
            let write_and_fail_result =
                env.rollback_write_and_fail(MethodContext::Keep, contract, &OWNERS[0], &10);
            let call_write_and_fail_result =
                env.rollback_call_write_and_fail(MethodContext::Keep, contract, &10, &OWNERS[1]);
            (write_and_fail_result, call_write_and_fail_result)
        });

    assert_eq!(write_and_fail_result, Err(ContractError::Forbidden));
    assert_eq!(call_write_and_fail_result, Err(ContractError::Forbidden));
    assert_eq!(read_all(&executor, &slots, contract), [1, 2, 3, 4]);
}

#[test]
fn failed_call_rolls_back_successful_nested_calls_chain() {
    let (executor, mut slots, contract) = setup();

    let result = executor.transaction_emulate(Address::NULL, &mut slots, |env| {
        // Written in the same transaction before the failed call, which must restore this value
        // rather than the one from before the transaction
        env.rollback_write(MethodContext::Keep, contract, &OWNERS[2], &20)
            .unwrap();

        env.rollback_write_call_nested_and_fail(
            MethodContext::Keep,
            contract,
            &OWNERS[0],
            &10,
            &OWNERS[1],
            &OWNERS[2],
        )
    });

    assert_eq!(result, Err(ContractError::Forbidden));
    assert_eq!(read_all(&executor, &slots, contract), [1, 2, 20, 4]);
}

#[test]
fn failed_nested_call_rolls_back_only_its_changes() {
    let (executor, mut slots, contract) = setup();

    // The caller has its slot open for writing during the nested calls
    let result = executor.transaction_emulate(Address::NULL, &mut slots, |env| {
        env.rollback_survive_nested_failure(
            MethodContext::Keep,
            contract,
            &OWNERS[0],
            &10,
            &OWNERS[1],
            &OWNERS[2],
            &OWNERS[3],
        )
    });

    assert_eq!(result, Ok(()));
    assert_eq!(read_all(&executor, &slots, contract), [13, 11, 3, 4]);
}

#[test]
fn host_error_after_successful_return_rolls_back_call() {
    let (executor, mut slots, contract) = setup();

    let result = executor.transaction_emulate(Address::NULL, &mut slots, |env| {
        env.rollback_new_without_state(MethodContext::Keep, contract, &OWNERS[0], &10, &OWNERS[1])
    });

    assert_eq!(result, Err(ContractError::BadOutput));
    assert_eq!(read_all(&executor, &slots, contract), [1, 2, 3, 4]);
}

#[test]
fn failed_call_rolls_back_address_allocation() {
    let (executor, mut slots, contract) = setup();
    let mut expected_slots = slots.clone();

    let deployed = executor.transaction_emulate(Address::NULL, &mut slots, |env| {
        let result = env.rollback_deploy_and_fail(MethodContext::Keep, contract, &Rollback::code());
        assert_eq!(result, Err(ContractError::Forbidden));

        env.code_deploy(MethodContext::Keep, Address::SYSTEM_CODE, &Rollback::code())
            .unwrap()
    });
    let expected_deployed =
        executor.transaction_emulate(Address::NULL, &mut expected_slots, |env| {
            env.code_deploy(MethodContext::Keep, Address::SYSTEM_CODE, &Rollback::code())
                .unwrap()
        });

    // The address allocated during the failed call is allocated again
    assert_eq!(deployed, expected_deployed);
}
