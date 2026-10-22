//! Simple wallet transactions give return values their size, so later methods can refer to them
//! as inputs and slots, see `TransactionPayloadBuilder::with_method_call()`

// Auto-generated constants will conflict with the main crate when `guest` feature is enabled
#![cfg(not(feature = "guest"))]

use crate::ffi::address::ReturnValuesAddressArgs;
use crate::ffi::expect_empty::ReturnValuesExpectEmptyArgs;
use crate::ffi::fail::ReturnValuesFailArgs;
use crate::ffi::increment::ReturnValuesIncrementArgs;
use crate::ffi::new::ReturnValuesNewArgs;
use crate::ffi::store::ReturnValuesStoreArgs;
use ab_contracts_common::env::{Env, MethodContext};
use ab_contracts_common::{Contract, ContractError};
use ab_contracts_macros::contract;
use ab_contracts_standards::tx_handler::TxHandler;
use ab_core_primitives::address::Address;
use ab_core_primitives::block::BlockRoot;
use ab_core_primitives::shard::ShardIndex;
use ab_core_primitives::transaction::{Gas, Transaction, TransactionHeader, TransactionSlot};
use ab_example_contract_wallet::{ExampleWallet, ExampleWalletExt};
use ab_executor_native::NativeExecutor;
use ab_io_type::maybe_data::MaybeData;
use ab_io_type::trivial_type::TrivialType;
use ab_system_contract_code::CodeExt;
use ab_system_contract_simple_wallet_base::payload::TransactionMethodContext;
use ab_system_contract_simple_wallet_base::payload::builder::{
    MethodCallReferences, TransactionPayloadBuilder,
};
use ab_system_contract_simple_wallet_base::payload::caller::{
    TransactionPayloadCallError, TransactionPayloadCaller,
};
use ab_system_contract_simple_wallet_base::seal::hash_and_sign;
use ab_system_contract_simple_wallet_base::{
    EXTERNAL_ARGS_BUFFER_SIZE, MAX_OUTPUTS, OUTPUT_BUFFER_SIZE,
};
use core::mem::MaybeUninit;
use core::ptr;
use schnorrkel::Keypair;

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct ReturnValues {
    pub value: u8,
}

#[contract]
impl ReturnValues {
    /// Initializes the state with `value`
    #[init]
    pub fn new(#[input] &value: &u8) -> Self {
        Self { value }
    }

    /// Returns the value in the state
    #[view]
    pub fn value(&self) -> u8 {
        self.value
    }

    /// Returns `value + 1`
    #[view]
    pub fn increment(#[input] &value: &u8) -> u8 {
        value.wrapping_add(1)
    }

    /// Returns `address`
    #[view]
    pub fn address(#[input] address: &Address) -> Address {
        *address
    }

    /// Stores `value` in `slot`
    #[update]
    pub fn store(#[slot] slot: &mut MaybeData<u8>, #[input] &value: &u8) {
        slot.replace(value);
    }

    /// Returns the value stored in `slot`
    #[view]
    pub fn stored(#[slot] slot: &MaybeData<u8>) -> Result<u8, ContractError> {
        slot.get().copied().ok_or(ContractError::NotFound)
    }

    /// Fails unless `value` is given empty
    #[view]
    pub fn expect_empty(#[input] value: &MaybeData<ReturnValues>) -> Result<(), ContractError> {
        if value.get().is_some() {
            return Err(ContractError::Forbidden);
        }

        Ok(())
    }

    /// Fails without writing its return value
    #[view]
    pub fn fail() -> Result<u8, ContractError> {
        Err(ContractError::Forbidden)
    }
}

/// Executor with the wallet and `ReturnValues`
fn new_executor() -> NativeExecutor {
    NativeExecutor::builder(ShardIndex::new(1).unwrap())
        .with_contract::<ExampleWallet>()
        .with_contract_trait::<ExampleWallet, dyn TxHandler>()
        .with_contract::<ReturnValues>()
        .build()
        .unwrap()
}

/// Deploy an initialized wallet and `ReturnValues` without state, returns their addresses
fn deploy(env: &mut Env<'_>, keypair: &Keypair) -> (Address, Address) {
    let wallet_address = env
        .code_deploy(
            MethodContext::Keep,
            Address::SYSTEM_CODE,
            &ExampleWallet::code(),
        )
        .unwrap();
    env.example_wallet_initialize(
        MethodContext::Keep,
        wallet_address,
        &keypair.public.to_bytes(),
    )
    .unwrap();
    let return_values_address = env
        .code_deploy(
            MethodContext::Keep,
            Address::SYSTEM_CODE,
            &ReturnValues::code(),
        )
        .unwrap();

    (wallet_address, return_values_address)
}

