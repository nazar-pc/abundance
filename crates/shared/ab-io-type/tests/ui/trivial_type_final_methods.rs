//! Provided methods of `TrivialType` can't be overridden, they must behave as documented regardless
//! of the implementation

use ab_io_type::trivial_type::TrivialType;
use ab_io_type::unaligned::Unaligned;

#[derive(Copy, Clone)]
#[repr(C)]
struct Value {
    value: u8,
}

unsafe impl TrivialType for Value {
    const METADATA: &[u8] = &[];

    fn try_from_bytes_unaligned(_bytes: &[u8]) -> Option<(&Unaligned<Self>, &[u8])> {
        None
    }

    fn try_from_bytes(_bytes: &[u8]) -> Option<(&Self, &[u8])> {
        None
    }
}

fn main() {}
