//! Opaque helpers shared by AMO-style extensions (`Zaamo`, `Zabha`, `Zacas`)

use crate::{BasicInt, ExecutionError, PackedAddress, VirtualMemory};
use core::hint::cold_path;

/// Read memory for the read half of an AMO instruction's atomic read-modify-write cycle.
///
/// An AMO's memory access is a single atomic operation; per spec, a fault on it - whether it
/// surfaces during the read half or the write half - must be reported as a Store/AMO fault, never
/// a Load fault. [`VirtualMemory::read`] alone can't tell an AMO's read from an ordinary load's,
/// so this maps any error onto [`ExecutionError::OutOfBoundsWrite`] before it can be misclassified
/// as a load fault by the caller.
#[inline(always)]
#[doc(hidden)]
pub const fn amo_read<T, M, Address>(memory: &M, address: u64) -> Result<T, ExecutionError<Address>>
where
    T: BasicInt,
    M: [const] VirtualMemory,
    Address: Copy,
{
    if let Ok(value) = memory.read::<T>(address) {
        Ok(value)
    } else {
        cold_path();
        Err(ExecutionError::OutOfBoundsWrite {
            address: PackedAddress::new(address),
        })
    }
}
