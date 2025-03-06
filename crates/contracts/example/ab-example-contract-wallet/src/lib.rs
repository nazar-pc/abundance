#![no_std]

use ab_contracts_common::env::{Env, MethodContext, TransactionHeader};
use ab_contracts_common::{Address, ContractError};
use ab_contracts_io_type::trivial_type::TrivialType;
use ab_contracts_macros::contract;
use ab_contracts_standards::tx_handler::{
    TxHandler, TxHandlerPayload, TxHandlerSeal, TxHandlerSlots,
};
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
        #[input] read_slots: &TxHandlerSlots,
        #[input] write_slots: &TxHandlerSlots,
        #[input] payload: &TxHandlerPayload,
        #[input] seal: &TxHandlerSeal,
    ) -> Result<(), ContractError> {
        with_state_buffer(|state| {
            env.state_read(Address::SYSTEM_STATE, &env.own_address(), state)?;

            env.simple_wallet_base_authorize(
                Address::SYSTEM_SIMPLE_WALLET_BASE,
                state.cast_ref(),
                header,
                read_slots,
                write_slots,
                payload,
                seal,
            )
        })
    }

    #[update]
    fn execute(
        #[env] env: &mut Env,
        #[input] header: &TransactionHeader,
        #[input] read_slots: &TxHandlerSlots,
        #[input] write_slots: &TxHandlerSlots,
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
            read_slots,
            write_slots,
            payload,
            seal,
        )?;

        // Manual state management due to the possibility that one of the calls during execution
        // above may update the state.
        with_state_buffer(|old_state| {
            env.state_read(Address::SYSTEM_STATE, &env.own_address(), old_state)?;

            with_state_buffer(|new_state| {
                // Fill `state` with updated state containing increased nonce
                env.simple_wallet_base_increase_nonce(
                    Address::SYSTEM_SIMPLE_WALLET_BASE,
                    old_state.cast_ref(),
                    seal,
                    new_state.cast_mut(),
                )?;
                // Write new state of the contract, this can only be done by the direct owner
                env.state_write(
                    MethodContext::Reset,
                    Address::SYSTEM_STATE,
                    &env.own_address(),
                    new_state,
                )
            })
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
                state.cast_mut(),
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

        with_state_buffer(|old_state| {
            env.state_read(Address::SYSTEM_STATE, &env.own_address(), old_state)?;

            with_state_buffer(|new_state| {
                // Fill `state` with updated state containing changed public key
                env.simple_wallet_base_change_public_key(
                    Address::SYSTEM_SIMPLE_WALLET_BASE,
                    old_state.cast_ref(),
                    public_key,
                    new_state.cast_mut(),
                )?;
                // Write new state of the contract, this can only be done by the direct owner
                env.state_write(
                    MethodContext::Reset,
                    Address::SYSTEM_STATE,
                    &env.own_address(),
                    new_state,
                )
            })
        })
    }
}
