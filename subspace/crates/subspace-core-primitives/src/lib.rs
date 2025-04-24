//! Core primitives for Subspace Network.

#![no_std]
#![warn(rust_2018_idioms, missing_debug_implementations, missing_docs)]
#![feature(array_chunks, const_trait_impl, const_try, portable_simd, step_trait)]
#![cfg_attr(feature = "alloc", feature(new_zeroed_alloc))]
#![expect(incomplete_features, reason = "generic_const_exprs")]
// TODO: This feature is not actually used in this crate, but is added as a workaround for
//  https://github.com/rust-lang/rust/issues/133199
#![feature(generic_const_exprs)]

#[cfg(feature = "scale-codec")]
pub mod checksum;
pub mod hashes;
pub mod objects;
pub mod pieces;
pub mod pos;
pub mod pot;
pub mod sectors;
pub mod segments;
pub mod solutions;

#[cfg(feature = "alloc")]
extern crate alloc;

use crate::hashes::{blake3_hash_list, Blake3Hash};
use core::fmt;
use derive_more::{AsMut, AsRef, Deref, From, Into};
#[cfg(feature = "scale-codec")]
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
#[cfg(feature = "scale-codec")]
use scale_info::TypeInfo;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "serde")]
use serde::{Deserializer, Serializer};
use static_assertions::const_assert;

// Refuse to compile on lower than 32-bit platforms
const_assert!(size_of::<usize>() >= size_of::<u32>());

/// Type of randomness.
#[derive(Default, Copy, Clone, Eq, PartialEq, From, Into, Deref)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
pub struct Randomness([u8; Randomness::SIZE]);

impl fmt::Debug for Randomness {
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
struct RandomnessBinary([u8; Randomness::SIZE]);

#[cfg(feature = "serde")]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct RandomnessHex(#[serde(with = "hex")] [u8; Randomness::SIZE]);

#[cfg(feature = "serde")]
impl Serialize for Randomness {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            RandomnessHex(self.0).serialize(serializer)
        } else {
            RandomnessBinary(self.0).serialize(serializer)
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for Randomness {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(if deserializer.is_human_readable() {
            RandomnessHex::deserialize(deserializer)?.0
        } else {
            RandomnessBinary::deserialize(deserializer)?.0
        }))
    }
}

impl AsRef<[u8]> for Randomness {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl AsMut<[u8]> for Randomness {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

impl Randomness {
    /// Size of randomness (in bytes).
    pub const SIZE: usize = 32;

    /// Derive global slot challenge from global randomness.
    // TODO: Separate type for global challenge
    pub fn derive_global_challenge(&self, slot: SlotNumber) -> Blake3Hash {
        blake3_hash_list(&[&self.0, &slot.to_le_bytes()])
    }
}

/// Block number in Subspace network.
pub type BlockNumber = u32;

/// Block hash in Subspace network.
pub type BlockHash = [u8; 32];

/// Slot number in Subspace network.
pub type SlotNumber = u64;

/// BlockWeight type for fork choice rules.
///
/// The closer solution's tag is to the target, the heavier it is.
pub type BlockWeight = u128;
