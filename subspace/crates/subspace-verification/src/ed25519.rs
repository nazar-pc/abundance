//! Primitives related to Ed25519

use ab_core_primitives::ed25519::{Ed25519PublicKey, Ed25519Signature};
#[cfg(feature = "scale-codec")]
use parity_scale_codec::{Decode, Encode};
#[cfg(feature = "scale-codec")]
use scale_info::TypeInfo;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

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
