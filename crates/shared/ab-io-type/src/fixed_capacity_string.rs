use crate::fixed_capacity_bytes::{
    FixedCapacityBytesU8, FixedCapacityBytesU16, SUPPORTED_CAPACITY_U8, SUPPORTED_CAPACITY_U16,
};
use crate::metadata::{IoTypeMetadataKind, MAX_METADATA_CAPACITY, concat_metadata_sources};
use crate::trivial_type::TrivialType;
use core::ops::{Deref, DerefMut};

/// Container for storing a UTF-8 string limited by the specified fixed bytes capacity as `u8`.
///
/// This is a string only by convention, there is no runtime verification done, contents is
/// treated as regular bytes.
///
/// See also [`FixedCapacityStringU16`] if you need to store more bytes.
///
/// This is just a wrapper for [`FixedCapacityBytesU8`] that the type dereferences to with a
/// different semantic meaning.
///
/// `CAPACITY` must not exceed `u8::MAX`.
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct FixedCapacityStringU8<const CAPACITY: usize> {
    bytes: FixedCapacityBytesU8<CAPACITY>,
}

impl<const CAPACITY: usize> Default for FixedCapacityStringU8<CAPACITY>
where
    [(); SUPPORTED_CAPACITY_U8::<CAPACITY>]:,
{
    #[inline(always)]
    fn default() -> Self {
        Self {
            bytes: FixedCapacityBytesU8::default(),
        }
    }
}

impl<const CAPACITY: usize> Deref for FixedCapacityStringU8<CAPACITY> {
    type Target = FixedCapacityBytesU8<CAPACITY>;

    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

impl<const CAPACITY: usize> DerefMut for FixedCapacityStringU8<CAPACITY> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.bytes
    }
}

// SAFETY: Any bit pattern is valid, so it is safe to implement `TrivialType` for this type
unsafe impl<const CAPACITY: usize> TrivialType for FixedCapacityStringU8<CAPACITY>
where
    [(); SUPPORTED_CAPACITY_U8::<CAPACITY>]:,
{
    const METADATA: &[u8] = {
        #[inline(always)]
        const fn metadata(capacity: usize) -> ([u8; MAX_METADATA_CAPACITY], usize) {
            concat_metadata_sources(&[&[
                IoTypeMetadataKind::FixedCapacityString8b as u8,
                capacity as u8,
            ]])
        }
        metadata(CAPACITY).0.split_at(metadata(CAPACITY).1).0
    };
}

impl<const CAPACITY: usize> FixedCapacityStringU8<CAPACITY>
where
    [(); SUPPORTED_CAPACITY_U8::<CAPACITY>]:,
{
    /// Try to create an instance from provided string.
    ///
    /// Returns `None` if provided string does not fit into the capacity.
    #[inline(always)]
    pub fn try_from_str(s: &str) -> Option<Self> {
        Self::try_from_bytes(s.as_bytes())
    }

    /// Try to create an instance from provided bytes.
    ///
    /// Returns `None` if provided bytes do not fit into the capacity.
    #[inline(always)]
    pub fn try_from_bytes(bytes: &[u8]) -> Option<Self> {
        Some(Self {
            bytes: FixedCapacityBytesU8::try_from_bytes(bytes)?,
        })
    }
}

/// Container for storing a UTF-8 string limited by the specified fixed bytes capacity as `u16`.
///
/// This is a string only by convention, there is no runtime verification done, contents is
/// treated as regular bytes.
///
/// See also [`FixedCapacityStringU8`] if you need to store fewer bytes.
///
/// This is just a wrapper for [`FixedCapacityBytesU16`] that the type dereferences to with a
/// different semantic meaning.
///
/// `CAPACITY` must not exceed `u16::MAX` and must be a multiple of 2 to avoid padding.
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct FixedCapacityStringU16<const CAPACITY: usize> {
    bytes: FixedCapacityBytesU16<CAPACITY>,
}

impl<const CAPACITY: usize> Default for FixedCapacityStringU16<CAPACITY>
where
    [(); SUPPORTED_CAPACITY_U16::<CAPACITY>]:,
{
    #[inline(always)]
    fn default() -> Self {
        Self {
            bytes: FixedCapacityBytesU16::default(),
        }
    }
}

impl<const CAPACITY: usize> Deref for FixedCapacityStringU16<CAPACITY> {
    type Target = FixedCapacityBytesU16<CAPACITY>;

    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

impl<const CAPACITY: usize> DerefMut for FixedCapacityStringU16<CAPACITY> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.bytes
    }
}

// SAFETY: Any bit pattern is valid and there is no padding since capacity is a multiple of 2, so it
// is safe to implement `TrivialType` for this type
unsafe impl<const CAPACITY: usize> TrivialType for FixedCapacityStringU16<CAPACITY>
where
    [(); SUPPORTED_CAPACITY_U16::<CAPACITY>]:,
{
    const METADATA: &[u8] = {
        #[inline(always)]
        const fn metadata(capacity: usize) -> ([u8; MAX_METADATA_CAPACITY], usize) {
            concat_metadata_sources(&[
                &[IoTypeMetadataKind::FixedCapacityString16b as u8],
                &(capacity as u16).to_le_bytes(),
            ])
        }
        metadata(CAPACITY).0.split_at(metadata(CAPACITY).1).0
    };
}

impl<const CAPACITY: usize> FixedCapacityStringU16<CAPACITY>
where
    [(); SUPPORTED_CAPACITY_U16::<CAPACITY>]:,
{
    /// Try to create an instance from provided string.
    ///
    /// Returns `None` if provided string does not fit into the capacity.
    #[inline(always)]
    pub fn try_from_str(s: &str) -> Option<Self> {
        Self::try_from_bytes(s.as_bytes())
    }

    /// Try to create an instance from provided bytes.
    ///
    /// Returns `None` if provided bytes do not fit into the capacity.
    #[inline(always)]
    pub fn try_from_bytes(bytes: &[u8]) -> Option<Self> {
        Some(Self {
            bytes: FixedCapacityBytesU16::try_from_bytes(bytes)?,
        })
    }
}
