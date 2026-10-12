//! Capacity of fixed capacity elements, bytes and strings must fit into `u32`, the type of the
//! length, otherwise the length would silently wrap. The size of the container, which includes the
//! length, must fit into `u32` too, like the size of any `TrivialType`.

use ab_io_type::fixed_capacity_bytes::FixedCapacityBytes;
use ab_io_type::fixed_capacity_elements::FixedCapacityElements;
use ab_io_type::fixed_capacity_string::FixedCapacityString;
use ab_io_type::trivial_type::TrivialType;

fn trivial_type<T: TrivialType>() {}

fn main() {
    // Capacity above `u32::MAX`, the size of zero-sized elements fits
    let _ = FixedCapacityElements::<(), { 1 << 32 }>::try_from_slice(&[]);
    // Capacity fits, but the size doesn't because of the length
    let _ = FixedCapacityBytes::<{ u32::MAX as usize - 3 }>::try_from_slice(&[]);
    let _ = FixedCapacityString::<{ u32::MAX as usize }>::try_from_str("a");
    // Capacity fits, but the size doesn't because of the length extended to the alignment of
    // elements
    let _ = FixedCapacityElements::<u128, { (1 << 28) - 1 }>::default();
    // Capacity fits, but the size of elements doesn't
    trivial_type::<FixedCapacityElements<u16, { 1 << 31 }>>();
}
