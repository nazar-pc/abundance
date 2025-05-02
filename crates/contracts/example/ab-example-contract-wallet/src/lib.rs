#![no_std]

use ab_contracts_common::ContractError;
use ab_contracts_common::env::Env;
use ab_contracts_macros::contract;
use ab_contracts_standards::tx_handler::{
    TxHandler, TxHandlerPayload, TxHandlerSeal, TxHandlerSlots,
};
use ab_core_primitives::transaction::TransactionHeader;
use ab_io_type::trivial_type::TrivialType;
use ab_system_contract_simple_wallet_base::utils::{
    authorize, change_public_key, execute, initialize_state,
};

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct ExampleWallet;

#[contract]
impl TxHandler for ExampleWallet {
    #[view]
    fn authorize(
        #[env] env: &Env<'_>,
        #[input] header: &TransactionHeader,
        #[input] read_slots: &TxHandlerSlots,
        #[input] write_slots: &TxHandlerSlots,
        #[input] payload: &TxHandlerPayload,
        #[input] seal: &TxHandlerSeal,
    ) -> Result<(), ContractError> {
        authorize(env, header, read_slots, write_slots, payload, seal)
    }

    #[update]
    fn execute(
        #[env] env: &mut Env<'_>,
        #[input] header: &TransactionHeader,
        #[input] read_slots: &TxHandlerSlots,
        #[input] write_slots: &TxHandlerSlots,
        #[input] payload: &TxHandlerPayload,
        #[input] seal: &TxHandlerSeal,
    ) -> Result<(), ContractError> {
        execute(env, header, read_slots, write_slots, payload, seal)
    }
}

/// TODO: Support upgrading wallet to a different implementation
#[contract]
impl ExampleWallet {
    /// Initialize a wallet with specified public key
    #[update]
    pub fn initialize(
        #[env] env: &mut Env<'_>,
        #[input] public_key: &[u8; 32],
    ) -> Result<(), ContractError> {
        initialize_state(env, public_key)
    }

    /// Change public key to a different one
    #[update]
    pub fn change_public_key(
        #[env] env: &mut Env<'_>,
        #[input] public_key: &[u8; 32],
    ) -> Result<(), ContractError> {
        change_public_key(env, public_key)
    }
}
