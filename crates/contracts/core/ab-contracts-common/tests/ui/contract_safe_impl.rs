//! `Contract` and `ContractTrait` can only be implemented with `unsafe impl`, since the native
//! executor calls FFI functions from them according to the metadata next to them

use ab_contracts_common::{Contract, ContractTrait, MAX_CODE_SIZE, NativeExecutorContactMethod};
use ab_io_type::trivial_type::TrivialType;
use ab_io_type::variable_bytes::VariableBytes;
use core::ops::Deref;

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
struct Manual {
    value: u8,
}

impl Contract for Manual {
    const MAIN_CONTRACT_METADATA: &[u8] = &[];
    const CODE: &str = "Manual";
    const NATIVE_EXECUTOR_METHODS: &[NativeExecutorContactMethod] = &[];
    type Slot = ();
    type Tmp = ();

    fn code() -> impl Deref<Target = VariableBytes<MAX_CODE_SIZE>> {
        static CODE_SIZE: u32 = 6;

        VariableBytes::from_buffer(Self::CODE.as_bytes(), &CODE_SIZE).unwrap()
    }
}

trait ManualTrait {}

impl ContractTrait<dyn ManualTrait> for Manual {
    const NATIVE_EXECUTOR_METHODS: &[NativeExecutorContactMethod] = &[];
}

fn main() {}
