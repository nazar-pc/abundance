//! Outputs are given to calls through `OutputArg`: memory whose contents accept every bit pattern
//! directly and other memory in `Lent`, which parses the contents before they are accessed.
//! Methods that declare their `#[output]`s with the lifetime of `Env` can lend them to calls (see
//! `Env::lend_output()`), and return values are parsed by extension methods.
//!
//! Accessing contents that are not valid values of their types is undefined behavior, which Miri
//! detects, so these tests are also run under Miri.

use crate::ffi::get_flag::OutputsGetFlagArgs;
use crate::ffi::plain_flag_output::OutputsPlainFlagOutputArgs;
use ab_contracts_common::env::{Env, EnvState, ExecutorContext, MethodContext, PreparedMethod};
use ab_contracts_common::method::ExternalArgs;
use ab_contracts_common::output::Lent;
use ab_contracts_common::{Contract, ContractError};
use ab_contracts_macros::contract;
use ab_core_primitives::address::Address;
use ab_core_primitives::shard::ShardIndex;
use ab_executor_native::NativeExecutor;
use ab_executor_slots::Slots;
use ab_io_type::maybe_data::MaybeData;
use ab_io_type::trivial_type::TrivialType;
use ab_io_type::variable_elements::VariableElements;
use ab_system_contract_code::CodeExt;
use core::mem::MaybeUninit;
use core::ptr::NonNull;

/// One byte, of which only `0` and `1` are valid values
#[derive(Debug, Copy, Clone, PartialEq, Eq, TrivialType)]
#[repr(u8)]
pub enum Flag {
    Off,
    On,
}

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct Outputs {
    pub flag: Flag,
}

#[contract]
impl Outputs {
    #[init]
    pub fn init() -> Self {
        Self { flag: Flag::On }
    }

    #[view]
    pub fn get_flag(&self) -> Flag {
        self.flag
    }

    #[view]
    pub fn plain_flag_output(#[output] flag: &mut Flag) {
        *flag = Flag::On;
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

    /// Lends its output to `set_plain()`
    #[view]
    pub fn lend_plain<'a>(
        #[env] env: &Env<'a>,
        #[input] value: &u64,
        #[output] out: &'a mut u64,
    ) -> Result<(), ContractError> {
        let mut out = env.lend_output(out);
        env.outputs_set_plain(env.own_address(), value, &mut out)
    }

    #[view]
    pub fn set_flag(#[input] &flag: &Flag, #[output] out: &mut MaybeData<'_, Flag>) {
        out.replace(flag);
    }

    /// Lends its output to `set_flag()` without accessing it
    #[update]
    pub fn forward_flag<'a>(
        #[env] env: &mut Env<'a>,
        #[input] flag: &Flag,
        #[output] out: &'a mut MaybeData<'_, Flag>,
    ) -> Result<(), ContractError> {
        let mut out = env.lend_output(out);
        env.outputs_set_flag(env.own_address(), flag, &mut out)
    }

    #[view]
    pub fn set_flags(
        #[input] &flag: &Flag,
        #[output] flags: &mut VariableElements<'_, Flag>,
    ) -> Result<(), ContractError> {
        if flags.append(&[flag, flag]) {
            Ok(())
        } else {
            Err(ContractError::BadInput)
        }
    }

    #[view]
    pub fn set_three(
        #[input] &flag: &Flag,
        #[output] maybe_flag: &mut MaybeData<'_, Flag>,
        #[output] plain_flag: &mut Flag,
        #[output] value: &mut u64,
    ) {
        maybe_flag.replace(flag);
        *plain_flag = flag;
        *value = 3;
    }

    /// Gives `set_three()` its own output lent with `Env::lend_output()`, a `Lent::new()` and a
    /// local whose contents accept every bit pattern in one call
    #[update]
    pub fn mixed<'a>(
        #[env] env: &mut Env<'a>,
        #[input] flag: &Flag,
        #[output] maybe_flag: &'a mut MaybeData<'_, Flag>,
        #[output] value: &mut u64,
    ) -> Result<Flag, ContractError> {
        let mut maybe_flag = env.lend_output(maybe_flag);
        let mut plain_flag = Lent::new(Flag::Off);
        let mut local_value = 0;
        env.outputs_set_three(
            env.own_address(),
            flag,
            &mut maybe_flag,
            &mut plain_flag,
            &mut local_value,
        )?;
        *value = local_value;
        plain_flag.get_mut().copied()
    }
}

/// Executor context that leaves contents behind that are not valid values: it writes `2` as the
/// return value of `get_flag()` and `7` into the output of `plain_flag_output()`, which fails
#[derive(Debug)]
struct InvalidContents;

// SAFETY: Only writes initialized bytes: one byte as the return value of `get_flag()` before
// returning `Ok(())` and one byte into the output of `plain_flag_output()`, whose capacity is one
// byte, before returning an error, without changing its size. Doesn't access anything behind
// pointers in prepared methods after returning.
unsafe impl ExecutorContext for InvalidContents {
    fn call(
        &self,
        _previous_env_state: &EnvState,
        prepared_method: &PreparedMethod<'_>,
    ) -> Result<(), ContractError> {
        let external_args = prepared_method.external_args;
        if prepared_method.fingerprint == OutputsGetFlagArgs::FINGERPRINT {
            // SAFETY: `ExternalArgs` of `get_flag()` only has the pointer to the return value, a
            // `Flag` of one byte, which is valid for writes during the call
            unsafe {
                external_args.cast::<NonNull<u8>>().read().write(2);
            }
            Ok(())
        } else if prepared_method.fingerprint == OutputsPlainFlagOutputArgs::FINGERPRINT {
            // SAFETY: `ExternalArgs` of `plain_flag_output()` starts with the pointer to the
            // output, a `Flag` of one byte, which is valid for writes during the call
            unsafe {
                external_args.cast::<NonNull<u8>>().read().write(7);
            }
            Err(ContractError::Forbidden)
        } else {
            Err(ContractError::NotImplemented)
        }
    }
}

