#![no_std]

use ab_contracts_common::env::Env;
use ab_contracts_common::{Address, ContractError, ShardIndex};
use ab_contracts_io_type::trivial_type::TrivialType;
use ab_contracts_macros::contract;

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
pub struct AddressAllocator {
    /// Next address to be allocated on this shard
    pub next_address: u64,
    /// Max address to be allocated on this shard
    pub max_address: u64,
}

#[contract]
impl AddressAllocator {
    /// Initialize address allocator for a shard
    #[init]
    pub fn new(#[env] env: &Env) -> Self {
        let shard_index = env.shard_index();
        Self {
            next_address: shard_index.to_u32() as u64 * ShardIndex::MAX_SHARDS as u64,
            max_address: (shard_index.to_u32() as u64 + 1) * ShardIndex::MAX_SHARDS as u64 - 1,
        }
    }

    /// Allocate a new address for a contract.
    ///
    /// This can only be called by [`Address::SYSTEM_CODE`] contract.
    #[update]
    pub fn allocate_address(&mut self, #[env] env: &mut Env) -> Result<Address, ContractError> {
        if env.caller() != &Address::SYSTEM_CODE {
            return Err(ContractError::AccessDenied);
        }

        let next_address = self.next_address;
        if next_address == self.max_address {
            // No more addresses can be allocated on this shard
            return Err(ContractError::InvalidState);
        }

        self.next_address += 1;
        Ok(Address::from(next_address))
    }
}
