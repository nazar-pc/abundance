//! `#[trivial_type(..)]` only supports `validate = path`, specified once

use ab_io_type::trivial_type::TrivialType;

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
#[trivial_type(validates = is_valid)]
struct UnknownOption {
    value: u32,
}

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
#[trivial_type(validate = is_valid)]
#[trivial_type(validate = is_valid)]
struct TwiceValidate {
    value: u32,
}

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
#[trivial_type()]
struct NoOptions {
    value: u32,
}

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
#[trivial_type(validate = is_valid<u32>)]
struct TypeStyleGenerics {
    value: u32,
}

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
#[trivial_type(validate = #[inline] is_valid)]
struct AttributeOnPath {
    value: u32,
}

fn main() {}
