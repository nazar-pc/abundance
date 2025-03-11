use ab_contracts_common::env::Env;
use ab_contracts_common::{Address, Balance, ContractError};
use ab_contracts_macros::contract;

/// Fungible token trait prototype
#[contract]
pub trait Fungible {
    /// Transfer some `amount` of tokens `from` one contract `to` another
    #[update]
    fn transfer(
        #[env] env: &mut Env<'_>,
        #[input] from: &Address,
        #[input] to: &Address,
        #[input] amount: &Balance,
    ) -> Result<(), ContractError>;

    /// Get balance of specified address
    #[view]
    fn balance(#[env] env: &Env<'_>, #[input] address: &Address) -> Result<Balance, ContractError>;
}
