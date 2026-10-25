//! `self` counts towards `MAX_TOTAL_METHOD_ARGS` in methods that have it, so this method has one
//! argument too many

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
    pub fn many(
        &mut self,
        #[input] _a: &u8,
        #[input] _b: &u8,
        #[input] _c: &u8,
        #[input] _d: &u8,
        #[input] _e: &u8,
        #[input] _f: &u8,
        #[input] _g: &u8,
        #[input] _h: &u8,
    ) {
    }
}

fn main() {}
