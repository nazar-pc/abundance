use crate::{SimpleWalletBaseExt, WalletState};
use ab_contracts_common::env::{Env, MethodContext};
use ab_contracts_common::transaction::TransactionHeader;
use ab_contracts_common::{Address, ContractError};
use ab_contracts_io_type::IoType;
use ab_contracts_io_type::trivial_type::TrivialType;
use ab_contracts_io_type::variable_bytes::VariableBytes;
use ab_contracts_standards::tx_handler::{TxHandlerPayload, TxHandlerSeal, TxHandlerSlots};
use ab_system_contract_state::StateExt;
use core::mem::MaybeUninit;

/// Utility function to initialize the state of the wallet in a typical setup
#[inline(always)]
pub fn initialize_state(env: &mut Env<'_>, public_key: &[u8; 32]) -> Result<(), ContractError> {
    let state =
        env.simple_wallet_base_initialize(Address::SYSTEM_SIMPLE_WALLET_BASE, public_key)?;

    env.state_initialize(
        MethodContext::Reset,
        Address::SYSTEM_STATE,
        &env.own_address(),
        &VariableBytes::from_buffer(state.as_bytes(), &state.size()),
    )
}

/// Utility function to authorize transaction with the wallet in a typical setup
#[inline(always)]
pub fn authorize(
    env: &Env<'_>,
    header: &TransactionHeader,
    read_slots: &TxHandlerSlots,
    write_slots: &TxHandlerSlots,
    payload: &TxHandlerPayload,
    seal: &TxHandlerSeal,
) -> Result<(), ContractError> {
    let state = load_current_state(env)?;

    env.simple_wallet_base_authorize(
        Address::SYSTEM_SIMPLE_WALLET_BASE,
        &state,
        header,
        read_slots,
        write_slots,
        payload,
        seal,
    )
}

/// Utility function to execute transaction with the wallet in a typical setup and increase nonce
/// afterward
#[inline(always)]
pub fn execute(
    env: &mut Env<'_>,
    header: &TransactionHeader,
    read_slots: &TxHandlerSlots,
    write_slots: &TxHandlerSlots,
    payload: &TxHandlerPayload,
    seal: &TxHandlerSeal,
) -> Result<(), ContractError> {
    // Only execution environment is allowed to make this call
    if env.caller() != Address::NULL {
        return Err(ContractError::Forbidden);
    }

    // Read existing state
    let old_state = load_current_state(env)?;

    env.simple_wallet_base_execute(
        MethodContext::Replace,
        Address::SYSTEM_SIMPLE_WALLET_BASE,
        header,
        read_slots,
        write_slots,
        payload,
        seal,
    )?;

    // Manual state management due to the possibility that one of the calls during execution above
    // may update the state too (like changing public key)
    {
        // Fill `new_state` with updated `old_state` containing increased nonce
        let new_state =
            env.simple_wallet_base_increase_nonce(Address::SYSTEM_SIMPLE_WALLET_BASE, &old_state)?;
        // Write new state of the contract, this can only be done by the direct owner
        env.state_compare_and_write(
            MethodContext::Reset,
            Address::SYSTEM_STATE,
            &env.own_address(),
            &VariableBytes::from_buffer(old_state.as_bytes(), &old_state.size()),
            &VariableBytes::from_buffer(new_state.as_bytes(), &new_state.size()),
        )
        .map(|_| ())
    }
}

/// Utility function to change public key of the wallet in a typical setup
#[inline(always)]
pub fn change_public_key(env: &mut Env<'_>, public_key: &[u8; 32]) -> Result<(), ContractError> {
    // Only the system simple wallet base contract under the context of this contract is allowed
    // to change public key
    if !(env.context() == env.own_address() && env.caller() == Address::SYSTEM_SIMPLE_WALLET_BASE) {
        return Err(ContractError::Forbidden);
    }

    // Read existing state
    let old_state = load_current_state(env)?;
    // Fill `new_state` with updated `old_state` containing new public key
    let new_state = env.simple_wallet_base_change_public_key(
        Address::SYSTEM_SIMPLE_WALLET_BASE,
        &old_state,
        public_key,
    )?;
    // Write new state of the contract, this can only be done by the direct owner
    env.state_write(
        MethodContext::Reset,
        Address::SYSTEM_STATE,
        &env.own_address(),
        &VariableBytes::from_buffer(new_state.as_bytes(), &new_state.size()),
    )
}

#[inline(always)]
fn load_current_state(env: &Env<'_>) -> Result<WalletState, ContractError> {
    let current_state = {
        let mut current_state = MaybeUninit::<WalletState>::uninit();
        let mut current_state_size = 0;
        env.state_read(
            Address::SYSTEM_STATE,
            &env.own_address(),
            &mut VariableBytes::from_uninit(current_state.as_bytes_mut(), &mut current_state_size),
        )?;
        if current_state_size != WalletState::SIZE {
            return Err(ContractError::BadOutput);
        }
        // Just initialized
        unsafe { current_state.assume_init() }
    };

    Ok(current_state)
}
