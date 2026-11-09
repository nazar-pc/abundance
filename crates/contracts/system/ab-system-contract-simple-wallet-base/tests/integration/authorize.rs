// Auto-generated constants will conflict with the main crate when `guest` feature is enabled
#![cfg(not(feature = "guest"))]

use crate::test_wallet::{TestWallet, TestWalletExt};
use ab_contracts_common::env::MethodContext;
use ab_contracts_common::{Contract, ContractError};
use ab_contracts_standards::tx_handler::TxHandler;
use ab_core_primitives::address::Address;
use ab_core_primitives::block::BlockRoot;
use ab_core_primitives::shard::ShardIndex;
use ab_core_primitives::transaction::owned::OwnedTransaction;
use ab_core_primitives::transaction::{Gas, TransactionHeader, TransactionSlot};
use ab_executor_native::NativeExecutor;
use ab_io_type::trivial_type::TrivialType;
use ab_system_contract_code::CodeExt;
use ab_system_contract_simple_wallet_base::seal::hash_and_sign;
use schnorrkel::{ExpansionMode, MiniSecretKey};

#[test]
fn seal_signed_with_write_slot_as_read_slot_is_rejected() {
    let executor = NativeExecutor::builder(ShardIndex::new(1).unwrap())
        .with_contract::<TestWallet>()
        .with_contract_trait::<TestWallet, dyn TxHandler>()
        .build()
        .unwrap();
    let slots = &mut executor.new_storage_slots().unwrap();
    let keypair = MiniSecretKey::from_bytes(&[1; 32])
        .unwrap()
        .expand_to_keypair(ExpansionMode::Ed25519);

    let wallet_address = executor.transaction_emulate(Address::NULL, slots, |env| {
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
        wallet_address
    });

    let header = TransactionHeader {
        version: TransactionHeader::TRANSACTION_VERSION,
        block_root: BlockRoot::default(),
        gas_limit: Gas::default(),
        contract: wallet_address,
    };
    let slot = TransactionSlot {
        owner: wallet_address,
        contract: Address::SYSTEM_STATE,
    };
    let nonce = 0;

    let seal = hash_and_sign(&keypair, &header, &[], &[slot], &[], nonce).unwrap();
    executor
        .transaction_verify(
            OwnedTransaction::from_parts(&header, &[], &[slot], &[], seal.as_bytes())
                .unwrap()
                .transaction(),
            slots,
        )
        .unwrap();

    let seal = hash_and_sign(&keypair, &header, &[slot], &[], &[], nonce).unwrap();
    assert_eq!(
        executor.transaction_verify(
            OwnedTransaction::from_parts(&header, &[], &[slot], &[], seal.as_bytes())
                .unwrap()
                .transaction(),
            slots,
        ),
        Err(ContractError::Forbidden)
    );
}
