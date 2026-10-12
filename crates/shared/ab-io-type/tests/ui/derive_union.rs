//! `TrivialType` derive must reject unions, it is unknown which field is initialized

use ab_io_type::trivial_type::TrivialType;

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
union Union {
    a: u8,
    b: u16,
}

fn main() {}
