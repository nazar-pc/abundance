//! An extension method of a trait whose `#[output]` hides `'static` in a type alias only accepts
//! memory that lives for `'static`, and the error points to the alias in the trait

use ab_contracts_common::ContractError;
use ab_contracts_common::env::Env;
use ab_contracts_common::output::Lent;
use ab_contracts_macros::contract;
use ab_core_primitives::address::Address;
use ab_io_type::variable_bytes::VariableBytes;
use core::mem::MaybeUninit;

type Bytes = VariableBytes<'static, 8>;

#[contract]
pub trait Reader {
    #[view]
    fn read(#[env] env: &Env<'_>, #[output] output: &mut Bytes) -> Result<(), ContractError>;
}

pub fn call(env: &Env<'_>, contract: Address) {
    let mut buffer = [MaybeUninit::<u8>::uninit(); 8];
    let mut size = 0;
    let mut lent = Lent::<VariableBytes<'_, 8>>::from_uninit(&mut buffer, &mut size).unwrap();
    let _ = env.reader_read(contract, &mut lent);
}

fn main() {}
