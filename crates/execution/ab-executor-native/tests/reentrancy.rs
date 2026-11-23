//! A method keeps the state of its contract borrowed until it returns, like a Rust reference.
//! Calls made in the meantime, including those that come back through other contracts, can share
//! `&self` with it, but fail with `ContractError::Forbidden` when they need the state while the
//! method holds `&mut self`, or need `&mut self` while it holds `&self`.

use ab_contracts_common::env::{Env, MethodContext};
use ab_contracts_common::{Contract, ContractError};
use ab_contracts_macros::contract;
use ab_core_primitives::address::Address;
use ab_core_primitives::shard::ShardIndex;
use ab_executor_native::NativeExecutor;
use ab_executor_slots::Slots;
use ab_io_type::trivial_type::TrivialType;
use ab_system_contract_code::CodeExt;

/// Contract that calls back into itself through a second instance of this contract
#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct Reentrancy {
    pub value: u64,
}

#[contract]
impl Reentrancy {
    #[init]
    pub fn new(#[input] &value: &u64) -> Self {
        Self { value }
    }

    #[view]
    pub fn get(&self) -> u64 {
        self.value
    }

    #[update]
    pub fn set(&mut self, #[input] &value: &u64) {
        self.value = value;
    }

    /// Calls [`Self::get()`] of `contract`
    #[view]
    pub fn get_of(#[env] env: &Env<'_>, #[input] contract: &Address) -> Result<u64, ContractError> {
        env.reentrancy_get(*contract)
    }

    /// Calls [`Self::set()`] of `contract`
    #[update]
    pub fn set_of(
        #[env] env: &mut Env<'_>,
        #[input] contract: &Address,
        #[input] value: &u64,
    ) -> Result<(), ContractError> {
        env.reentrancy_set(MethodContext::Keep, *contract, value)
    }

    /// Calls [`Self::get_of()`] of `via` while holding the state with `&mut self`
    #[update]
    pub fn get_of_via_while_writing(
        &mut self,
        #[env] env: &Env<'_>,
        #[input] via: &Address,
        #[input] contract: &Address,
    ) -> Result<u64, ContractError> {
        env.reentrancy_get_of(*via, contract)
    }

    /// Calls [`Self::get_of()`] of `via` while holding the state with `&self`
    #[update]
    pub fn get_of_via_while_reading(
        &self,
        #[env] env: &Env<'_>,
        #[input] via: &Address,
        #[input] contract: &Address,
    ) -> Result<u64, ContractError> {
        env.reentrancy_get_of(*via, contract)
    }

    /// Calls [`Self::set_of()`] of `via` while holding the state with `&self`
    #[update]
    pub fn set_of_via_while_reading(
        &self,
        #[env] env: &mut Env<'_>,
        #[input] via: &Address,
        #[input] contract: &Address,
        #[input] value: &u64,
    ) -> Result<(), ContractError> {
        env.reentrancy_set_of(MethodContext::Keep, *via, contract, value)
    }
}

/// Two instances of [`Reentrancy`] with values `1` and `2`
fn setup() -> (NativeExecutor, Slots, [Address; 2]) {
    let executor = NativeExecutor::builder(ShardIndex::new(1).unwrap())
        .with_contract::<Reentrancy>()
        .build()
        .unwrap();
    let mut slots = executor.new_storage_slots().unwrap();

    let contracts = executor.transaction_emulate(Address::NULL, &mut slots, |env| {
        [1, 2].map(|value| {
            let contract = env
                .code_deploy(
                    MethodContext::Keep,
                    Address::SYSTEM_CODE,
                    &Reentrancy::code(),
                )
                .unwrap();
            env.reentrancy_new(MethodContext::Keep, contract, &value)
                .unwrap();
            contract
        })
    });

    (executor, slots, contracts)
}

#[test]
fn reentrant_call_cannot_read_state_borrowed_for_writing() {
    let (executor, mut slots, [first, second]) = setup();

    executor.transaction_emulate(Address::NULL, &mut slots, |env| {
        // The other contract can read its own state
        assert_eq!(
            env.reentrancy_get_of_via_while_writing(MethodContext::Keep, first, &second, &second),
            Ok(2)
        );
        // But not the state of the caller
        assert_eq!(
            env.reentrancy_get_of_via_while_writing(MethodContext::Keep, first, &second, &first),
            Err(ContractError::Forbidden)
        );
    });
}

#[test]
fn reentrant_call_can_read_state_borrowed_for_reading() {
    let (executor, mut slots, [first, second]) = setup();

    executor.transaction_emulate(Address::NULL, &mut slots, |env| {
        assert_eq!(
            env.reentrancy_get_of_via_while_reading(MethodContext::Keep, first, &second, &first),
            Ok(1)
        );
    });
}

#[test]
fn reentrant_call_cannot_write_state_borrowed_for_reading() {
    let (executor, mut slots, [first, second]) = setup();

    executor.transaction_emulate(Address::NULL, &mut slots, |env| {
        // The other contract can write its own state
        assert_eq!(
            env.reentrancy_set_of_via_while_reading(
                MethodContext::Keep,
                first,
                &second,
                &second,
                &20
            ),
            Ok(())
        );
        // But not the state of the caller
        assert_eq!(
            env.reentrancy_set_of_via_while_reading(
                MethodContext::Keep,
                first,
                &second,
                &first,
                &10
            ),
            Err(ContractError::Forbidden)
        );
    });
}
