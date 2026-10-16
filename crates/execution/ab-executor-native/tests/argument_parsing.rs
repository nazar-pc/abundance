//! Methods parse contents of `#[input]` arguments and initial contents of `#[output]` arguments,
//! which callers provide, and return `ContractError::BadInput` for those that are not valid values
//! of their types, without changing the state of the contract.
//!
//! Calling a method with arguments that are not valid values of their types without these checks
//! is undefined behavior, which Miri detects, so these tests are also run under Miri.

use crate::ffi::input::ArgumentsInputArgs;
use crate::ffi::maybe_data_output::ArgumentsMaybeDataOutputArgs;
use crate::ffi::new::ArgumentsNewArgs;
use crate::ffi::plain_output::ArgumentsPlainOutputArgs;
use crate::ffi::set::ArgumentsSetArgs;
use crate::ffi::variable_elements_output::ArgumentsVariableElementsOutputArgs;
use ab_contracts_common::env::{Env, MethodContext, PreparedMethod};
use ab_contracts_common::method::ExternalArgs;
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
use core::ptr::NonNull;

/// One byte, of which only `0` and `1` are valid values
#[derive(Debug, Copy, Clone, PartialEq, Eq, TrivialType)]
#[repr(u8)]
pub enum Flag {
    Off,
    On,
}

impl Flag {
    fn flipped(self) -> Self {
        match self {
            Self::Off => Self::On,
            Self::On => Self::Off,
        }
    }
}

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct Arguments {
    pub flag: Flag,
}

#[contract]
impl Arguments {
    #[init]
    pub fn new(#[input] &flag: &Flag) -> Self {
        Self { flag }
    }

    #[update]
    pub fn set(&mut self, #[input] &flag: &Flag) {
        self.flag = flag;
    }

    #[view]
    pub fn get(&self) -> Flag {
        self.flag
    }

    #[view]
    pub fn input(#[input] &flag: &Flag) -> Result<(), ContractError> {
        match flag {
            Flag::Off => Err(ContractError::Forbidden),
            Flag::On => Ok(()),
        }
    }

    #[view]
    pub fn plain_output(#[output] flag: &mut Flag) {
        *flag = flag.flipped();
    }

    #[view]
    pub fn maybe_data_output(#[output] maybe_flag: &mut MaybeData<Flag>) {
        match maybe_flag.get_mut() {
            Some(flag) => {
                *flag = flag.flipped();
            }
            None => {
                maybe_flag.replace(Flag::On);
            }
        }
    }

    #[view]
    pub fn variable_elements_output(#[output] flags: &mut VariableElements<Flag>) {
        for flag in flags.get_initialized_mut() {
            *flag = flag.flipped();
        }
    }
}

/// Method of `Arguments` that has a single `#[input]` or `#[output]` and returns nothing to the
/// caller
#[derive(Debug, Copy, Clone)]
enum Method {
    New,
    Set,
    Input,
    PlainOutput,
    MaybeDataOutput,
    VariableElementsOutput,
}

/// Pointer, size and capacity of a single `#[input]` or `#[output]` argument, laid out like
/// `ExternalArgs` of a method that has only this argument and returns nothing to the caller
#[derive(Debug)]
#[repr(C)]
struct RawArgs {
    ptr: NonNull<u8>,
    size: u32,
    capacity: u32,
}

/// Call `method` of `Arguments` with `bytes` as its only argument, with `size` and `capacity` that
/// match the method's metadata
fn call_with_bytes(
    env: &Env<'_>,
    contract: Address,
    method: Method,
    bytes: &mut [u8],
    size: u32,
    capacity: u32,
) -> Result<(), ContractError> {
    assert!(size <= capacity);
    assert!(usize::try_from(capacity).unwrap() <= bytes.len());
    // The native executor trusts callers to give sizes and capacities that match the method's
    // metadata
    match method {
        Method::New | Method::Set | Method::Input | Method::PlainOutput => {
            assert_eq!(size, Flag::SIZE);
        }
        Method::MaybeDataOutput => {
            assert!(size == 0 || size == Flag::SIZE);
            assert!(capacity >= Flag::SIZE);
        }
        // Elements are one byte, so every size is a multiple of the element size
        Method::VariableElementsOutput => {}
    }

    let fingerprint = match method {
        Method::New => ArgumentsNewArgs::FINGERPRINT,
        Method::Set => ArgumentsSetArgs::FINGERPRINT,
        Method::Input => ArgumentsInputArgs::FINGERPRINT,
        Method::PlainOutput => ArgumentsPlainOutputArgs::FINGERPRINT,
        Method::MaybeDataOutput => ArgumentsMaybeDataOutputArgs::FINGERPRINT,
        Method::VariableElementsOutput => ArgumentsVariableElementsOutputArgs::FINGERPRINT,
    };
    let mut args = RawArgs {
        ptr: NonNull::from_mut(bytes).cast::<u8>(),
        size,
        capacity,
    };
    // SAFETY: `fingerprint` is that of a method in `Method`, each of which has a single `#[input]`
    // or `#[output]` of a type with alignment 1 and returns nothing to the caller (`#[init]`
    // returns the state to the host), so `RawArgs` is laid out like its `ExternalArgs` and the
    // pointer is aligned. The size and the capacity match the method's metadata, as asserted
    // above, which the native executor requires, since it trusts callers. `bytes` are initialized,
    // valid for reads and writes of `capacity` bytes, which is not smaller than `size`, as asserted
    // above, and not accessed otherwise during the call, while whether the contents are valid for
    // the type of the argument is not part of the contract.
    let prepared_method = unsafe {
        PreparedMethod::new_untyped(contract, fingerprint, &mut args, MethodContext::Reset)
    };
    env.call_prepared(prepared_method)
}

