//! `#[init]` method must have `Self` as either return type or last `#[output]` argument, which the
//! host stores as the initial state of the contract

use ab_contracts_macros::contract;
use ab_io_type::trivial_type::TrivialType;

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct Example {
    pub value: u8,
}

#[contract]
impl Example {
    #[init]
    pub fn new() -> u8 {
        0
    }
}

fn main() {}
