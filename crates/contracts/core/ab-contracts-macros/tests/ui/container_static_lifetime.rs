//! Generated code gives containers in arguments a lifetime that only covers the call, so a method
//! can't declare them with `'static` and keep them after the call

use ab_contracts_macros::contract;
use ab_io_type::trivial_type::TrivialType;
use ab_io_type::variable_bytes::VariableBytes;

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct Example {
    pub value: u8,
}

#[contract]
impl Example {
    #[view]
    pub fn keep_input(#[input] _input: &VariableBytes<'static, 8>) {}

    #[view]
    pub fn keep_output(#[output] _output: &mut VariableBytes<'static, 8>) {}
}

fn main() {}
