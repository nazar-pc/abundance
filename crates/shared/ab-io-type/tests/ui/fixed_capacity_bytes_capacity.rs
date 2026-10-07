//! Capacity of fixed capacity bytes and strings must fit into their length type, otherwise the
//! length would silently wrap. Capacity of `u16` versions must also be even, otherwise there is a
//! padding byte that `TrivialType` would expose as bytes.

use ab_io_type::fixed_capacity_bytes::{FixedCapacityBytesU8, FixedCapacityBytesU16};
use ab_io_type::fixed_capacity_string::{FixedCapacityStringU8, FixedCapacityStringU16};

fn main() {
    // Capacity above `u8::MAX`
    let _ = FixedCapacityBytesU8::<256>::try_from_bytes(&[1; 256]);
    let _ = FixedCapacityStringU8::<257>::try_from_str("a");
    // Capacity above `u16::MAX`
    let _ = FixedCapacityBytesU16::<65536>::try_from_bytes(&[1; 65536]);
    let _ = FixedCapacityStringU16::<65538>::try_from_str("a");
    // Odd capacity
    let _ = FixedCapacityBytesU16::<1>::try_from_bytes(&[1]);
    let _ = FixedCapacityStringU16::<3>::try_from_str("a");
}
