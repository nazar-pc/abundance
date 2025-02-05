#![no_std]

use ab_contracts_common::env::Env;
use ab_contracts_common::{Address, ContractError};
use ab_contracts_io_type::trivial_type::TrivialType;
use ab_contracts_io_type::variable_bytes::VariableBytes;
use ab_contracts_macros::contract;

// TODO: How/where should this limit defined?
pub const MAX_STATE_SIZE: u32 = 1024 * 1024;

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
pub struct State;

#[contract]
impl State {
    /// Write contract's state
    #[update]
    pub fn write(
        #[env] env: &mut Env,
        #[slot] (address, contract_state): (&Address, &mut VariableBytes<MAX_STATE_SIZE>),
        #[input] new_state: &VariableBytes<MAX_STATE_SIZE>,
    ) -> Result<(), ContractError> {
        // TODO: Check shard
        if env.caller() != address {
            return Err(ContractError::Forbidden);
        }

        if !contract_state.copy_from(new_state) {
            return Err(ContractError::BadInput);
        }

        Ok(())
    }

    /// Read contract's state
    #[view]
    pub fn read(
        #[slot] contract_state: &VariableBytes<MAX_STATE_SIZE>,
        #[output] state: &mut VariableBytes<MAX_STATE_SIZE>,
    ) -> Result<(), ContractError> {
        if state.copy_from(contract_state) {
            Ok(())
        } else {
            Err(ContractError::BadInput)
        }
    }

    /// Check if contract's state is empty
    #[view]
    pub fn is_empty(#[slot] contract_state: &VariableBytes<MAX_STATE_SIZE>) -> bool {
        contract_state.size() == 0
    }
}
