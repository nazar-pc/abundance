#![no_std]

use ab_contracts_macros::contract;
use ab_io_type::trivial_type::TrivialType;

#[derive(Debug, Copy, Clone, TrivialType)]
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
