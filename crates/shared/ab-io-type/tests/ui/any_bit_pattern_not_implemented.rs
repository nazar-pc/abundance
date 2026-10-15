//! `AnyBitPattern` is not implemented for types with invalid bit patterns: enums, structs and
//! arrays containing them, and containers whose length must not exceed their capacity

use ab_io_type::fixed_capacity_bytes::FixedCapacityBytes;
use ab_io_type::fixed_capacity_string::FixedCapacityString;
use ab_io_type::trivial_type::{AnyBitPattern, TrivialType};
use ab_io_type::unaligned::Unaligned;

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

fn require<T>()
where
    T: AnyBitPattern,
{
}

fn main() {
    require::<Ctx>();
    require::<WithCtx>();
    require::<[Ctx; 2]>();
    require::<Unaligned<Ctx>>();
    require::<FixedCapacityBytes<4>>();
    require::<FixedCapacityString<4>>();
    require::<WithBytes>();
}