fn deploy() -> (NativeExecutor, Slots, Address) {
    let executor = NativeExecutor::builder(ShardIndex::new(1).unwrap())
        .with_contract::<Arguments>()
        .build()
        .unwrap();
    let mut slots = executor.new_storage_slots().unwrap();

    let contract = executor.transaction_emulate(Address::NULL, &mut slots, |env| {
        env.code_deploy(
            MethodContext::Keep,
            Address::SYSTEM_CODE,
            &Arguments::code(),
        )
        .unwrap()
    });

    (executor, slots, contract)
}

#[test]
fn input() {
    let (executor, mut slots, contract) = deploy();

    executor.transaction_emulate(Address::NULL, &mut slots, |env| {
        // Valid values
        assert_eq!(
            call_with_bytes(env, contract, Method::Input, &mut [1], 1, 1),
            Ok(())
        );
        assert_eq!(
            call_with_bytes(env, contract, Method::Input, &mut [0], 1, 1),
            Err(ContractError::Forbidden)
        );
        // Not a valid value
        assert_eq!(
            call_with_bytes(env, contract, Method::Input, &mut [2], 1, 1),
            Err(ContractError::BadInput)
        );
    });
}

#[test]
fn plain_output() {
    let (executor, mut slots, contract) = deploy();

    executor.transaction_emulate(Address::NULL, &mut slots, |env| {
        // Valid value
        let mut bytes = [1];
        assert_eq!(
            call_with_bytes(env, contract, Method::PlainOutput, &mut bytes, 1, 1),
            Ok(())
        );
        assert_eq!(bytes, [0]);
        // Not a valid value
        assert_eq!(
            call_with_bytes(env, contract, Method::PlainOutput, &mut [2], 1, 1),
            Err(ContractError::BadInput)
        );
    });
}

#[test]
fn maybe_data_output() {
    let (executor, mut slots, contract) = deploy();

    executor.transaction_emulate(Address::NULL, &mut slots, |env| {
        // Valid value
        let mut bytes = [1];
        assert_eq!(
            call_with_bytes(env, contract, Method::MaybeDataOutput, &mut bytes, 1, 1),
            Ok(())
        );
        assert_eq!(bytes, [0]);
        // No value, the byte beyond the size is not read
        let mut bytes = [2];
        assert_eq!(
            call_with_bytes(env, contract, Method::MaybeDataOutput, &mut bytes, 0, 1),
            Ok(())
        );
        assert_eq!(bytes, [1]);
        // Not a valid value
        assert_eq!(
            call_with_bytes(env, contract, Method::MaybeDataOutput, &mut [2], 1, 1),
            Err(ContractError::BadInput)
        );
    });
}

#[test]
fn variable_elements_output() {
    let (executor, mut slots, contract) = deploy();

    executor.transaction_emulate(Address::NULL, &mut slots, |env| {
        // Valid elements, the byte beyond the size is not read
        let mut bytes = [1, 0, 2];
        assert_eq!(
            call_with_bytes(
                env,
                contract,
                Method::VariableElementsOutput,
                &mut bytes,
                2,
                3
            ),
            Ok(())
        );
        assert_eq!(bytes, [0, 1, 2]);
        // An element is not a valid value
        assert_eq!(
            call_with_bytes(
                env,
                contract,
                Method::VariableElementsOutput,
                &mut [1, 0, 2],
                3,
                3
            ),
            Err(ContractError::BadInput)
        );
    });
}

#[test]
fn init_input() {
    let (executor, mut slots, contract) = deploy();

    executor.transaction_emulate(Address::NULL, &mut slots, |env| {
        // Not a valid value, the contract stays uninitialized
        assert_eq!(
            call_with_bytes(env, contract, Method::New, &mut [2], 1, 1),
            Err(ContractError::BadInput)
        );
        assert_eq!(env.arguments_get(contract), Err(ContractError::Forbidden));
        // Valid value
        assert_eq!(
            call_with_bytes(env, contract, Method::New, &mut [1], 1, 1),
            Ok(())
        );
        assert_eq!(env.arguments_get(contract), Ok(Flag::On));
    });
}

#[test]
fn update_input() {
    let (executor, mut slots, contract) = deploy();

    executor.transaction_emulate(Address::NULL, &mut slots, |env| {
        env.arguments_new(MethodContext::Reset, contract, &Flag::On)
            .unwrap();
        // Not a valid value, the state stays unchanged
        assert_eq!(
            call_with_bytes(env, contract, Method::Set, &mut [2], 1, 1),
            Err(ContractError::BadInput)
        );
        assert_eq!(env.arguments_get(contract), Ok(Flag::On));
        // Valid value
        assert_eq!(
            call_with_bytes(env, contract, Method::Set, &mut [0], 1, 1),
            Ok(())
        );
        assert_eq!(env.arguments_get(contract), Ok(Flag::Off));
    });
}
