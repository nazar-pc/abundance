//! The last `#[output]` of `#[init]` without a return value is the initial state, which is trusted
//! without being parsed, so its lifetime can't be named, which would allow lending it to calls with
//! `Env::lend_output()`

use ab_contracts_macros::contract;
#[expect(unused_imports, reason = "Only used by the method, which is rejected")]
use ab_io_type::maybe_data::MaybeData;
use ab_io_type::trivial_type::TrivialType;

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct Example {
    pub value: u8,
}

#[contract]
impl Example {
    #[init]
    pub fn new<'a>(
        #[env] env: &mut ab_contracts_common::env::Env<'a>,
        #[output] state: &'a mut MaybeData<'_, Self>,
    ) {
        let _state = env.lend_output(state);
    }
}

fn main() {}
