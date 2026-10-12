//! `TrivialType` derive must reject structs that are not exactly `#[repr(C)]` or
//! `#[repr(transparent)]`

use ab_io_type::trivial_type::TrivialType;

#[derive(Copy, Clone, TrivialType)]
#[repr(C, align(8))]
struct Aligned {
    value: u64,
}

#[derive(Copy, Clone, TrivialType)]
#[repr(C, packed)]
struct Packed {
    value: u64,
}

#[derive(Copy, Clone, TrivialType)]
#[repr(Rust)]
struct Rust {
    value: u64,
}

// `#[repr(u8)]` is not valid for structs either, rustc rejects it too
#[derive(Copy, Clone, TrivialType)]
#[repr(u8)]
struct Numeric {
    value: u8,
}

fn main() {}
