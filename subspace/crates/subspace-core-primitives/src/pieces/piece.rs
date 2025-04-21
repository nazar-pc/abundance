use crate::pieces::cow_bytes::CowBytes;
use crate::pieces::PieceArray;
use alloc::format;
use alloc::vec::Vec;
use bytes::{Bytes, BytesMut};
use core::ops::{Deref, DerefMut};
#[cfg(feature = "scale-codec")]
use parity_scale_codec::{Decode, Encode, EncodeLike, Input, Output};
#[cfg(feature = "scale-codec")]
use scale_info::build::Fields;
#[cfg(feature = "scale-codec")]
use scale_info::{Path, Type, TypeInfo};
#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A piece of archival history in Subspace Network.
///
/// This version is allocated on the heap, for stack-allocated piece see [`PieceArray`].
///
/// Internally piece contains a record and corresponding witness that together with segment
/// root of the segment this piece belongs to can be used to verify that a piece belongs to
/// the actual archival history of the blockchain.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Piece(pub(super) CowBytes);

#[cfg(feature = "scale-codec")]
impl Encode for Piece {
    #[inline]
    fn size_hint(&self) -> usize {
        self.as_ref().size_hint()
    }

    #[inline]
    fn encode_to<O: Output + ?Sized>(&self, output: &mut O) {
        self.as_ref().encode_to(output)
    }

    #[inline]
    fn encode(&self) -> Vec<u8> {
        self.as_ref().encode()
    }

    #[inline]
    fn using_encoded<R, F: FnOnce(&[u8]) -> R>(&self, f: F) -> R {
        self.as_ref().using_encoded(f)
    }
}

#[cfg(feature = "scale-codec")]
impl EncodeLike for Piece {}

#[cfg(feature = "scale-codec")]
impl Decode for Piece {
    fn decode<I: Input>(input: &mut I) -> Result<Self, parity_scale_codec::Error> {
        let bytes =
            Bytes::decode(input).map_err(|error| error.chain("Could not decode `Piece`"))?;

        if bytes.len() != Self::SIZE {
            return Err(
                parity_scale_codec::Error::from("Incorrect Piece length").chain(format!(
                    "Expected {} bytes, found {} bytes",
                    Self::SIZE,
                    bytes.len()
                )),
            );
        }

        Ok(Piece(CowBytes::Shared(bytes)))
    }
}

#[cfg(feature = "scale-codec")]
impl TypeInfo for Piece {
    type Identity = Self;

    fn type_info() -> Type {
        Type::builder()
            .path(Path::new("Piece", module_path!()))
            .docs(&["A piece of archival history in Subspace Network"])
            .composite(
                Fields::unnamed().field(|f| f.ty::<[u8; Piece::SIZE]>().type_name("PieceArray")),
            )
    }
}

#[cfg(feature = "serde")]
impl Serialize for Piece {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let bytes = match &self.0 {
            CowBytes::Shared(bytes) => bytes.as_ref(),
            CowBytes::Owned(bytes) => bytes.as_ref(),
        };

        if serializer.is_human_readable() {
            hex::serde::serialize(bytes, serializer)
        } else {
            bytes.serialize(serializer)
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for Piece {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes = if deserializer.is_human_readable() {
            hex::serde::deserialize::<_, Vec<u8>>(deserializer).and_then(|bytes| {
                if bytes.len() == Piece::SIZE {
                    Ok(Bytes::from(bytes))
                } else {
                    Err(serde::de::Error::invalid_length(
                        bytes.len(),
                        &format!("Expected {} bytes", Piece::SIZE).as_str(),
                    ))
                }
            })?
        } else {
            Bytes::deserialize(deserializer)?
        };

        Ok(Piece(CowBytes::Shared(bytes)))
    }
}

impl Default for Piece {
    #[inline]
    fn default() -> Self {
        Self(CowBytes::Owned(BytesMut::zeroed(Self::SIZE)))
    }
}

impl From<Piece> for Vec<u8> {
    #[inline]
    fn from(piece: Piece) -> Self {
        match piece.0 {
            CowBytes::Shared(bytes) => bytes.to_vec(),
            CowBytes::Owned(bytes) => Vec::from(bytes),
        }
    }
}

impl TryFrom<&[u8]> for Piece {
    type Error = ();

    #[inline]
    fn try_from(slice: &[u8]) -> Result<Self, Self::Error> {
        if slice.len() != Self::SIZE {
            return Err(());
        }

        Ok(Self(CowBytes::Shared(Bytes::copy_from_slice(slice))))
    }
}

impl TryFrom<Vec<u8>> for Piece {
    type Error = ();

    #[inline]
    fn try_from(vec: Vec<u8>) -> Result<Self, Self::Error> {
        if vec.len() != Self::SIZE {
            return Err(());
        }

        Ok(Self(CowBytes::Shared(Bytes::from(vec))))
    }
}

impl TryFrom<Bytes> for Piece {
    type Error = ();

    #[inline]
    fn try_from(bytes: Bytes) -> Result<Self, Self::Error> {
        if bytes.len() != Self::SIZE {
            return Err(());
        }

        Ok(Self(CowBytes::Shared(bytes)))
    }
}

impl TryFrom<BytesMut> for Piece {
    type Error = ();

    #[inline]
    fn try_from(bytes: BytesMut) -> Result<Self, Self::Error> {
        if bytes.len() != Self::SIZE {
            return Err(());
        }

        Ok(Self(CowBytes::Owned(bytes)))
    }
}

impl From<&PieceArray> for Piece {
    #[inline]
    fn from(value: &PieceArray) -> Self {
        Self(CowBytes::Shared(Bytes::copy_from_slice(value.as_ref())))
    }
}

impl Deref for Piece {
    type Target = PieceArray;

    #[inline]
    fn deref(&self) -> &Self::Target {
        <&[u8; Self::SIZE]>::try_from(self.as_ref())
            .expect("Slice of memory has correct length; qed")
            .into()
    }
}

impl DerefMut for Piece {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        <&mut [u8; Self::SIZE]>::try_from(self.as_mut())
            .expect("Slice of memory has correct length; qed")
            .into()
    }
}

impl AsRef<[u8]> for Piece {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl AsMut<[u8]> for Piece {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        self.0.as_mut()
    }
}

impl Piece {
    /// Size of a piece (in bytes).
    pub const SIZE: usize = PieceArray::SIZE;

    /// Ensure piece contains cheaply cloneable shared data.
    ///
    /// Internally piece uses CoW mechanism and can store either mutable owned data or data that is
    /// cheap to clone, calling this method will ensure further clones will not result in additional
    /// memory allocations.
    pub fn to_shared(self) -> Self {
        Self(match self.0 {
            CowBytes::Shared(bytes) => CowBytes::Shared(bytes),
            CowBytes::Owned(bytes) => CowBytes::Shared(bytes.freeze()),
        })
    }
}
