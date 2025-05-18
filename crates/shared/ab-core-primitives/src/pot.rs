//! Proof of time-related data structures.

use crate::hashes::{Blake3Hash, blake3_hash, blake3_hash_list};
use crate::pieces::RecordChunk;
use ab_io_type::trivial_type::TrivialType;
use core::iter::Step;
use core::num::{NonZeroU8, NonZeroU32};
use core::str::FromStr;
use core::time::Duration;
use core::{fmt, mem};
use derive_more::{
    Add, AddAssign, AsMut, AsRef, Deref, DerefMut, Display, Div, DivAssign, From, Into, Mul,
    MulAssign, Sub, SubAssign,
};
#[cfg(feature = "scale-codec")]
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
#[cfg(feature = "scale-codec")]
use scale_info::TypeInfo;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "serde")]
use serde::{Deserializer, Serializer};

/// Slot duration
#[derive(
    Debug, Display, Default, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, From, Into,
)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(C)]
pub struct SlotDuration(u16);

impl SlotDuration {
    /// Size in bytes
    pub const SIZE: usize = size_of::<u16>();

    /// Create new instance
    #[inline(always)]
    pub const fn from_millis(n: u16) -> Self {
        Self(n)
    }

    /// Get internal representation
    #[inline(always)]
    pub const fn as_millis(self) -> u16 {
        self.0
    }

    /// Get the value as [`Duration`] instance
    #[inline(always)]
    pub const fn as_duration(self) -> Duration {
        Duration::from_millis(self.as_millis() as u64)
    }
}

/// Slot number
#[derive(
    Debug,
    Display,
    Default,
    Copy,
    Clone,
    Ord,
    PartialOrd,
    Eq,
    PartialEq,
    Hash,
    From,
    Into,
    Add,
    AddAssign,
    Sub,
    SubAssign,
    Mul,
    MulAssign,
    Div,
    DivAssign,
    TrivialType,
)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(C)]
pub struct SlotNumber(u64);

impl Step for SlotNumber {
    #[inline(always)]
    fn steps_between(start: &Self, end: &Self) -> (usize, Option<usize>) {
        u64::steps_between(&start.0, &end.0)
    }

    #[inline(always)]
    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        u64::forward_checked(start.0, count).map(Self)
    }

    #[inline(always)]
    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        u64::backward_checked(start.0, count).map(Self)
    }
}

impl From<SlotNumber> for u128 {
    #[inline(always)]
    fn from(original: SlotNumber) -> Self {
        u128::from(original.0)
    }
}

impl SlotNumber {
    /// Size in bytes
    pub const SIZE: usize = size_of::<u64>();
    /// Slot 0
    pub const ZERO: Self = Self(0);
    /// Slot 1
    pub const ONE: Self = Self(1);
    /// Max slot
    pub const MAX: Self = Self(u64::MAX);

    /// Create new instance
    #[inline(always)]
    pub const fn new(n: u64) -> Self {
        Self(n)
    }

    /// Get internal representation
    #[inline(always)]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// Create slot number from bytes
    #[inline(always)]
    pub const fn from_bytes(bytes: [u8; Self::SIZE]) -> Self {
        Self(u64::from_le_bytes(bytes))
    }

    /// Convert slot number to bytes
    #[inline(always)]
    pub const fn to_bytes(self) -> [u8; Self::SIZE] {
        self.0.to_le_bytes()
    }

    /// Checked integer addition. Computes `self + rhs`, returning `None` if overflow occurred
    #[inline]
    pub fn checked_add(self, rhs: Self) -> Option<Self> {
        self.0.checked_add(rhs.0).map(Self)
    }

    /// Checked integer subtraction. Computes `self - rhs`, returning `None` if overflow occurred
    #[inline]
    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        self.0.checked_sub(rhs.0).map(Self)
    }
}

/// Proof of time key(input to the encryption).
#[derive(Default, Copy, Clone, Eq, PartialEq, From, Into, AsRef, AsMut, Deref, DerefMut)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
pub struct PotKey([u8; PotKey::SIZE]);

impl fmt::Debug for PotKey {
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
struct PotKeyBinary([u8; PotKey::SIZE]);

#[cfg(feature = "serde")]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct PotKeyHex(#[serde(with = "hex")] [u8; PotKey::SIZE]);

#[cfg(feature = "serde")]
impl Serialize for PotKey {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            PotKeyHex(self.0).serialize(serializer)
        } else {
            PotKeyBinary(self.0).serialize(serializer)
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for PotKey {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(if deserializer.is_human_readable() {
            PotKeyHex::deserialize(deserializer)?.0
        } else {
            PotKeyBinary::deserialize(deserializer)?.0
        }))
    }
}

impl fmt::Display for PotKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl FromStr for PotKey {
    type Err = hex::FromHexError;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut key = Self::default();
        hex::decode_to_slice(s, key.as_mut())?;

        Ok(key)
    }
}

impl PotKey {
    /// Size in bytes
    pub const SIZE: usize = 16;
}

/// Proof of time seed
#[derive(Default, Copy, Clone, Eq, PartialEq, Hash, From, Into, AsRef, AsMut, Deref, DerefMut)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
pub struct PotSeed([u8; PotSeed::SIZE]);

