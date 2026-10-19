//! A method can only lend its `#[output]` to calls with `Env::lend_output()` if it declares the
//! output with the lifetime of `Env`, rustc suggests the signature, which compiles

use ab_contracts_common::ContractError;
use ab_contracts_common::env::Env;
use ab_contracts_macros::contract;
use ab_io_type::trivial_type::TrivialType;

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct Example {
    pub value: u8,
}

#[contract]
impl Example {
    #[update]
    pub fn forward(#[env] env: &mut Env<'_>, #[output] out: &mut u64) -> Result<(), ContractError> {
        let _out = env.lend_output(out);
        Ok(())
    }

    #[update]
    pub fn forward_suggested<'a>(
        #[env] env: &mut Env<'a>,
        #[output] out: &'a mut u64,
    ) -> Result<(), ContractError> {
        let _out = env.lend_output(out);
        Ok(())
    }
}

fn main() {}
