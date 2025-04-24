//! Primitives related to Sr25519

use core::fmt;
use derive_more::{Deref, From, Into};
#[cfg(feature = "scale-codec")]
use parity_scale_codec::{Decode, Encode};
#[cfg(feature = "scale-codec")]
use scale_info::TypeInfo;
#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};
#[cfg(feature = "serde")]
use serde_big_array::BigArray;
use subspace_core_primitives::hashes::{blake3_hash, Blake3Hash};

/// Signing context used for creating reward signatures by farmers.
pub const REWARD_SIGNING_CONTEXT: &[u8] = b"subspace_reward";

/// A Ristretto Schnorr public key as bytes produced by `schnorrkel` crate.
#[derive(Default, Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Deref, From, Into)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, TypeInfo))]
pub struct PublicKey([u8; PublicKey::SIZE]);

impl fmt::Debug for PublicKey {
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
struct PublicKeyBinary([u8; PublicKey::SIZE]);

#[cfg(feature = "serde")]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct PublicKeyHex(#[serde(with = "hex")] [u8; PublicKey::SIZE]);

#[cfg(feature = "serde")]
impl Serialize for PublicKey {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            PublicKeyHex(self.0).serialize(serializer)
        } else {
            PublicKeyBinary(self.0).serialize(serializer)
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for PublicKey {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(if deserializer.is_human_readable() {
            PublicKeyHex::deserialize(deserializer)?.0
        } else {
            PublicKeyBinary::deserialize(deserializer)?.0
        }))
    }
}

impl fmt::Display for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl AsRef<[u8]> for PublicKey {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl PublicKey {
    /// Public key size in bytes
    pub const SIZE: usize = 32;

    /// Public key hash.
    pub fn hash(&self) -> Blake3Hash {
        blake3_hash(&self.0)
    }
}

/// A Ristretto Schnorr signature as bytes produced by `schnorrkel` crate.
#[derive(Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Deref, From, Into)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, TypeInfo))]
pub struct Signature([u8; Signature::SIZE]);

impl Default for Signature {
    fn default() -> Self {
        Self([0; Self::SIZE])
    }
}

impl fmt::Debug for Signature {
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
struct SignatureBinary(#[serde(with = "BigArray")] [u8; Signature::SIZE]);

#[cfg(feature = "serde")]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct SignatureHex(#[serde(with = "hex")] [u8; Signature::SIZE]);

#[cfg(feature = "serde")]
impl Serialize for Signature {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            SignatureHex(self.0).serialize(serializer)
        } else {
            SignatureBinary(self.0).serialize(serializer)
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for Signature {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(if deserializer.is_human_readable() {
            SignatureHex::deserialize(deserializer)?.0
        } else {
            SignatureBinary::deserialize(deserializer)?.0
        }))
    }
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl AsRef<[u8]> for Signature {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl Signature {
    /// Signature size in bytes
    pub const SIZE: usize = 64;
}

// TODO: Remove this from core primitives
/// A Ristretto Schnorr signature as bytes produced by `schnorrkel` crate.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, TypeInfo))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RewardSignature {
    /// Public key that signature corresponds to
    pub public_key: PublicKey,
    /// Signature itself
    pub signature: Signature,
}

impl RewardSignature {
    /// Reward signature size in bytes
    pub const SIZE: usize = PublicKey::SIZE + Signature::SIZE;
}
