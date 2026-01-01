use crate::metadata::{IoTypeMetadataKind, MAX_METADATA_CAPACITY, concat_metadata_sources};
use crate::trivial_type::TrivialType;
use core::fmt;
#[cfg(feature = "scale-codec")]
use parity_scale_codec::{Decode, Encode, EncodeLike, MaxEncodedLen, Output};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize, Serializer};

/// Wrapper type for `Data` that is the same size, but doesn't need to be aligned/has alignment of
/// one byte.
///
/// This is similar to `#[repr(packed)]`, but makes sure to only expose safe API when dealing with
/// the contents. For example, if `Data` is unaligned and has fields, it is not sounds having
/// references to its fields. This data structure prevents such invalid invariants.
#[derive(Debug, Default, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "scale-codec", derive(Decode, MaxEncodedLen))]
#[cfg_attr(feature = "serde", derive(Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
#[repr(C, packed)]
pub struct Unaligned<Data>(Data)
where
    Data: TrivialType;

impl<Data> From<Data> for Unaligned<Data>
where
    Data: TrivialType,
{
    #[inline(always)]
    fn from(value: Data) -> Self {
        Self(value)
    }
}

impl<Data> fmt::Display for Unaligned<Data>
where
    Data: TrivialType + fmt::Display,
{
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let inner = self.0;
        inner.fmt(f)
    }
}

// SAFETY: Any bit pattern is valid, so it is safe to implement `TrivialType` for this type
unsafe impl<Data> TrivialType for Unaligned<Data>
where
    Data: TrivialType,
{
    const METADATA: &[u8] = {
        const fn metadata(inner_metadata: &[u8]) -> ([u8; MAX_METADATA_CAPACITY], usize) {
            concat_metadata_sources(&[&[IoTypeMetadataKind::Unaligned as u8], inner_metadata])
        }

        // Strange syntax to allow Rust to extend the lifetime of metadata scratch automatically
        metadata(Data::METADATA)
            .0
            .split_at(metadata(Data::METADATA).1)
            .0
    };
}

#[cfg(feature = "scale-codec")]
impl<Data> Encode for Unaligned<Data>
where
    Data: TrivialType + Encode,
{
    #[inline(always)]
    fn size_hint(&self) -> usize {
        let inner = self.0;
        inner.size_hint()
    }

    #[inline(always)]
    fn encode_to<T: Output + ?Sized>(&self, dest: &mut T) {
        let inner = self.0;
        inner.encode_to(dest)
    }

    #[inline(always)]
    fn using_encoded<R, F: FnOnce(&[u8]) -> R>(&self, f: F) -> R {
        let inner = self.0;
        inner.using_encoded(f)
    }
}

#[cfg(feature = "scale-codec")]
impl<Data> EncodeLike for Unaligned<Data> where Data: TrivialType + Encode {}

#[cfg(feature = "serde")]
impl<Data> Serialize for Unaligned<Data>
where
    Data: TrivialType + Serialize,
{
    #[inline(always)]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let inner = self.0;
        inner.serialize(serializer)
    }
}

impl<Data> Unaligned<Data>
where
    Data: TrivialType,
{
    /// Create a new instance
    #[inline(always)]
    pub const fn new(inner: Data) -> Self {
        Self(inner)
    }

    /// Get inner value
    #[inline(always)]
    pub const fn as_inner(&self) -> Data {
        self.0
    }

    /// Replace inner value
    pub const fn replace(&mut self, value: Data) {
        self.0 = value;
    }
}
