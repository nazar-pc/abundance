//! Hashes-related data structures and functions.

use ab_io_type::trivial_type::TrivialType;
use blake3::{Hash, OUT_LEN};
use core::array::TryFromSliceError;
use core::{fmt, mem};
use derive_more::{AsMut, AsRef, Deref, DerefMut, From, Into};
#[cfg(feature = "scale-codec")]
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
#[cfg(feature = "scale-codec")]
use scale_info::TypeInfo;
#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// BLAKE3 hash output transparent wrapper
#[derive(
    Default,
    Copy,
    Clone,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    From,
    Into,
    AsRef,
    AsMut,
    Deref,
    DerefMut,
    TrivialType,
)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
#[repr(C)]
pub struct Blake3Hash([u8; Blake3Hash::SIZE]);

impl fmt::Display for Blake3Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[cfg(feature = "serde")]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct Blake3HashBinary([u8; Blake3Hash::SIZE]);

#[cfg(feature = "serde")]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct Blake3HashHex(#[serde(with = "hex")] [u8; Blake3Hash::SIZE]);

#[cfg(feature = "serde")]
impl Serialize for Blake3Hash {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            Blake3HashHex(self.0).serialize(serializer)
        } else {
            Blake3HashBinary(self.0).serialize(serializer)
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for Blake3Hash {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(if deserializer.is_human_readable() {
            Blake3HashHex::deserialize(deserializer)?.0
        } else {
            Blake3HashBinary::deserialize(deserializer)?.0
        }))
    }
}

impl fmt::Debug for Blake3Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl AsRef<[u8]> for Blake3Hash {
    #[inline(always)]
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl AsMut<[u8]> for Blake3Hash {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

impl From<&[u8; Self::SIZE]> for Blake3Hash {
    #[inline(always)]
    fn from(value: &[u8; Self::SIZE]) -> Self {
        Self(*value)
    }
}

impl TryFrom<&[u8]> for Blake3Hash {
    type Error = TryFromSliceError;

    #[inline(always)]
    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        Ok(Self(value.try_into()?))
    }
}

impl From<Hash> for Blake3Hash {
    #[inline(always)]
    fn from(value: Hash) -> Self {
        Self(value.into())
    }
}

impl Blake3Hash {
    /// Size in bytes
    pub const SIZE: usize = OUT_LEN;

    /// Create new instance
    #[inline(always)]
    pub const fn new(hash: [u8; OUT_LEN]) -> Self {
        Self(hash)
    }

    /// Get internal representation
    #[inline(always)]
    pub const fn as_bytes(&self) -> &[u8; Self::SIZE] {
        &self.0
    }

    /// Convenient conversion from slice of underlying representation for efficiency purposes
    #[inline(always)]
    pub const fn slice_from_repr(value: &[[u8; Self::SIZE]]) -> &[Self] {
        // SAFETY: `Blake3Hash` is `#[repr(C)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion to slice of underlying representation for efficiency purposes
    #[inline(always)]
    pub const fn repr_from_slice(value: &[Self]) -> &[[u8; Self::SIZE]] {
        // SAFETY: `Blake3Hash` is `#[repr(C)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }
}

/// BLAKE3 hashing of a single value.
#[inline(always)]
pub fn blake3_hash(data: &[u8]) -> Blake3Hash {
    Blake3Hash(*blake3::hash(data).as_bytes())
}

/// BLAKE3 hashing of a single value in parallel (only useful for large values well above 128kiB).
#[cfg(feature = "parallel")]
#[inline(always)]
pub fn blake3_hash_parallel(data: &[u8]) -> Blake3Hash {
    let mut state = blake3::Hasher::new();
    state.update_rayon(data);
    Blake3Hash::from(state.finalize())
}

/// BLAKE3 keyed hashing of a single value.
#[inline(always)]
pub fn blake3_hash_with_key(key: &[u8; 32], data: &[u8]) -> Blake3Hash {
    Blake3Hash::from(blake3::keyed_hash(key, data))
}

/// BLAKE3 keyed hashing of a list of values.
#[inline(always)]
pub fn blake3_hash_list_with_key(key: &[u8; 32], data: &[&[u8]]) -> Blake3Hash {
    let mut state = blake3::Hasher::new_keyed(key);
    for d in data {
        state.update(d);
    }
    Blake3Hash::from(state.finalize())
}

/// BLAKE3 hashing of a list of values.
#[inline(always)]
pub fn blake3_hash_list(data: &[&[u8]]) -> Blake3Hash {
    let mut state = blake3::Hasher::new();
    for d in data {
        state.update(d);
    }
    Blake3Hash::from(state.finalize())
}