#[test]
fn return_value_as_slot() {
    let executor = new_executor();
    let slots = &mut executor.new_storage_slots().unwrap();
    let keypair = Keypair::generate();
    let (wallet_address, return_values_address) =
        executor.transaction_emulate(Address::NULL, slots, |env| deploy(env, &keypair));
    let header = TransactionHeader {
        version: 0,
        block_root: BlockRoot::default(),
        gas_limit: Gas::default(),
        contract: wallet_address,
    };
    // The returned address is the slot the second method stores a value in
    let payload = {
        // Output index of the only declared output
        let address = 0;
        let mut builder = TransactionPayloadBuilder::new(1024, 1).unwrap();
        builder
            .with_method_call(
                &return_values_address,
                &ReturnValuesAddressArgs::new(&wallet_address, &mut MaybeUninit::uninit()),
                TransactionMethodContext::Null,
                MethodCallReferences {
                    outputs: &[address],
                    ..MethodCallReferences::default()
                },
            )
            .unwrap();
        builder
            .with_method_call(
                &return_values_address,
                &ReturnValuesStoreArgs::new(&Address::NULL, &7),
                TransactionMethodContext::Null,
                MethodCallReferences {
                    slots: &[Some(address)],
                    ..MethodCallReferences::default()
                },
            )
            .unwrap();
        builder.into_aligned_bytes().unwrap()
    };
    let read_slots = &[];
    let write_slots = &[TransactionSlot {
        owner: wallet_address,
        contract: return_values_address,
    }];
    let seal = hash_and_sign(
        &keypair,
        &header,
        read_slots,
        write_slots,
        payload.as_u128_slice(),
        0,
    );

    assert_eq!(
        executor.transaction_execute(
            Transaction {
                header: &header,
                payload: payload.as_u128_slice(),
                read_slots,
                write_slots,
                seal: seal.as_bytes(),
            },
            slots,
        ),
        Ok(())
    );
    assert_eq!(
        executor.with_env_ro(slots, |env| {
            env.return_values_stored(return_values_address, &wallet_address)
        }),
        Ok(7)
    );
}

#[test]
fn return_value_as_plain_input() {
    let executor = new_executor();
    let slots = &mut executor.new_storage_slots().unwrap();
    let keypair = Keypair::generate();
    let (wallet_address, return_values_address) =
        executor.transaction_emulate(Address::NULL, slots, |env| deploy(env, &keypair));
    let header = TransactionHeader {
        version: 0,
        block_root: BlockRoot::default(),
        gas_limit: Gas::default(),
        contract: wallet_address,
    };
    // The incremented value is the plain input of the third method, whose slot is the returned
    // address, so that if return values were given empty, the caller would reject the slot
    // before the method could be called with an empty plain input, violating its requirements
    let payload = {
        // Output indices of the declared outputs
        let address = 0;
        let incremented = 1;
        let mut builder = TransactionPayloadBuilder::new(1024, 2).unwrap();
        builder
            .with_method_call(
                &return_values_address,
                &ReturnValuesAddressArgs::new(&wallet_address, &mut MaybeUninit::uninit()),
                TransactionMethodContext::Null,
                MethodCallReferences {
                    outputs: &[address],
                    ..MethodCallReferences::default()
                },
            )
            .unwrap();
        builder
            .with_method_call(
                &return_values_address,
                &ReturnValuesIncrementArgs::new(&42, &mut MaybeUninit::uninit()),
                TransactionMethodContext::Null,
                MethodCallReferences {
                    outputs: &[incremented],
                    ..MethodCallReferences::default()
                },
            )
            .unwrap();
        builder
            .with_method_call(
                &return_values_address,
                &ReturnValuesStoreArgs::new(&Address::NULL, &0),
                TransactionMethodContext::Null,
                MethodCallReferences {
                    slots: &[Some(address)],
                    inputs: &[Some(incremented)],
                    outputs: &[],
                },
            )
            .unwrap();
        builder.into_aligned_bytes().unwrap()
    };
    let read_slots = &[];
    let write_slots = &[TransactionSlot {
        owner: wallet_address,
        contract: return_values_address,
    }];
    let seal = hash_and_sign(
        &keypair,
        &header,
        read_slots,
        write_slots,
        payload.as_u128_slice(),
        0,
    );

    assert_eq!(
        executor.transaction_execute(
            Transaction {
                header: &header,
                payload: payload.as_u128_slice(),
                read_slots,
                write_slots,
                seal: seal.as_bytes(),
            },
            slots,
        ),
        Ok(())
    );
    assert_eq!(
        executor.with_env_ro(slots, |env| {
            env.return_values_stored(return_values_address, &wallet_address)
        }),
        Ok(43)
    );
}

