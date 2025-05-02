#![no_std]

use ab_contracts_common::ContractError;
use ab_contracts_common::env::Env;
use ab_contracts_macros::contract;
use ab_core_primitives::address::Address;
use ab_core_primitives::block::{BlockHash, BlockNumber};
use ab_io_type::trivial_type::TrivialType;

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct Block {
    pub number: BlockNumber,
    pub parent_hash: BlockHash,
}

// TODO: Probably maintain a history of recent block headers and allow to extract them
#[contract]
impl Block {
    /// Initialize block state at genesis
    #[init]
    pub fn genesis() -> Self {
        Self {
            number: BlockNumber::ZERO,
            parent_hash: BlockHash::default(),
        }
    }

    /// Initialize new block
    #[update]
    pub fn initialize(
        &mut self,
        #[env] env: &mut Env<'_>,
        #[input] &parent_hash: &BlockHash,
    ) -> Result<(), ContractError> {
        // Only execution environment can make a direct call here
        if env.caller() != Address::NULL {
            return Err(ContractError::Forbidden);
        }

        *self = Self {
            number: self.number + BlockNumber::ONE,
            parent_hash,
        };

        Ok(())
    }

    #[view]
    pub fn get(&self) -> Self {
        *self
    }
}
