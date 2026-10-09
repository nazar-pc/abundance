//! `#[input]` argument must be a shared reference, inputs are read-only and changes to them are
//! never propagated back to the caller

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
    pub fn set(#[input] _value: &mut u8) {}
}

fn main() {}
