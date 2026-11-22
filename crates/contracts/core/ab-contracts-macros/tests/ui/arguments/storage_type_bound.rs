//! `#[tmp]` and `#[slot]` arguments must implement `IoTypeOptional`, like `MaybeData<T>`, because
//! the host passes empty values to them when nothing was stored yet

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
    pub fn touch(#[tmp] _tmp: &u8, #[slot] _slot: &u8) {}
}

fn main() {}
