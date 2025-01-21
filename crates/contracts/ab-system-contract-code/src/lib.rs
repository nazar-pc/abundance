#![no_std]

use ab_contracts_common::env::{Env, MethodContext};
use ab_contracts_common::{Address, ContractError};
use ab_contracts_io_type::trivial_type::TrivialType;
use ab_contracts_io_type::variable_bytes::VariableBytes;
use ab_contracts_macros::contract;
use ab_system_contract_address_allocator::AddressAllocatorExt;

// TODO: How/where should this limit defined?
pub const MAX_CODE_SIZE: u32 = 1024 * 1024;

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
pub struct Code;

#[contract]
impl Code {
    /// Deploy a new contract with specified code
    #[update]
    pub fn deploy(
        #[env] env: &mut Env,
        #[input] code: &VariableBytes<MAX_CODE_SIZE>,
    ) -> Result<Address, ContractError> {
        let new_contract_address = env.address_allocator_allocate_address(
            &MethodContext::Replace,
            &Address::system_address_allocator(env.shard_index()),
        )?;

        env.code_store(
            &MethodContext::Replace,
            env.own_address(),
            &new_contract_address,
            code,
        )?;

        Ok(new_contract_address)
    }

    /// Store contact's code overriding previous code that might have been there.
    ///
    /// Updates can only be done by the contract itself with direct calls.
    // TODO: Some code validation?
    #[update]
    pub fn store(
        #[env] env: &mut Env,
        #[slot] (address, contract_code): (&Address, &mut VariableBytes<MAX_CODE_SIZE>),
        #[input] new_code: &VariableBytes<MAX_CODE_SIZE>,
    ) -> Result<(), ContractError> {
        // TODO: Would it be helpful to allow indirect updates?
        // Allow updates to system deploy contract (for initial deployment) and to contract itself
        // for upgrades, but only direct calls
        if !(env.caller() == env.own_address() || env.caller() == address) {
            return Err(ContractError::AccessDenied);
        }

        if !contract_code.copy_from(new_code) {
            return Err(ContractError::InvalidInput);
        }

        Ok(())
    }

    /// Read contract's code
    #[view]
    pub fn read(
        #[slot] contract_code: &VariableBytes<MAX_CODE_SIZE>,
        #[output] code: &mut VariableBytes<MAX_CODE_SIZE>,
    ) -> Result<(), ContractError> {
        if code.copy_from(contract_code) {
            Ok(())
        } else {
            Err(ContractError::InvalidInput)
        }
    }
}
