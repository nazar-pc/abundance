//! Methods that declare their `#[output]`s with the lifetime of `Env` can lend them to calls (see
//! `Env::lend_output()`).
//!
//! These tests are also run under Miri.

use ab_contracts_common::env::{Env, MethodContext};
use ab_contracts_common::{Contract, ContractError};
use ab_contracts_macros::contract;
use ab_core_primitives::address::Address;
use ab_core_primitives::shard::ShardIndex;
use ab_executor_native::NativeExecutor;
use ab_executor_slots::Slots;
use ab_io_type::trivial_type::TrivialType;
use ab_system_contract_code::CodeExt;

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct Outputs {
    pub value: u8,
}

#[contract]
impl Outputs {
    #[init]
    pub fn init() -> Self {
        Self { value: 0 }
    }

    #[view]
    pub fn set_plain(#[input] &value: &u64, #[output] out: &mut u64) {
        *out = value;
    }

    /// Gives its output to `set_plain()`
    #[update]
    pub fn forward_plain<'a>(
        #[env] env: &mut Env<'a>,
        #[input] value: &u64,
        #[output] out: &'a mut u64,
    ) -> Result<(), ContractError> {
        env.outputs_set_plain(env.own_address(), value, out)
    }

    /// Sets its output through `Env::lend_output()`
    #[view]
    pub fn lend_plain<'a>(
        #[env] env: &Env<'a>,
        #[input] &value: &u64,
        #[output] out: &'a mut u64,
    ) -> Result<(), ContractError> {
        let mut out = env.lend_output(out);
        out.set(value);
        Ok(())
    }
}

fn deploy() -> (NativeExecutor, Slots, Address) {
    let executor = NativeExecutor::builder(ShardIndex::new(1).unwrap())
        .with_contract::<Outputs>()
        .build()
        .unwrap();
    let mut slots = executor.new_storage_slots().unwrap();

    let contract = executor.transaction_emulate(Address::NULL, &mut slots, |env| {
        env.code_deploy(MethodContext::Keep, Address::SYSTEM_CODE, &Outputs::code())
            .unwrap()
    });

    (executor, slots, contract)
}

#[test]
fn output_with_env_lifetime() {
    let (executor, mut slots, contract) = deploy();

    executor.transaction_emulate(Address::NULL, &mut slots, |env| {
        let mut out = 0;
        env.outputs_forward_plain(MethodContext::Keep, contract, &1, &mut out)
            .unwrap();
        assert_eq!(out, 1);
    });

    let out = executor.with_env_ro(&slots, |env| {
        let mut out = 0;
        env.outputs_lend_plain(contract, &2, &mut out).unwrap();
        out
    });
    assert_eq!(out, 2);
}
