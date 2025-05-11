#![no_std]

use ab_contracts_common::ContractError;
use ab_contracts_common::env::Env;
use ab_contracts_macros::contract;
use ab_core_primitives::address::Address;
use ab_core_primitives::shard::ShardIndex;
use ab_io_type::trivial_type::TrivialType;

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct AddressAllocator {
    /// Next address to be allocated on this shard
    pub next_address: u128,
    /// Max address to be allocated on this shard
    pub max_address: u128,
}

#[contract]
impl AddressAllocator {
    /// Initialize address allocator for a shard
    #[init]
    pub fn new(#[env] env: &Env<'_>) -> Self {
        let shard_index = env.shard_index();

        let expected_self_address = u128::from(Address::system_address_allocator(shard_index));
        debug_assert_eq!(
            env.own_address(),
            Address::from(expected_self_address),
            "Unexpected allocator address"
        );

        Self {
            next_address: expected_self_address + 1,
            max_address: expected_self_address + (ShardIndex::MAX_ADDRESSES_PER_SHARD.get() - 1),
        }
    }

    /// Allocate a new address for a contract.
    ///
    /// This can only be called by [`Address::SYSTEM_CODE`] contract.
    #[update]
    pub fn allocate_address(&mut self, #[env] env: &mut Env<'_>) -> Result<Address, ContractError> {
        if env.caller() != Address::SYSTEM_CODE {
            return Err(ContractError::Forbidden);
        }

        let next_address = self.next_address;
        if next_address >= self.max_address {
            // No more addresses can be allocated on this shard
            return Err(ContractError::Forbidden);
        }

        self.next_address += 1;
        Ok(Address::from(next_address))
    }
}