#[test]
fn return_value_as_input_of_method_with_return_value() {
    let executor = new_executor();
    let slots = &mut executor.new_storage_slots().unwrap();
    let keypair = Keypair::generate();
    let (wallet_address, return_values_address) =
        executor.transaction_emulate(Address::NULL, slots, |env| deploy(env, &keypair));
    let header = TransactionHeader {
        version: 0,
        block_root: BlockRoot::default(),
        gas_limit: Gas::default(),
        contract: wallet_address,
    };
    // The second method increments the return value of the first one, so its input points into
    // the output buffer, and its own return value is written into another declared output there.
    // Creating the pointer for it must not invalidate the pointer of the input, which Miri checks.
    let payload = {
        // Output indices of the declared outputs
        let first = 0;
        let second = 1;
        let mut builder = TransactionPayloadBuilder::new(1024, 2).unwrap();
        builder
            .with_method_call(
                &return_values_address,
                &ReturnValuesIncrementArgs::new(&41, &mut MaybeUninit::uninit()),
                TransactionMethodContext::Null,
                MethodCallReferences {
                    outputs: &[first],
                    ..MethodCallReferences::default()
                },
            )
            .unwrap();
        builder
            .with_method_call(
                &return_values_address,
                &ReturnValuesIncrementArgs::new(&0, &mut MaybeUninit::uninit()),
                TransactionMethodContext::Null,
                MethodCallReferences {
                    slots: &[],
                    inputs: &[Some(first)],
                    outputs: &[second],
                },
            )
            .unwrap();
        builder
            .with_method_call(
                &return_values_address,
                &ReturnValuesStoreArgs::new(&wallet_address, &0),
                TransactionMethodContext::Null,
                MethodCallReferences {
                    inputs: &[Some(second)],
                    ..MethodCallReferences::default()
                },
            )
            .unwrap();
        builder.into_aligned_bytes().unwrap()
    };
    let read_slots = &[];
    let write_slots = &[TransactionSlot {
        owner: wallet_address,
        contract: return_values_address,
    }];
    let seal = hash_and_sign(
        &keypair,
        &header,
        read_slots,
        write_slots,
        payload.as_u128_slice(),
        0,
    );

    assert_eq!(
        executor.transaction_execute(
            Transaction {
                header: &header,
                payload: payload.as_u128_slice(),
                read_slots,
                write_slots,
                seal: seal.as_bytes(),
            },
            slots,
        ),
        Ok(())
    );
    assert_eq!(
        executor.with_env_ro(slots, |env| {
            env.return_values_stored(return_values_address, &wallet_address)
        }),
        Ok(43)
    );
}

