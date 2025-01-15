#![no_std]

use ab_contracts_common::env::Env;
use ab_contracts_common::{Address, Balance, ContractError};
use ab_contracts_io_type::maybe_data::MaybeData;
use ab_contracts_io_type::trivial_type::TrivialType;
use ab_contracts_io_type::variable_bytes::VariableBytes;
use ab_contracts_macros::contract_impl;
use core::cmp::Ordering;

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(u8)]
pub enum TestEnum {
    A(u8),
    B { f: u8 },
}

#[derive(Debug, Default, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct Slot {
    pub balance: Balance,
}

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
pub struct ExampleContract {
    pub total_supply: Balance,
    pub owner: Address,
    pub padding: [u8; 8],
}

#[contract_impl]
impl ExampleContract {
    #[init]
    pub fn new(#[env] env: &mut Env, #[input] total_supply: &Balance) -> Self {
        Self {
            total_supply: *total_supply,
            owner: *env.context(),
            padding: [0; 8],
        }
    }

    #[init]
    pub fn new_result(
        #[env] env: &mut Env,
        #[input] total_supply: &Balance,
        #[result] result: &mut MaybeData<Self>,
    ) {
        result.replace(Self {
            total_supply: *total_supply,
            owner: *env.context(),
            padding: [0; 8],
        });
    }

    #[update]
    pub fn mint(
        &mut self,
        #[env] env: &mut Env,
        #[slot] to: &mut MaybeData<Slot>,
        #[input] &value: &Balance,
    ) -> Result<(), ContractError> {
        if env.context() != &self.owner && env.caller() != &self.owner {
            return Err(ContractError::AccessDenied);
        }

        if Balance::MAX - value > self.total_supply {
            return Err(ContractError::InvalidInput);
        }

        self.total_supply += value;

        match to.get_mut() {
            Some(contents) => {
                contents.balance += value;
            }
            None => {
                to.replace(Slot { balance: value });
            }
        }

        Ok(())
    }

    #[view]
    pub fn balance(#[slot] target: &MaybeData<Slot>) -> Balance {
        target
            .get()
            .map_or_else(Balance::default, |slot| slot.balance)
    }

    #[view]
    pub fn balance2(#[slot] target: &MaybeData<Slot>, #[output] balance: &mut MaybeData<Balance>) {
        balance.replace(
            target
                .get()
                .map_or_else(Balance::default, |slot| slot.balance),
        );
    }

    #[view]
    pub fn balance3(#[slot] target: &MaybeData<Slot>, #[result] result: &mut MaybeData<Balance>) {
        result.replace(
            target
                .get()
                .map_or_else(Balance::default, |slot| slot.balance),
        );
    }

    #[view]
    pub fn var_bytes(#[output] _out: &mut VariableBytes<1024>) {
        // TODO
    }

    #[update]
    pub fn transfer(
        #[env] env: &mut Env,
        #[slot] (from_address, from): (&Address, &mut MaybeData<Slot>),
        #[slot] to: &mut MaybeData<Slot>,
        #[input] &value: &Balance,
    ) -> Result<(), ContractError> {
        if env.context() != from_address && env.caller() != from_address {
            return Err(ContractError::AccessDenied);
        }

        {
            let Some(contents) = from.get_mut() else {
                return Err(ContractError::InvalidState);
            };

            match contents.balance.cmp(&value) {
                Ordering::Less => {
                    return Err(ContractError::InvalidInput);
                }
                Ordering::Equal => {
                    // All balance is transferred out, remove slot contents
                    from.remove();
                }
                Ordering::Greater => {
                    contents.balance -= value;
                }
            }
        }

        match to.get_mut() {
            Some(contents) => {
                contents.balance += value;
            }
            None => {
                to.replace(Slot { balance: value });
            }
        }

        Ok(())
    }
}
