#![no_std]

use ab_contracts_macros::contract;
use ab_io_type::bool::Bool;
use ab_io_type::trivial_type::TrivialType;

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct Flipper {
    pub value: Bool,
}

#[contract]
impl Flipper {
    #[init]
    pub fn new(#[input] &init_value: &Bool) -> Self {
        Self { value: init_value }
    }

    #[update]
    pub fn flip(&mut self) {
        self.value = !self.value;
    }

    #[view]
    pub fn value(&self) -> Bool {
        self.value
    }
}
