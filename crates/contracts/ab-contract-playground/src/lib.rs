#![no_std]

use ab_contracts_common::env::{Env, MethodContext};
use ab_contracts_common::{Address, Balance, ContractError};
use ab_contracts_io_type::maybe_data::MaybeData;
use ab_contracts_io_type::trivial_type::TrivialType;
use ab_contracts_io_type::variable_bytes::VariableBytes;
use ab_contracts_macros::contract;
use ab_contracts_standards::Fungible;
use core::cmp::Ordering;

#[derive(Debug, Default, Copy, Clone, TrivialType)]
#[repr(u8)]
pub enum LastAction {
    #[default]
    None,
    Mint,
    Transfer,
}

#[derive(Debug, Default, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct Slot {
    pub balance: Balance,
}

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
pub struct Playground {
    pub total_supply: Balance,
    pub owner: Address,
}

#[contract]
impl Fungible for Playground {
    #[update]
    fn transfer(
        #[env] env: &mut Env,
        #[input] from: &Address,
        #[input] to: &Address,
        #[input] amount: &Balance,
    ) -> Result<(), ContractError> {
        if !(env.context() == from || env.caller() == from || env.caller() == env.own_address()) {
            return Err(ContractError::Forbidden);
        }

        env.playground_transfer(MethodContext::Replace, env.own_address(), from, to, amount)
    }

    #[view]
    fn balance(#[env] env: &Env, #[input] address: &Address) -> Result<Balance, ContractError> {
        env.playground_balance(env.own_address(), address)
    }
}

#[contract]
impl Playground {
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

    #[init]
    pub fn new_result(#[env] env: &mut Env, #[result] result: &mut MaybeData<Self>) {
        result.replace(Self {
            total_supply: Balance::MIN,
            owner: env.context(),
        });
    }

    #[update]
    pub fn mint(
        &mut self,
        #[env] env: &mut Env,
        #[tmp] last_action: &mut MaybeData<LastAction>,
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

        match to.get_mut() {
            Some(contents) => {
                contents.balance += value;
            }
            None => {
                to.replace(Slot { balance: value });
            }
        }

        last_action.replace(LastAction::Mint);

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
        #[tmp] last_action: &mut MaybeData<LastAction>,
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

        match to.get_mut() {
            Some(contents) => {
                contents.balance += amount;
            }
            None => {
                to.replace(Slot { balance: amount });
            }
        }

        last_action.replace(LastAction::Transfer);

        Ok(())
    }

    #[update]
    pub fn last_action(#[tmp] maybe_last_action: &MaybeData<LastAction>) -> LastAction {
        maybe_last_action.get().copied().unwrap_or_default()
    }
}
