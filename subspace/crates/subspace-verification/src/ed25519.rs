//! Primitives related to Sr25519

use ab_core_primitives::hashes::{Blake3Hash, blake3_hash};
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

/// Ed25519 public key
#[derive(Default, Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Deref, From, Into)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, TypeInfo))]
pub struct Ed25519PublicKey([u8; Ed25519PublicKey::SIZE]);

impl fmt::Debug for Ed25519PublicKey {
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
struct Ed25519PublicKeyBinary([u8; Ed25519PublicKey::SIZE]);

#[cfg(feature = "serde")]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct Ed25519PublicKeyHex(#[serde(with = "hex")] [u8; Ed25519PublicKey::SIZE]);

#[cfg(feature = "serde")]
impl Serialize for Ed25519PublicKey {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            Ed25519PublicKeyHex(self.0).serialize(serializer)
        } else {
            Ed25519PublicKeyBinary(self.0).serialize(serializer)
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for Ed25519PublicKey {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(if deserializer.is_human_readable() {
            Ed25519PublicKeyHex::deserialize(deserializer)?.0
        } else {
            Ed25519PublicKeyBinary::deserialize(deserializer)?.0
        }))
    }
}

impl fmt::Display for Ed25519PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl AsRef<[u8]> for Ed25519PublicKey {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl Ed25519PublicKey {
    /// Public key size in bytes
    pub const SIZE: usize = 32;

    /// Public key hash.
    pub fn hash(&self) -> Blake3Hash {
        blake3_hash(&self.0)
    }
}

/// Ed25519 signature
#[derive(Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Deref, From, Into)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, TypeInfo))]
pub struct Ed25519Signature([u8; Ed25519Signature::SIZE]);

impl Default for Ed25519Signature {
    fn default() -> Self {
        Self([0; Self::SIZE])
    }
}

impl fmt::Debug for Ed25519Signature {
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
struct Ed25519SignatureBinary(#[serde(with = "BigArray")] [u8; Ed25519Signature::SIZE]);

#[cfg(feature = "serde")]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct Ed25519SignatureHex(#[serde(with = "hex")] [u8; Ed25519Signature::SIZE]);

#[cfg(feature = "serde")]
impl Serialize for Ed25519Signature {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            Ed25519SignatureHex(self.0).serialize(serializer)
        } else {
            Ed25519SignatureBinary(self.0).serialize(serializer)
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for Ed25519Signature {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(if deserializer.is_human_readable() {
            Ed25519SignatureHex::deserialize(deserializer)?.0
        } else {
            Ed25519SignatureBinary::deserialize(deserializer)?.0
        }))
    }
}

impl fmt::Display for Ed25519Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl AsRef<[u8]> for Ed25519Signature {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl Ed25519Signature {
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
    pub public_key: Ed25519PublicKey,
    /// Signature itself
    pub signature: Ed25519Signature,
}

impl RewardSignature {
    /// Reward signature size in bytes
    pub const SIZE: usize = Ed25519PublicKey::SIZE + Ed25519Signature::SIZE;
}
