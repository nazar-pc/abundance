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
pub struct ExampleFt {
    pub total_supply: Balance,
    pub owner: Address,
}

#[contract]
impl Fungible for ExampleFt {
    #[update]
    fn transfer(
        #[env] env: &mut Env<'_>,
        #[input] from: &Address,
        #[input] to: &Address,
        #[input] amount: &Balance,
    ) -> Result<(), ContractError> {
        if !(env.context() == from || env.caller() == from || env.caller() == env.own_address()) {
            return Err(ContractError::Forbidden);
        }

        env.example_ft_transfer(MethodContext::Replace, env.own_address(), from, to, amount)
    }

    #[view]
    fn balance(#[env] env: &Env<'_>, #[input] address: &Address) -> Result<Balance, ContractError> {
        env.example_ft_balance(env.own_address(), address)
    }
}

#[contract]
impl ExampleFt {
    #[init]
    pub fn new(
        #[slot] (owner_addr, owner): (&Address, &mut MaybeData<Slot>),
        #[input] total_supply: &Balance,
    ) -> Self {
        owner.replace(Slot {
            balance: *total_supply,
        });
        Self {
            total_supply: *total_supply,
            owner: *owner_addr,
        }
    }

    #[update]
    pub fn mint(
        &mut self,
        #[env] env: &mut Env<'_>,
        #[slot] to: &mut MaybeData<Slot>,
        #[input] &value: &Balance,
    ) -> Result<(), ContractError> {
        if env.context() != self.owner && env.caller() != self.owner {
            return Err(ContractError::Forbidden);
        }

        if Balance::MAX - value > self.total_supply {
            return Err(ContractError::BadInput);
        }

        self.total_supply += value;
        to.get_mut_or_default().balance += value;

        Ok(())
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
            || env.caller() == env.own_address())
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
