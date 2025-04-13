#![no_std]

use ab_contracts_common::env::{Env, MethodContext};
use ab_contracts_common::{Address, Balance, ContractError};
use ab_contracts_io_type::maybe_data::MaybeData;
use ab_contracts_io_type::trivial_type::TrivialType;
use ab_contracts_macros::contract;
use ab_contracts_standards::fungible::Fungible;
use core::cmp::Ordering;

#[derive(Debug, Default, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct Slot {
    pub balance: Balance,
}

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct NativeToken {}

#[contract]
impl Fungible for NativeToken {
    #[update]
    fn transfer(
        #[env] env: &mut Env<'_>,
        #[input] from: &Address,
        #[input] to: &Address,
        #[input] amount: &Balance,
    ) -> Result<(), ContractError> {
        if !(env.context() == from
            || env.caller() == from
            || env.caller() == env.own_address()
            || env.caller() == Address::NULL)
        {
            return Err(ContractError::Forbidden);
        }

        env.native_token_transfer(MethodContext::Replace, env.own_address(), from, to, amount)
    }

    #[view]
    fn balance(#[env] env: &Env<'_>, #[input] address: &Address) -> Result<Balance, ContractError> {
        env.native_token_balance(env.own_address(), address)
    }
}

#[contract]
impl NativeToken {
    /// Initialize native token on a shard with max issuance allowed by this shard.
    ///
    /// Block rewards will be implemented using transfers from native token's balance.
    #[update]
    pub fn initialize(
        #[env] env: &mut Env<'_>,
        #[slot] (own_address, own_balance): (&Address, &mut MaybeData<Slot>),
        #[input] &max_issuance: &Balance,
    ) -> Result<Self, ContractError> {
        // Only execution environment can make a direct call here
        if env.caller() != Address::NULL {
            return Err(ContractError::Forbidden);
        }

        if own_address != env.own_address() {
            return Err(ContractError::BadInput);
        }

        if own_balance.get().is_some() {
            return Err(ContractError::Conflict);
        }

        own_balance.replace(Slot {
            balance: max_issuance,
        });

        Ok(Self {})
    }

    #[view]
    pub fn balance(#[slot] target: &MaybeData<Slot>) -> Balance {
        target
            .get()
            .map_or_else(Balance::default, |slot| slot.balance)
    }

    #[update]
    pub fn transfer(
        #[env] env: &mut Env<'_>,
        #[slot] (from_address, from): (&Address, &mut MaybeData<Slot>),
        #[slot] to: &mut MaybeData<Slot>,
        #[input] &amount: &Balance,
    ) -> Result<(), ContractError> {
        if !(env.context() == from_address
            || env.caller() == from_address
            || env.caller() == env.own_address()
            || env.caller() == Address::NULL)
        {
            return Err(ContractError::Forbidden);
        }

        {
            let Some(contents) = from.get_mut() else {
                return Err(ContractError::BadInput);
            };

            match contents.balance.cmp(&amount) {
                Ordering::Less => {
                    return Err(ContractError::BadInput);
                }
                Ordering::Equal => {
                    // All balance is transferred out, remove slot contents
                    from.remove();
                }
                Ordering::Greater => {
                    contents.balance -= amount;
                }
            }
        }

        to.get_mut_or_default().balance += amount;

        Ok(())
    }
}
