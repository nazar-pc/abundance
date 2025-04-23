#![no_std]

use ab_contracts_common::env::Env;
use ab_contracts_common::{Address, ContractError};
use ab_contracts_io_type::bool::Bool;
use ab_contracts_io_type::trivial_type::TrivialType;
use ab_contracts_io_type::variable_bytes::VariableBytes;
use ab_contracts_macros::contract;
use core::mem::MaybeUninit;

// TODO: How/where should this limit be defined?
pub const RECOMMENDED_STATE_CAPACITY: u32 = 1024;

/// Helper function that calls provided function with new empty state buffer
#[inline]
pub fn with_state_buffer<F, R>(f: F) -> R
where
    F: FnOnce(&mut VariableBytes<RECOMMENDED_STATE_CAPACITY>) -> R,
{
    let mut state_bytes = [MaybeUninit::uninit(); RECOMMENDED_STATE_CAPACITY as usize];
    let mut state_size = 0;
    let mut new_state = VariableBytes::from_uninit(&mut state_bytes, &mut state_size);
    f(&mut new_state)
}

/// Helper function that calls provided function with new empty state buffer pair
#[inline]
pub fn with_state_buffer_pair<F, R>(f: F) -> R
where
    F: FnOnce(
        &mut VariableBytes<RECOMMENDED_STATE_CAPACITY>,
        &mut VariableBytes<RECOMMENDED_STATE_CAPACITY>,
    ) -> R,
{
    let mut state_bytes_a = [MaybeUninit::uninit(); RECOMMENDED_STATE_CAPACITY as usize];
    let mut state_size_a = 0;
    let mut new_state_a = VariableBytes::from_uninit(&mut state_bytes_a, &mut state_size_a);

    let mut state_bytes_b = [MaybeUninit::uninit(); RECOMMENDED_STATE_CAPACITY as usize];
    let mut state_size_b = 0;
    let mut new_state_b = VariableBytes::from_uninit(&mut state_bytes_b, &mut state_size_b);

    f(&mut new_state_a, &mut new_state_b)
}

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct State;

#[contract]
impl State {
    /// Initialize state.
    ///
    /// Similar to [`State::write()`], but returns error if the state is not empty.
    #[update]
    pub fn initialize(
        #[env] env: &mut Env<'_>,
        #[slot] (address, contract_state): (
            &Address,
            &mut VariableBytes<RECOMMENDED_STATE_CAPACITY>,
        ),
        #[input] state: &VariableBytes<RECOMMENDED_STATE_CAPACITY>,
    ) -> Result<(), ContractError> {
        if !Self::is_empty(contract_state).get() {
            return Err(ContractError::Conflict);
        }

        Self::write(env, (address, contract_state), state)
    }

    /// Write state.
    ///
    /// Only direct caller is allowed to write its own state for security reasons.
    #[update]
    pub fn write(
        #[env] env: &mut Env<'_>,
        #[slot] (address, state): (&Address, &mut VariableBytes<RECOMMENDED_STATE_CAPACITY>),
        #[input] new_state: &VariableBytes<RECOMMENDED_STATE_CAPACITY>,
    ) -> Result<(), ContractError> {
        // TODO: Check shard?
        if env.caller() != address {
            return Err(ContractError::Forbidden);
        }

        if !state.copy_from(new_state) {
            return Err(ContractError::BadInput);
        }

        Ok(())
    }

    /// Compare state with a given old state and write new state if old state matches.
    ///
    /// Only direct caller is allowed to write its own state for security reasons.
    ///
    /// Returns boolean indicating whether write happened or not.
    #[update]
    pub fn compare_and_write(
        #[env] env: &mut Env<'_>,
        #[slot] (address, state): (&Address, &mut VariableBytes<RECOMMENDED_STATE_CAPACITY>),
        #[input] old_state: &VariableBytes<RECOMMENDED_STATE_CAPACITY>,
        #[input] new_state: &VariableBytes<RECOMMENDED_STATE_CAPACITY>,
    ) -> Result<Bool, ContractError> {
        // TODO: Check shard?
        if env.caller() != address {
            return Err(ContractError::Forbidden);
        }

        if state.get_initialized() != old_state.get_initialized() {
            return Ok(Bool::new(false));
        }

        if !state.copy_from(new_state) {
            return Err(ContractError::BadInput);
        }

        Ok(Bool::new(true))
    }

    /// Read state
    #[view]
    pub fn read(
        #[slot] contract_state: &VariableBytes<RECOMMENDED_STATE_CAPACITY>,
        #[output] state: &mut VariableBytes<RECOMMENDED_STATE_CAPACITY>,
    ) -> Result<(), ContractError> {
        if state.copy_from(contract_state) {
            Ok(())
        } else {
            Err(ContractError::BadInput)
        }
    }

    /// Check if the state is empty
    #[view]
    pub fn is_empty(#[slot] contract_state: &VariableBytes<RECOMMENDED_STATE_CAPACITY>) -> Bool {
        Bool::new(contract_state.size() == 0)
    }
}
