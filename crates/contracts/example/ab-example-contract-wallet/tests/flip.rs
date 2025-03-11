// Auto-generated constants will conflict with the main crate when `guest` feature is enabled
#![cfg(not(feature = "guest"))]

use crate::ffi::flip::FlipperFlipArgs;
use ab_contracts_common::env::{
    Blake3Hash, MethodContext, Transaction, TransactionHeader, TransactionSlot,
};
use ab_contracts_common::{Address, Contract, ShardIndex};
use ab_contracts_executor::NativeExecutor;
use ab_contracts_io_type::trivial_type::TrivialType;
use ab_contracts_macros::contract;
use ab_contracts_standards::tx_handler::TxHandler;
use ab_example_contract_wallet::{ExampleWallet, ExampleWalletExt};
use ab_system_contract_code::CodeExt;
use ab_system_contract_simple_wallet_base::payload::TransactionMethodContext;
use ab_system_contract_simple_wallet_base::payload::builder::TransactionPayloadBuilder;
use ab_system_contract_simple_wallet_base::seal::hash_and_sign;
use schnorrkel::Keypair;

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct Flipper {
    pub value: bool,
}

#[contract]
impl Flipper {
    #[init]
    pub fn new(#[input] &init_value: &bool) -> Self {
        Self { value: init_value }
    }

    #[update]
    pub fn flip(&mut self) {
        self.value = !self.value;
    }
}

#[test]
fn flip() {
    let shard_index = ShardIndex::from_u32(1).unwrap();
    let executor = NativeExecutor::builder(shard_index)
        .with_contract::<ExampleWallet>()
        .with_contract_trait::<ExampleWallet, dyn TxHandler>()
        .with_contract::<Flipper>()
        .build()
        .unwrap();

    let slots = &mut executor.new_storage_slots().unwrap();

    let keypair = Keypair::generate();

    let wallet_address = executor
        .transaction_emulate(Address::NULL, slots, |env| {
            // Deploy
            let wallet_address = env
                .code_deploy(
                    MethodContext::Keep,
                    Address::SYSTEM_CODE,
                    &ExampleWallet::code(),
                )
                .unwrap();

            // Initialize state
            env.example_wallet_initialize(
                MethodContext::Keep,
                wallet_address,
                &keypair.public.to_bytes(),
            )
            .unwrap();

            wallet_address
        })
        .unwrap();

    let flipper_address = executor
        .transaction_emulate(Address::NULL, slots, |env| {
            // Deploy
            let flipper_address = env
                .code_deploy(MethodContext::Keep, Address::SYSTEM_CODE, &Flipper::code())
                .unwrap();

            // Initialize state
            env.flipper_new(MethodContext::Keep, flipper_address, &true)
                .unwrap();

            flipper_address
        })
        .unwrap();

    let header = TransactionHeader {
        block_hash: Blake3Hash::default(),
        gas_limit: Default::default(),
        contract: wallet_address,
    };
    let payload = {
        let mut builder = TransactionPayloadBuilder::default();
        builder
            .with_method_call(
                &flipper_address,
                &FlipperFlipArgs::new(),
                TransactionMethodContext::Null,
                &[],
            )
            .unwrap();
        builder.into_aligned_bytes()
    };
    let read_slots = &[];
    let write_slots = &[TransactionSlot {
        owner: flipper_address,
        contract: Address::SYSTEM_STATE,
    }];
    let nonce = 0;

    {
        let seal = hash_and_sign(&keypair, &header, read_slots, write_slots, &payload, nonce);
        executor
            .transaction_verify(
                Transaction {
                    header: &header,
                    payload: &payload,
                    read_slots,
                    write_slots,
                    seal: seal.as_bytes(),
                },
                slots,
            )
            .unwrap();
    }

    for nonce in (nonce..).take(2) {
        let seal = hash_and_sign(&keypair, &header, read_slots, write_slots, &payload, nonce);

        executor
            .transaction_execute(
                Transaction {
                    header: &header,
                    payload: &payload,
                    read_slots,
                    write_slots,
                    seal: seal.as_bytes(),
                },
                slots,
            )
            .unwrap();
    }
}