#[test]
fn init_state_as_input() {
    let executor = new_executor();
    let slots = &mut executor.new_storage_slots().unwrap();
    let keypair = Keypair::generate();
    let (wallet_address, return_values_address) =
        executor.transaction_emulate(Address::NULL, slots, |env| deploy(env, &keypair));
    let header = TransactionHeader {
        version: 0,
        block_root: BlockRoot::default(),
        gas_limit: Gas::default(),
        contract: wallet_address,
    };
    // The state returned by `#[init]` is stored as the state of the contract instead of being
    // written to the caller, and the metadata leaves out its type, so the builder declares an
    // output without capacity for it and the second method referring to it gets it empty
    let payload = {
        // Output index of the only declared output
        let state = 0;
        let mut builder = TransactionPayloadBuilder::new(1024, 1).unwrap();
        builder
            .with_method_call(
                &return_values_address,
                &ReturnValuesNewArgs::new(&5),
                TransactionMethodContext::Null,
                MethodCallReferences {
                    outputs: &[state],
                    ..MethodCallReferences::default()
                },
            )
            .unwrap();
        builder
            .with_method_call(
                &return_values_address,
                &ReturnValuesExpectEmptyArgs::new(&MaybeData::from_ref(None)),
                TransactionMethodContext::Null,
                MethodCallReferences {
                    inputs: &[Some(state)],
                    ..MethodCallReferences::default()
                },
            )
            .unwrap();
        builder.into_aligned_bytes().unwrap()
    };
    let read_slots = &[];
    let write_slots = &[TransactionSlot {
        owner: return_values_address,
        contract: Address::SYSTEM_STATE,
    }];
    let seal = hash_and_sign(
        &keypair,
        &header,
        read_slots,
        write_slots,
        payload.as_u128_slice(),
        0,
    );

    assert_eq!(
        executor.transaction_execute(
            Transaction {
                header: &header,
                payload: payload.as_u128_slice(),
                read_slots,
                write_slots,
                seal: seal.as_bytes(),
            },
            slots,
        ),
        Ok(())
    );
    assert_eq!(
        executor.with_env_ro(slots, |env| env.return_values_value(return_values_address)),
        Ok(5)
    );
}

#[test]
fn failed_method_stops_transaction() {
    let executor = new_executor();
    let slots = &mut executor.new_storage_slots().unwrap();
    let keypair = Keypair::generate();
    let (wallet_address, return_values_address) =
        executor.transaction_emulate(Address::NULL, slots, |env| deploy(env, &keypair));
    let header = TransactionHeader {
        version: 0,
        block_root: BlockRoot::default(),
        gas_limit: Gas::default(),
        contract: wallet_address,
    };
    // The second method stores the return value of the first one, which fails without writing it
    let payload = {
        // Output index of the only declared output
        let failed = 0;
        let mut builder = TransactionPayloadBuilder::new(1024, 1).unwrap();
        builder
            .with_method_call(
                &return_values_address,
                &ReturnValuesFailArgs::new(&mut MaybeUninit::uninit()),
                TransactionMethodContext::Null,
                MethodCallReferences {
                    outputs: &[failed],
                    ..MethodCallReferences::default()
                },
            )
            .unwrap();
        builder
            .with_method_call(
                &return_values_address,
                &ReturnValuesStoreArgs::new(&wallet_address, &0),
                TransactionMethodContext::Null,
                MethodCallReferences {
                    inputs: &[Some(failed)],
                    ..MethodCallReferences::default()
                },
            )
            .unwrap();
        builder.into_aligned_bytes().unwrap()
    };

    let result = executor.transaction_emulate(Address::NULL, slots, |env| {
        let mut external_args_buffer = [ptr::null_mut(); EXTERNAL_ARGS_BUFFER_SIZE];
        let mut output_buffer = [MaybeUninit::uninit(); OUTPUT_BUFFER_SIZE];
        let mut declared_outputs = [MaybeUninit::uninit(); MAX_OUTPUTS];
        let caller = TransactionPayloadCaller::new(
            payload.as_u128_slice(),
            &mut external_args_buffer,
            &mut output_buffer,
            &mut declared_outputs,
            |_method_context| MethodContext::Reset,
        )
        .unwrap();

        // The caller is only returned when the method succeeds, so there is nothing to call the
        // second method with
        caller.call_next_method(env).map(|_caller| ())
    });

    let Err(TransactionPayloadCallError::Method(error)) = result else {
        panic!("The first method must fail: {result:?}");
    };
    assert_eq!(error, ContractError::Forbidden);
    // The caller didn't call the second method either, which would have stored a value in the
    // slot
    assert_eq!(
        executor.with_env_ro(slots, |env| {
            env.return_values_stored(return_values_address, &wallet_address)
        }),
        Err(ContractError::NotFound)
    );

    // The wallet fails the transaction with the error of the first method
    let read_slots = &[];
    let write_slots = &[TransactionSlot {
        owner: wallet_address,
        contract: return_values_address,
    }];
    let seal = hash_and_sign(
        &keypair,
        &header,
        read_slots,
        write_slots,
        payload.as_u128_slice(),
        0,
    );

    assert_eq!(
        executor.transaction_execute(
            Transaction {
                header: &header,
                payload: payload.as_u128_slice(),
                read_slots,
                write_slots,
                seal: seal.as_bytes(),
            },
            slots,
        ),
        Err(ContractError::Forbidden)
    );
}
