// Auto-generated constants will conflict with the main crate when `guest` feature is enabled
#![cfg(not(feature = "guest"))]

use ab_contracts_common::ContractError;
use ab_contracts_common::env::Env;
use ab_contracts_macros::contract;
use ab_contracts_standards::tx_handler::{
    TxHandler, TxHandlerPayload, TxHandlerSeal, TxHandlerSlots,
};
use ab_core_primitives::transaction::TransactionHeader;
use ab_io_type::trivial_type::TrivialType;
use ab_system_contract_simple_wallet_base::utils::{authorize, execute, initialize_state};

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct TestWallet;

#[contract]
impl TxHandler for TestWallet {
    #[view]
    fn authorize(
        #[env] env: &Env<'_>,
        #[input] header: &TransactionHeader,
        #[input] read_slots: &TxHandlerSlots<'_>,
        #[input] write_slots: &TxHandlerSlots<'_>,
        #[input] payload: &TxHandlerPayload<'_>,
        #[input] seal: &TxHandlerSeal<'_>,
    ) -> Result<(), ContractError> {
        authorize(env, header, read_slots, write_slots, payload, seal)
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
        execute(env, header, read_slots, write_slots, payload, seal)
    }
}

#[contract]
impl TestWallet {
    #[update]
    pub fn initialize(
        #[env] env: &mut Env<'_>,
        #[input] public_key: &[u8; 32],
    ) -> Result<(), ContractError> {
        initialize_state(env, public_key)
    }
}
