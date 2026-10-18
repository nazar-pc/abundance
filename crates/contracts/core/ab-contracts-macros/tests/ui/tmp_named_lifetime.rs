//! `#[tmp]` is trusted without being parsed, so its lifetime can't be named, which would allow
//! lending it to calls with `Env::lend_output()`

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
    pub fn lend_tmp<'a>(
        #[env] env: &mut ab_contracts_common::env::Env<'a>,
        #[tmp] tmp: &'a mut ab_io_type::maybe_data::MaybeData<u8>,
    ) {
        let _tmp = env.lend_output(tmp);
    }
}

fn main() {}
