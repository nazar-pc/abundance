use crate::fixed_capacity_elements::FixedCapacityElements;

/// Container for storing a number of bytes limited by the specified fixed capacity
pub type FixedCapacityBytes<const CAPACITY: usize> = FixedCapacityElements<u8, CAPACITY>;
