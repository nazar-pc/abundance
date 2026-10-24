#![allow(
    unexpected_cfgs,
    reason = "Intentionally not adding `guest` feature, this is a test utility not to be deployed"
)]

use ab_contracts_common::ContractError;
use ab_contracts_common::env::{Env, MethodContext};
use ab_contracts_macros::contract;
use ab_contracts_standards::tx_handler::{
    TxHandler, TxHandlerPayload, TxHandlerSeal, TxHandlerSlots,
};
use ab_core_primitives::address::Address;
use ab_core_primitives::transaction::TransactionHeader;
use ab_io_type::trivial_type::TrivialType;
use ab_system_contract_simple_wallet_base::SimpleWalletBaseExt;

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct DummyWallet;

#[contract]
impl TxHandler for DummyWallet {
    #[view]
    fn authorize(
        #[env] _env: &Env<'_>,
        #[input] _header: &TransactionHeader,
        #[input] _read_slots: &TxHandlerSlots<'_>,
        #[input] _write_slots: &TxHandlerSlots<'_>,
        #[input] _payload: &TxHandlerPayload<'_>,
        #[input] _seal: &TxHandlerSeal<'_>,
    ) -> Result<(), ContractError> {
        // Allow any transaction
        Ok(())
    }

    #[update]
    fn execute(
        #[env] env: &mut Env<'_>,
        #[input] header: &TransactionHeader,
        #[input] read_slots: &TxHandlerSlots<'_>,
        #[input] write_slots: &TxHandlerSlots<'_>,
        #[input] payload: &TxHandlerPayload<'_>,
        #[input] seal: &TxHandlerSeal<'_>,
    ) -> Result<(), ContractError> {
        env.simple_wallet_base_execute(
            MethodContext::Replace,
            Address::SYSTEM_SIMPLE_WALLET_BASE,
            header,
            read_slots,
            write_slots,
            payload,
            seal,
        )
    }
}

#[contract]
impl DummyWallet {}
