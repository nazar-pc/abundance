//! `#[trivial_type(..)]` is only supported on the type, on fields and variants it would be ignored

use ab_io_type::trivial_type::TrivialType;

const fn is_valid<T>(_value: &T) -> bool
where
    T: Copy,
{
    true
}

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
struct OnField {
    #[trivial_type(validate = is_valid)]
    value: u32,
}

#[derive(Copy, Clone, TrivialType)]
#[repr(u8)]
enum OnVariant {
    #[trivial_type(validate = is_valid)]
    A(u8),
    B(u8),
}

#[derive(Copy, Clone, TrivialType)]
#[repr(u8)]
enum OnVariantField {
    A(#[trivial_type(validate = is_valid)] u8),
    B(u8),
}

fn main() {}
