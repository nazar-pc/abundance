//! `IoTypeOptional` can't be implemented without `unsafe`, a `TrivialType` like the one below would
//! expose uninitialized memory when its size is `0`.

use ab_io_type::IoTypeOptional;
use ab_io_type::trivial_type::TrivialType;

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
struct Value {
    value: u64,
}

impl IoTypeOptional for Value {}

fn main() {}
