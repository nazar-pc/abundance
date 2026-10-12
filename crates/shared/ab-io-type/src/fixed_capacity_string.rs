#[cfg(test)]
mod tests;

use crate::fixed_capacity_elements::{FixedCapacityElements, SUPPORTED_CAPACITY};
use crate::metadata::{IoTypeMetadataKind, MAX_METADATA_CAPACITY, concat_metadata_sources};
use crate::trivial_type::TrivialType;
use core::ops::{Deref, DerefMut};

/// Container for storing a UTF-8 string limited by the specified fixed bytes capacity.
///
/// This is a string only by convention, there is no runtime verification done, contents is
/// treated as regular bytes.
///
/// This is just a wrapper for [`FixedCapacityElements`] of bytes that the type dereferences to
/// with a different semantic meaning.
///
/// `CAPACITY` must not exceed `u32::MAX - 4`.
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct FixedCapacityString<const CAPACITY: usize> {
    bytes: FixedCapacityElements<u8, CAPACITY>,
}

impl<const CAPACITY: usize> Default for FixedCapacityString<CAPACITY>
where
    [(); SUPPORTED_CAPACITY::<u8, CAPACITY>]:,
{
    #[inline(always)]
    fn default() -> Self {
        Self {
            bytes: FixedCapacityElements::default(),
        }
    }
}

impl<const CAPACITY: usize> Deref for FixedCapacityString<CAPACITY> {
    type Target = FixedCapacityElements<u8, CAPACITY>;

    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

impl<const CAPACITY: usize> DerefMut for FixedCapacityString<CAPACITY> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.bytes
    }
}

// SAFETY: The only field is `TrivialType`, so the layout is the same and there is no padding
unsafe impl<const CAPACITY: usize> TrivialType for FixedCapacityString<CAPACITY>
where
    [(); SUPPORTED_CAPACITY::<u8, CAPACITY>]:,
{
    // Casting `CAPACITY` to `u32` is lossless, which is checked by the `SUPPORTED_CAPACITY` bound
    const METADATA: &[u8] = {
        #[inline(always)]
        const fn metadata(capacity: u32) -> ([u8; MAX_METADATA_CAPACITY], usize) {
            concat_metadata_sources(&[
                &[IoTypeMetadataKind::FixedCapacityString as u8],
                &capacity.to_le_bytes(),
            ])
        }
        metadata(CAPACITY as u32)
            .0
            .split_at(metadata(CAPACITY as u32).1)
            .0
    };
}

impl<const CAPACITY: usize> FixedCapacityString<CAPACITY>
where
    [(); SUPPORTED_CAPACITY::<u8, CAPACITY>]:,
{
    /// Try to create an instance from provided string.
    ///
    /// Returns `None` if provided string does not fit into the capacity.
    #[inline(always)]
    pub fn try_from_str(s: &str) -> Option<Self> {
        Self::try_from_slice(s.as_bytes())
    }

    /// Try to create an instance from provided bytes.
    ///
    /// Returns `None` if provided bytes do not fit into the capacity.
    #[inline(always)]
    pub fn try_from_slice(bytes: &[u8]) -> Option<Self> {
        Some(Self {
            bytes: FixedCapacityElements::try_from_slice(bytes)?,
        })
    }
}
