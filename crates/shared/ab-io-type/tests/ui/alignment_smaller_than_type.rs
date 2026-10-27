//! `Alignment::of()` doesn't compile for an implementation of `TrivialType` whose `ALIGNMENT` is
//! smaller than the alignment of the type

use ab_io_type::alignment::Alignment;
use ab_io_type::trivial_type::TrivialType;
use ab_io_type::unaligned::Unaligned;

#[derive(Copy, Clone)]
#[repr(C)]
struct Value {
    value: u64,
}

// The alignment of `u8` is smaller than that of `u64`
unsafe impl TrivialType for Value {
    const ALIGNMENT: Alignment = <u8 as TrivialType>::ALIGNMENT;
    const METADATA: &[u8] = <u64 as TrivialType>::METADATA;

    unsafe fn parse_step<const ALIGNED: bool>(_bytes: &[u8]) -> Option<(&Unaligned<Self>, &[u8])> {
        None
    }
}

fn main() {
    let _ = Alignment::of::<Value>();
}