fn invalid_contents_env_state() -> EnvState {
    EnvState {
        shard_index: ShardIndex::new(1).unwrap(),
        padding_0: [0; _],
        own_address: Address::NULL,
        context: Address::NULL,
        caller: Address::NULL,
    }
}

fn deploy() -> (NativeExecutor, Slots, Address) {
    let executor = NativeExecutor::builder(ShardIndex::new(1).unwrap())
        .with_contract::<Outputs>()
        .build()
        .unwrap();
    let mut slots = executor.new_storage_slots().unwrap();

    let contract = executor.transaction_emulate(Address::NULL, &mut slots, |env| {
        let contract = env
            .code_deploy(MethodContext::Keep, Address::SYSTEM_CODE, &Outputs::code())
            .unwrap();
        env.outputs_init(MethodContext::Keep, contract).unwrap();
        contract
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

#[test]
fn forward_output_that_needs_parsing() {
    let (executor, mut slots, contract) = deploy();

    executor.transaction_emulate(Address::NULL, &mut slots, |env| {
        let mut flag = MaybeUninit::uninit();
        let mut size = 0;
        {
            let mut lent = Lent::<MaybeData<'_, Flag>>::from_uninit(&mut flag, &mut size).unwrap();
            env.outputs_forward_flag(MethodContext::Keep, contract, &Flag::On, &mut lent)
                .unwrap();
            assert_eq!(lent.get_mut().unwrap().get(), Some(&Flag::On));
        }
        // The size set by `set_flag()` reached the caller through the forwarding method
        assert_eq!(size, 1);
    });
}

#[test]
fn mixed_outputs() {
    let (executor, mut slots, contract) = deploy();

    executor.transaction_emulate(Address::NULL, &mut slots, |env| {
        let mut maybe_flag = MaybeUninit::uninit();
        let mut maybe_flag_size = 0;
        let mut value = 0;
        let flag = {
            let mut lent =
                Lent::<MaybeData<'_, Flag>>::from_uninit(&mut maybe_flag, &mut maybe_flag_size)
                    .unwrap();
            let flag = env
                .outputs_mixed(
                    MethodContext::Keep,
                    contract,
                    &Flag::On,
                    &mut lent,
                    &mut value,
                )
                .unwrap();
            assert_eq!(lent.get_mut().unwrap().get(), Some(&Flag::On));
            flag
        };
        assert_eq!(flag, Flag::On);
        assert_eq!(maybe_flag_size, 1);
        assert_eq!(value, 3);
    });
}

#[test]
fn lent_from_uninit() {
    let (executor, slots, contract) = deploy();

    let (flag, flag_size, flags, flags_size) = executor.with_env_ro(&slots, |env| {
        let mut flag = MaybeUninit::uninit();
        let mut flag_size = 0;
        let flag = {
            let mut lent =
                Lent::<MaybeData<'_, Flag>>::from_uninit(&mut flag, &mut flag_size).unwrap();
            env.outputs_set_flag(contract, &Flag::Off, &mut lent)
                .unwrap();
            lent.get_mut().unwrap().get().copied()
        };

        let mut flags = [MaybeUninit::uninit(); 3];
        let mut flags_size = 0;
        let flags = {
            let mut lent =
                Lent::<VariableElements<'_, Flag>>::from_uninit(&mut flags, &mut flags_size)
                    .unwrap();
            env.outputs_set_flags(contract, &Flag::On, &mut lent)
                .unwrap();
            lent.get_mut().unwrap().get_initialized().to_vec()
        };

        (flag, flag_size, flags, flags_size)
    });
    assert_eq!(flag, Some(Flag::Off));
    assert_eq!(flag_size, 1);
    assert_eq!(flags, [Flag::On, Flag::On]);
    assert_eq!(flags_size, 2);
}

#[test]
fn return_value_is_parsed() {
    let (executor, slots, contract) = deploy();

    // A valid return value through the native executor
    let flag = executor.with_env_ro(&slots, |env| env.outputs_get_flag(contract));
    assert_eq!(flag, Ok(Flag::On));

    // SAFETY: Created outside of any method and only given to an extension method, which borrows
    // no memory for the lifetime of `Env`, and only dropped afterward
    let env = unsafe { Env::with_executor_context(invalid_contents_env_state(), &InvalidContents) };
    assert_eq!(
        env.outputs_get_flag(Address::NULL),
        Err(ContractError::BadOutput)
    );
}

#[test]
fn failed_call_leaves_invalid_output() {
    // SAFETY: Created outside of any method and only given to an extension method, which borrows
    // no memory for the lifetime of `Env`, and only dropped afterward
    let env = unsafe { Env::with_executor_context(invalid_contents_env_state(), &InvalidContents) };

    let mut flag = Lent::new(Flag::Off);
    assert_eq!(
        env.outputs_plain_flag_output(Address::NULL, &mut flag),
        Err(ContractError::Forbidden)
    );
    assert_eq!(flag.get_mut(), Err(ContractError::BadOutput));
    flag.set(Flag::On);
    assert_eq!(flag.get_mut(), Ok(&mut Flag::On));
}
