//! A method gets `Env` with a lifetime that only covers its call, so it can't declare `Env` with
//! `'static` and keep it after the call

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
    #[view]
    pub fn keep_env(#[env] env: &'static Env<'static>) {
        let _env = env;
    }
}

fn main() {}
