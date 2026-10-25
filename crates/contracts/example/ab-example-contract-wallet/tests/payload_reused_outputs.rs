//! Simple wallet transactions can write the same declared output from several methods: an
//! `#[output]` is given empty whatever was written before, and later methods get what was written
//! last, see `TransactionPayloadBuilder::with_method_call()`

// Auto-generated constants will conflict with the main crate when `guest` feature is enabled
#![cfg(not(feature = "guest"))]

use crate::ffi::check_address::ReusedOutputsCheckAddressArgs;
use crate::ffi::check_byte::ReusedOutputsCheckByteArgs;
use crate::ffi::fill::ReusedOutputsFillArgs;
use crate::ffi::small::ReusedOutputsSmallArgs;
use ab_contracts_common::env::MethodContext;
use ab_contracts_common::{Contract, ContractError};
use ab_contracts_macros::contract;
use ab_contracts_standards::tx_handler::TxHandler;
use ab_core_primitives::address::Address;
use ab_core_primitives::block::BlockRoot;
use ab_core_primitives::shard::ShardIndex;
use ab_core_primitives::transaction::{Gas, Transaction, TransactionHeader};
use ab_example_contract_wallet::{ExampleWallet, ExampleWalletExt};
use ab_executor_native::NativeExecutor;
use ab_io_type::maybe_data::MaybeData;
use ab_io_type::trivial_type::TrivialType;
use ab_io_type::variable_bytes::VariableBytes;
use ab_system_contract_code::CodeExt;
use ab_system_contract_simple_wallet_base::payload::TransactionMethodContext;
use ab_system_contract_simple_wallet_base::payload::builder::{
    MethodCallReferences, TransactionPayloadBuilder,
};
use ab_system_contract_simple_wallet_base::seal::hash_and_sign;
use core::mem::MaybeUninit;
use schnorrkel::Keypair;

/// The address that [`ReusedOutputs::fill()`] writes
const FILLED: Address = Address::SYSTEM_STATE;
/// The value that [`ReusedOutputs::small()`] returns
const SMALL: u8 = 7;

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct ReusedOutputs;

#[contract]
impl ReusedOutputs {
    /// Fails unless the output is given empty, then writes [`FILLED`]
    #[view]
    pub fn fill(#[output] address: &mut MaybeData<'_, Address>) -> Result<(), ContractError> {
        if address.get().is_some() {
            return Err(ContractError::Forbidden);
        }

        address.replace(FILLED);

        Ok(())
    }

    /// Fails unless `address` is [`FILLED`]
    #[view]
    pub fn check_address(#[input] address: &Address) -> Result<(), ContractError> {
        if *address != FILLED {
            return Err(ContractError::BadInput);
        }

        Ok(())
    }

    /// Returns [`SMALL`]
    #[view]
    pub fn small() -> u8 {
        SMALL
    }

    /// Fails unless `value` is the single byte [`SMALL`]
    #[view]
    pub fn check_byte(#[input] value: &VariableBytes<'_>) -> Result<(), ContractError> {
        if value.get_initialized() != [SMALL] {
            return Err(ContractError::BadInput);
        }

        Ok(())
    }
}

fn add_fill(builder: &mut TransactionPayloadBuilder, contract: &Address, output: u8) {
    let mut address = Address::NULL;
    let mut size = 0;
    builder
        .with_method_call(
            contract,
            &ReusedOutputsFillArgs::new(&mut MaybeData::from_mut(&mut address, &mut size).unwrap()),
            TransactionMethodContext::Null,
            MethodCallReferences {
                outputs: &[output],
                ..MethodCallReferences::default()
            },
        )
        .unwrap();
}

fn add_check_address(builder: &mut TransactionPayloadBuilder, contract: &Address, output: u8) {
    builder
        .with_method_call(
            contract,
            &ReusedOutputsCheckAddressArgs::new(&Address::NULL),
            TransactionMethodContext::Null,
            MethodCallReferences {
                inputs: &[Some(output)],
                ..MethodCallReferences::default()
            },
        )
        .unwrap();
}

fn add_small(builder: &mut TransactionPayloadBuilder, contract: &Address, output: u8) {
    builder
        .with_method_call(
            contract,
            &ReusedOutputsSmallArgs::new(&mut MaybeUninit::uninit()),
            TransactionMethodContext::Null,
            MethodCallReferences {
                outputs: &[output],
                ..MethodCallReferences::default()
            },
        )
        .unwrap();
}

fn add_check_byte(builder: &mut TransactionPayloadBuilder, contract: &Address, output: u8) {
    builder
        .with_method_call(
            contract,
            &ReusedOutputsCheckByteArgs::new(&VariableBytes::from_buffer(&[], &0).unwrap()),
            TransactionMethodContext::Null,
            MethodCallReferences {
                inputs: &[Some(output)],
                ..MethodCallReferences::default()
            },
        )
        .unwrap();
}

#[test]
fn reused_output() {
    let executor = NativeExecutor::builder(ShardIndex::new(1).unwrap())
        .with_contract::<ExampleWallet>()
        .with_contract_trait::<ExampleWallet, dyn TxHandler>()
        .with_contract::<ReusedOutputs>()
        .build()
        .unwrap();
    let slots = &mut executor.new_storage_slots().unwrap();
    let keypair = Keypair::generate();

    let (wallet_address, reused_outputs_address) =
        executor.transaction_emulate(Address::NULL, slots, |env| {
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
            let reused_outputs_address = env
                .code_deploy(
                    MethodContext::Keep,
                    Address::SYSTEM_CODE,
                    &ReusedOutputs::code(),
                )
                .unwrap();

            (wallet_address, reused_outputs_address)
        });

    let header = TransactionHeader {
        version: 0,
        block_root: BlockRoot::default(),
        gas_limit: Gas::default(),
        contract: wallet_address,
    };
    // Every method uses the only declared output, whose capacity the builder computes as the 16
    // bytes of the address that `fill()` writes. The second `fill()` fails unless it is given the
    // output empty, although its memory holds the address. `small()` returns a single byte into
    // that memory, so `check_byte()` fails if it is given the size of the address. The last two
    // methods write and read the address again after the return value.
    let payload = {
        let output = 0;
        let mut builder = TransactionPayloadBuilder::new(1024, 1).unwrap();
        add_fill(&mut builder, &reused_outputs_address, output);
        add_check_address(&mut builder, &reused_outputs_address, output);
        add_fill(&mut builder, &reused_outputs_address, output);
        add_check_address(&mut builder, &reused_outputs_address, output);
        add_small(&mut builder, &reused_outputs_address, output);
        add_check_byte(&mut builder, &reused_outputs_address, output);
        add_fill(&mut builder, &reused_outputs_address, output);
        add_check_address(&mut builder, &reused_outputs_address, output);
        builder.into_aligned_bytes().unwrap()
    };
    let read_slots = &[];
    let write_slots = &[];
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
}
