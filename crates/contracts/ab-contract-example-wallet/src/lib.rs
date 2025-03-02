#![no_std]

use ab_contracts_common::env::{Env, MethodContext, TransactionHeader};
use ab_contracts_common::{Address, ContractError};
use ab_contracts_io_type::trivial_type::TrivialType;
use ab_contracts_macros::contract;
use ab_contracts_standards::tx_handler::{TxHandler, TxHandlerPayload, TxHandlerSeal};
use ab_system_contract_simple_wallet_base::SimpleWalletBaseExt;
use ab_system_contract_state::{StateExt, with_state_buffer};

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
pub struct ExampleWallet;

#[contract]
impl TxHandler for ExampleWallet {
    #[view]
    fn authorize(
        #[env] env: &Env,
        #[input] header: &TransactionHeader,
        #[input] payload: &TxHandlerPayload,
        #[input] seal: &TxHandlerSeal,
    ) -> Result<(), ContractError> {
        env.simple_wallet_base_authorize(
            Address::SYSTEM_SIMPLE_WALLET_BASE,
            &env.own_address(),
            header,
            payload,
            seal,
        )
    }

    #[update]
    fn execute(
        #[env] env: &mut Env,
        #[input] header: &TransactionHeader,
        #[input] payload: &TxHandlerPayload,
        #[input] seal: &TxHandlerSeal,
    ) -> Result<(), ContractError> {
        // Only execution environment is allowed to make this call
        if env.caller() != Address::NULL {
            return Err(ContractError::Forbidden);
        }

        env.simple_wallet_base_execute(
            MethodContext::Replace,
            Address::SYSTEM_SIMPLE_WALLET_BASE,
            header,
            payload,
            seal,
        )?;

        // Manual state management due to the possibility that one of the calls during execution
        // above may update the state.
        with_state_buffer(|state| {
            // Fill `state` with updated state containing increased nonce
            env.simple_wallet_base_increase_nonce(
                Address::SYSTEM_SIMPLE_WALLET_BASE,
                &env.own_address(),
                seal,
                state,
            )?;
            // Write new state of the contract, this can only be done by the direct owner
            env.state_write(
                MethodContext::Reset,
                Address::SYSTEM_STATE,
                &env.own_address(),
                state,
            )
        })
    }
}

/// TODO: Support upgrading wallet to a different implementation
#[contract]
impl ExampleWallet {
    /// Initialize a wallet with specified public key
    #[update]
    pub fn initialize(
        #[env] env: &mut Env,
        #[input] public_key: &[u8; 32],
    ) -> Result<(), ContractError> {
        with_state_buffer(|state| {
            // Fill `state` with initialized wallet state
            env.simple_wallet_base_initialize(
                Address::SYSTEM_SIMPLE_WALLET_BASE,
                public_key,
                state,
            )?;
            // Initialize state of the contract, this can only be done by the direct owner
            env.state_initialize(
                MethodContext::Reset,
                Address::SYSTEM_STATE,
                &env.own_address(),
                state,
            )
        })
    }

    /// Change public key to a different one
    #[update]
    pub fn change_public_key(
        #[env] env: &mut Env,
        #[input] public_key: &[u8; 32],
    ) -> Result<(), ContractError> {
        // Only the system simple wallet base contract under the context of this contract is allowed
        // to change public key
        if !(env.context() == env.own_address()
            && env.caller() == Address::SYSTEM_SIMPLE_WALLET_BASE)
        {
            return Err(ContractError::Forbidden);
        }

        with_state_buffer(|state| {
            // Fill `state` with updated state containing changed public key
            env.simple_wallet_base_change_public_key(
                Address::SYSTEM_SIMPLE_WALLET_BASE,
                &env.own_address(),
                public_key,
                state,
            )?;
            // Write new state of the contract, this can only be done by the direct owner
            env.state_write(
                MethodContext::Reset,
                Address::SYSTEM_STATE,
                &env.own_address(),
                state,
            )
        })
    }
}
