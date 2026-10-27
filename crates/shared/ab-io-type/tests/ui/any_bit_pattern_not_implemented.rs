//! `AnyBitPattern` is not implemented for types with invalid bit patterns: `bool`, enums, types
//! with a `validate` function, structs and arrays containing them, and containers whose length
//! must not exceed their capacity

use ab_io_type::fixed_capacity_bytes::FixedCapacityBytes;
use ab_io_type::fixed_capacity_string::FixedCapacityString;
use ab_io_type::trivial_type::{AnyBitPattern, TrivialType};
use ab_io_type::unaligned::Unaligned;

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
struct WithBool {
    value: u8,
    flag: bool,
}

#[derive(Copy, Clone, TrivialType)]
#[repr(u8)]
enum Ctx {
    A,
    B,
}

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
struct WithCtx {
    value: u8,
    ctx: Ctx,
}

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
struct WithBytes {
    value: u8,
    bytes: FixedCapacityBytes<4>,
}

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
#[trivial_type(validate = Self::is_valid)]
struct NonMax {
    value: u32,
}

impl NonMax {
    const fn is_valid(&self) -> bool {
        self.value != u32::MAX
    }
}

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
struct WithNonMax {
    non_max: NonMax,
}

fn require<T>()
where
    T: AnyBitPattern,
{
}

fn main() {
    require::<bool>();
    require::<[bool; 2]>();
    require::<Unaligned<bool>>();
    require::<WithBool>();
    require::<Ctx>();
    require::<WithCtx>();
    require::<[Ctx; 2]>();
    require::<Unaligned<Ctx>>();
    require::<NonMax>();
    require::<WithNonMax>();
    require::<FixedCapacityBytes<4>>();
    require::<FixedCapacityString<4>>();
    require::<WithBytes>();
}
