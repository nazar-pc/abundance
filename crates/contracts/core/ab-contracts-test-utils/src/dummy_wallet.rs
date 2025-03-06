#![allow(
    unexpected_cfgs,
    reason = "Intentionally not adding `guest` feature, this is a test utility not to be deployed"
)]

use ab_contracts_common::env::{Env, MethodContext, TransactionHeader};
use ab_contracts_common::{Address, ContractError};
use ab_contracts_io_type::trivial_type::TrivialType;
use ab_contracts_macros::contract;
use ab_contracts_standards::tx_handler::{TxHandler, TxHandlerPayload, TxHandlerSeal};
use ab_system_contract_simple_wallet_base::SimpleWalletBaseExt;

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
pub struct DummyWallet;

#[contract]
impl TxHandler for DummyWallet {
    #[view]
    fn authorize(
        #[env] _env: &Env,
        #[input] _header: &TransactionHeader,
        #[input] _payload: &TxHandlerPayload,
        #[input] _seal: &TxHandlerSeal,
    ) -> Result<(), ContractError> {
        // Allow any transaction
        Ok(())
    }

    #[update]
    fn execute(
        #[env] env: &mut Env,
        #[input] header: &TransactionHeader,
        #[input] payload: &TxHandlerPayload,
        #[input] seal: &TxHandlerSeal,
    ) -> Result<(), ContractError> {
        env.simple_wallet_base_execute(
            MethodContext::Replace,
            Address::SYSTEM_SIMPLE_WALLET_BASE,
            header,
            payload,
            seal,
        )
    }
}

#[contract]
impl DummyWallet {}
