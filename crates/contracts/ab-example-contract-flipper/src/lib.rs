#![no_std]

use ab_contracts_io_type::trivial_type::TrivialType;
use ab_contracts_macros::contract;

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
pub struct Flipper {
    pub value: bool,
}

#[contract]
impl Flipper {
    #[init]
    pub fn new(#[input] &init_value: &bool) -> Self {
        Self { value: init_value }
    }

    #[update]
    pub fn flip(&mut self) {
        self.value = !self.value;
    }

    #[view]
    pub fn value(&self) -> bool {
        self.value
    }
}
