//! Simple wallet transactions give outputs to methods empty, so methods called from transactions
//! accept empty outputs like `MaybeData`, see `TransactionPayloadBuilder::with_method_call()`

// Auto-generated constants will conflict with the main crate when `guest` feature is enabled
#![cfg(not(feature = "guest"))]

use crate::payload_outputs::ffi::maybe_data_output::OutputsMaybeDataOutputArgs;
use crate::test_wallet::{TestWallet, TestWalletExt};
use ab_contracts_common::env::MethodContext;
use ab_contracts_common::{Contract, ContractError};
use ab_contracts_macros::contract;
use ab_contracts_standards::tx_handler::TxHandler;
use ab_core_primitives::address::Address;
use ab_core_primitives::block::BlockRoot;
use ab_core_primitives::shard::ShardIndex;
use ab_core_primitives::transaction::owned::OwnedTransaction;
use ab_core_primitives::transaction::{Gas, TransactionHeader};
use ab_executor_native::NativeExecutor;
use ab_io_type::maybe_data::MaybeData;
use ab_io_type::trivial_type::TrivialType;
use ab_system_contract_code::CodeExt;
use ab_system_contract_simple_wallet_base::payload::TransactionMethodContext;
use ab_system_contract_simple_wallet_base::payload::builder::{
    MethodCallReferences, TransactionPayloadBuilder,
};
use ab_system_contract_simple_wallet_base::seal::hash_and_sign;
use schnorrkel::Keypair;

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct Outputs;

#[contract]
impl Outputs {
    /// Fails unless the output is given empty
    #[view]
    pub fn maybe_data_output(#[output] value: &mut MaybeData<'_, u8>) -> Result<(), ContractError> {
        if value.get().is_some() {
            return Err(ContractError::Forbidden);
        }

        value.replace(1);

        Ok(())
    }
}

#[test]
fn maybe_data_output() {
    let executor = NativeExecutor::builder(ShardIndex::new(1).unwrap())
        .with_contract::<TestWallet>()
        .with_contract_trait::<TestWallet, dyn TxHandler>()
        .with_contract::<Outputs>()
        .build()
        .unwrap();
    let slots = &mut executor.new_storage_slots().unwrap();
    let keypair = Keypair::generate();

    let (wallet_address, outputs_address) =
        executor.transaction_emulate(Address::NULL, slots, |env| {
            let wallet_address = env
                .code_deploy(
                    MethodContext::Keep,
                    Address::SYSTEM_CODE,
                    &TestWallet::code(),
                )
                .unwrap();
            env.test_wallet_initialize(
                MethodContext::Keep,
                wallet_address,
                &keypair.public.to_bytes(),
            )
            .unwrap();
            let outputs_address = env
                .code_deploy(MethodContext::Keep, Address::SYSTEM_CODE, &Outputs::code())
                .unwrap();

            (wallet_address, outputs_address)
        });

    let header = TransactionHeader {
        version: 0,
        block_root: BlockRoot::default(),
        gas_limit: Gas::default(),
        contract: wallet_address,
    };
    // The output is given empty, with capacity for `u8`
    let payload = {
        let mut value = 0;
        let mut size = 0;
        let mut builder = TransactionPayloadBuilder::new(1024, 1).unwrap();
        builder
            .with_method_call(
                &outputs_address,
                &OutputsMaybeDataOutputArgs::new(
                    &mut MaybeData::from_mut(&mut value, &mut size).unwrap(),
                ),
                TransactionMethodContext::Null,
                MethodCallReferences {
                    outputs: &[0],
                    ..MethodCallReferences::default()
                },
            )
            .unwrap();
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
    )
    .unwrap();

    assert_eq!(
        executor.transaction_execute(
            OwnedTransaction::from_parts(
                &header,
                read_slots,
                write_slots,
                payload.as_u128_slice(),
                seal.as_bytes()
            )
            .unwrap()
            .transaction(),
            slots,
        ),
        Ok(())
    );
}
