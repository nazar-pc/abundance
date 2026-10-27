//! Generic code bounded by `TrivialType` alone is told to require `AnyBitPattern`, not
//! `AnyBitPatternMarker`

use ab_io_type::trivial_type::{AnyBitPattern, TrivialType};

fn require<T>()
where
    T: AnyBitPattern,
{
}

fn require_in_generic_code<T>()
where
    T: TrivialType,
{
    require::<T>();
}

fn main() {
    require_in_generic_code::<u8>();
}
