use crate::fixed_capacity_bytes::{FixedCapacityBytesU8, FixedCapacityBytesU16};
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
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct FixedCapacityStringU8<const CAPACITY: usize> {
    bytes: FixedCapacityBytesU8<CAPACITY>,
}

impl<const CAPACITY: usize> Default for FixedCapacityStringU8<CAPACITY> {
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

unsafe impl<const CAPACITY: usize> TrivialType for FixedCapacityStringU8<CAPACITY> {
    const METADATA: &[u8] = {
        #[inline(always)]
        const fn metadata(capacity: usize) -> ([u8; MAX_METADATA_CAPACITY], usize) {
            assert!(
                capacity <= u8::MAX as usize,
                "`FixedCapacityStringU8` capacity must not exceed `u8::MAX`"
            );
            concat_metadata_sources(&[&[
                IoTypeMetadataKind::FixedCapacityString8b as u8,
                capacity as u8,
            ]])
        }
        metadata(CAPACITY).0.split_at(metadata(CAPACITY).1).0
    };
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
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct FixedCapacityStringU16<const CAPACITY: usize> {
    bytes: FixedCapacityBytesU16<CAPACITY>,
}

impl<const CAPACITY: usize> Default for FixedCapacityStringU16<CAPACITY> {
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

unsafe impl<const CAPACITY: usize> TrivialType for FixedCapacityStringU16<CAPACITY> {
    const METADATA: &[u8] = {
        #[inline(always)]
        const fn metadata(capacity: usize) -> ([u8; MAX_METADATA_CAPACITY], usize) {
            assert!(
                capacity <= u16::MAX as usize,
                "`FixedCapacityStringU16` capacity must not exceed `u16::MAX`"
            );
            concat_metadata_sources(&[&[
                IoTypeMetadataKind::FixedCapacityString16b as u8,
                capacity as u8,
            ]])
        }
        metadata(CAPACITY).0.split_at(metadata(CAPACITY).1).0
    };
}
