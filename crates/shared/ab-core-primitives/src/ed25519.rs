//! Primitives related to Ed25519

use crate::hashes::Blake3Hash;
use ab_blake3::single_block_hash;
use ab_io_type::trivial_type::TrivialType;
use core::fmt;
use derive_more::{Deref, From, Into};
use ed25519_dalek::{Signature, SignatureError, Verifier, VerifyingKey};
#[cfg(feature = "scale-codec")]
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};
#[cfg(feature = "serde")]
use serde_big_array::BigArray;

/// Ed25519 public key
#[derive(
    Default, Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Deref, From, Into, TrivialType,
)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, MaxEncodedLen))]
#[repr(C)]
pub struct Ed25519PublicKey([u8; Ed25519PublicKey::SIZE]);

impl From<VerifyingKey> for Ed25519PublicKey {
    #[inline(always)]
    fn from(verification_key: VerifyingKey) -> Self {
        Ed25519PublicKey(verification_key.to_bytes())
    }
}

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
        Blake3Hash::new(
            single_block_hash(&self.0).expect("Less than a single block worth of bytes; qed"),
        )
    }

    /// Verify Ed25519 signature
    #[inline]
    pub fn verify(&self, signature: &Ed25519Signature, msg: &[u8]) -> Result<(), SignatureError> {
        // TODO: Switch to RFC8032 / NIST validation criteria instead once
        //  https://github.com/dalek-cryptography/curve25519-dalek/issues/626 is resolved
        VerifyingKey::from_bytes(&self.0)?.verify(msg, &Signature::from_bytes(signature))
    }
}

/// Ed25519 signature
#[derive(Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Deref, From, Into, TrivialType)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, MaxEncodedLen))]
#[repr(C)]
pub struct Ed25519Signature([u8; Ed25519Signature::SIZE]);

impl From<Signature> for Ed25519Signature {
    #[inline(always)]
    fn from(signature: Signature) -> Self {
        Ed25519Signature::from(signature.to_bytes())
    }
}

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