impl fmt::Debug for PotSeed {
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
struct PotSeedBinary([u8; PotSeed::SIZE]);

#[cfg(feature = "serde")]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct PotSeedHex(#[serde(with = "hex")] [u8; PotSeed::SIZE]);

#[cfg(feature = "serde")]
impl Serialize for PotSeed {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            PotSeedHex(self.0).serialize(serializer)
        } else {
            PotSeedBinary(self.0).serialize(serializer)
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for PotSeed {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(if deserializer.is_human_readable() {
            PotSeedHex::deserialize(deserializer)?.0
        } else {
            PotSeedBinary::deserialize(deserializer)?.0
        }))
    }
}

impl fmt::Display for PotSeed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl PotSeed {
    /// Size in bytes
    pub const SIZE: usize = 16;

    /// Derive initial PoT seed from genesis block hash
    #[inline]
    pub fn from_genesis(genesis_block_hash: &[u8], external_entropy: &[u8]) -> Self {
        let hash = blake3_hash_list(&[genesis_block_hash, external_entropy]);
        let mut seed = Self::default();
        seed.copy_from_slice(&hash[..Self::SIZE]);
        seed
    }

    /// Derive key from proof of time seed
    #[inline]
    pub fn key(&self) -> PotKey {
        let mut key = PotKey::default();
        key.copy_from_slice(&blake3_hash(&self.0)[..Self::SIZE]);
        key
    }
}

/// Proof of time output, can be intermediate checkpoint or final slot output
#[derive(
    Default,
    Copy,
    Clone,
    Eq,
    PartialEq,
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
pub struct PotOutput([u8; PotOutput::SIZE]);

impl fmt::Debug for PotOutput {
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
struct PotOutputBinary([u8; PotOutput::SIZE]);

#[cfg(feature = "serde")]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct PotOutputHex(#[serde(with = "hex")] [u8; PotOutput::SIZE]);

#[cfg(feature = "serde")]
impl Serialize for PotOutput {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            PotOutputHex(self.0).serialize(serializer)
        } else {
            PotOutputBinary(self.0).serialize(serializer)
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for PotOutput {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(if deserializer.is_human_readable() {
            PotOutputHex::deserialize(deserializer)?.0
        } else {
            PotOutputBinary::deserialize(deserializer)?.0
        }))
    }
}

impl fmt::Display for PotOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl PotOutput {
    /// Size in bytes
    pub const SIZE: usize = 16;

    /// Derives the global challenge from the output and slot
    #[inline]
    pub fn derive_global_challenge(&self, slot: SlotNumber) -> Blake3Hash {
        blake3_hash_list(&[&self.0, &slot.to_bytes()])
    }

    /// Derive seed from proof of time in case entropy injection is not needed
    #[inline]
    pub fn seed(&self) -> PotSeed {
        PotSeed(self.0)
    }

    /// Derive seed from proof of time with entropy injection
    #[inline]
    pub fn seed_with_entropy(&self, entropy: &Blake3Hash) -> PotSeed {
        let hash = blake3_hash_list(&[entropy.as_ref(), &self.0]);
        let mut seed = PotSeed::default();
        seed.copy_from_slice(&hash[..Self::SIZE]);
        seed
    }

    /// Derive proof of time entropy from chunk and proof of time for injection purposes
    #[inline]
    pub fn derive_pot_entropy(&self, solution_chunk: &RecordChunk) -> Blake3Hash {
        blake3_hash_list(&[solution_chunk.as_ref(), &self.0])
    }
}

/// Proof of time checkpoints, result of proving
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, Hash, Deref, DerefMut, TrivialType)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
#[repr(C)]
pub struct PotCheckpoints([PotOutput; PotCheckpoints::NUM_CHECKPOINTS.get() as usize]);

impl PotCheckpoints {
    /// Size in bytes
    pub const SIZE: usize = PotOutput::SIZE * Self::NUM_CHECKPOINTS.get() as usize;
    /// Number of PoT checkpoints produced (used to optimize verification)
    pub const NUM_CHECKPOINTS: NonZeroU8 = NonZeroU8::new(8).expect("Not zero; qed");

    /// Get proof of time output out of checkpoints (last checkpoint)
    #[inline]
    pub fn output(&self) -> PotOutput {
        self.0[Self::NUM_CHECKPOINTS.get() as usize - 1]
    }

    /// Convenient conversion from slice of underlying representation for efficiency purposes
    #[inline(always)]
    pub const fn slice_from_bytes(value: &[[u8; Self::SIZE]]) -> &[Self] {
        // SAFETY: `PotOutput` and `PotCheckpoints` are `#[repr(C)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion to slice of underlying representation for efficiency purposes
    #[inline(always)]
    pub const fn bytes_from_slice(value: &[Self]) -> &[[u8; Self::SIZE]] {
        // SAFETY: `PotOutput` and `PotCheckpoints` are `#[repr(C)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

/// Change of parameters to apply to the proof of time chain
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct PotParametersChange {
    // TODO: Reduce this to `u16` or even `u8` since it is always an offset relatively to current
    //  block's slot number
    /// At which slot change of parameters takes effect
    pub slot: SlotNumber,
    /// New number of slot iterations
    pub slot_iterations: NonZeroU32,
    /// Entropy that should be injected at this time
    pub entropy: Blake3Hash,
}
