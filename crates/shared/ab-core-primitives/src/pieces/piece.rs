#[cfg(test)]
mod tests;

use crate::pieces::{InnerPiece, PieceHeader, Record};
use ab_aligned_buffer::alignment::{Align1, Alignment};
use ab_aligned_buffer::{OwnedAlignedBuffer, SharedAlignedBuffer};
use ab_io_type::trivial_type::TrivialType;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::ops::{Deref, DerefMut};
#[cfg(feature = "scale-codec")]
use parity_scale_codec::{Decode, Encode, EncodeLike, Input, Output};
#[cfg(feature = "serde")]
use serde::de::Visitor;
#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A piece of archival history.
///
/// This version is allocated on the heap, for stack-allocated piece see [`InnerPiece`].
///
/// Internally piece contains a record and corresponding proof that together with segment
/// root of the segment this piece belongs to can be used to verify that a piece belongs to
/// the actual archival history of the blockchain.
///
/// Piece is a shared buffer, which is cheap to clone. Mutable access to a piece whose allocation is
/// still shared (with other pieces or with [`FlatPieces`] it was created from, for example) copies
/// it into a new allocation first.
///
/// [`FlatPieces`]: crate::pieces::FlatPieces
#[derive(Clone)]
pub struct Piece(pub(super) SharedAlignedBuffer<Align1>);

impl fmt::Debug for Piece {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.as_ref() {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl PartialEq for Piece {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.as_ref() == other.as_ref()
    }
}

impl Eq for Piece {}

impl Hash for Piece {
    #[inline]
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        self.as_ref().hash(state);
    }
}

/// Encoded as bytes of the piece without length, since the size of a piece is fixed
#[cfg(feature = "scale-codec")]
impl Encode for Piece {
    #[inline]
    fn size_hint(&self) -> usize {
        Self::SIZE
    }

    #[inline]
    fn encode_to<O>(&self, dest: &mut O)
    where
        O: Output + ?Sized,
    {
        dest.write(self.as_ref());
    }

    #[inline]
    fn using_encoded<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        f(self.as_ref())
    }

    #[inline]
    fn encoded_size(&self) -> usize {
        Self::SIZE
    }
}

#[cfg(feature = "scale-codec")]
impl EncodeLike for Piece {}

#[cfg(feature = "scale-codec")]
impl Decode for Piece {
    fn decode<I>(input: &mut I) -> Result<Self, parity_scale_codec::Error>
    where
        I: Input,
    {
        input.on_before_alloc_mem(Self::SIZE)?;
        let mut buffer = OwnedAlignedBuffer::<Align1>::zeroed(Self::SIZE_U32);
        input
            .read(buffer.as_mut_slice())
            .map_err(|error| error.chain("Could not decode `Piece`"))?;

        Ok(Self(buffer.into_shared()))
    }

    #[inline]
    fn encoded_fixed_size() -> Option<usize> {
        Some(Self::SIZE)
    }
}

/// Serialized as a hex string in human-readable formats and as bytes otherwise
#[cfg(feature = "serde")]
impl Serialize for Piece {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            hex::serde::serialize(self.as_ref(), serializer)
        } else {
            serializer.serialize_bytes(self.as_ref())
        }
    }
}

/// Accepts a hex string or bytes of a piece, contents are written into a new buffer
/// directly
#[cfg(feature = "serde")]
struct PieceVisitor;

#[cfg(feature = "serde")]
impl Visitor<'_> for PieceVisitor {
    type Value = Piece;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} bytes or a hex string of them", Piece::SIZE)
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let mut buffer = OwnedAlignedBuffer::<Align1>::zeroed(Piece::SIZE_U32);
        hex::decode_to_slice(v, buffer.as_mut_slice()).map_err(E::custom)?;

        Ok(Piece(buffer.into_shared()))
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Piece::try_from(v).map_err(|()| E::invalid_length(v.len(), &self))
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for Piece {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            deserializer.deserialize_str(PieceVisitor)
        } else {
            deserializer.deserialize_bytes(PieceVisitor)
        }
    }
}

impl Default for Piece {
    #[inline]
    fn default() -> Self {
        Self(OwnedAlignedBuffer::<Align1>::zeroed(Self::SIZE_U32).into_shared())
    }
}

impl TryFrom<&[u8]> for Piece {
    type Error = ();

    #[inline]
    fn try_from(slice: &[u8]) -> Result<Self, Self::Error> {
        if slice.len() != Self::SIZE {
            return Err(());
        }

        Ok(Self(SharedAlignedBuffer::from_bytes(slice)))
    }
}

impl<A> TryFrom<SharedAlignedBuffer<A>> for Piece
where
    A: Alignment,
{
    type Error = ();

    /// Uses the buffer without copying it
    #[inline]
    fn try_from(buffer: SharedAlignedBuffer<A>) -> Result<Self, Self::Error> {
        if buffer.len() != Self::SIZE_U32 {
            return Err(());
        }

        Ok(Self(buffer.into_start_align()))
    }
}

impl<A> TryFrom<OwnedAlignedBuffer<A>> for Piece
where
    A: Alignment,
{
    type Error = ();

    /// Uses the buffer without copying it
    #[inline]
    fn try_from(buffer: OwnedAlignedBuffer<A>) -> Result<Self, Self::Error> {
        Self::try_from(buffer.into_shared())
    }
}

impl From<&InnerPiece> for Piece {
    #[inline]
    fn from(value: &InnerPiece) -> Self {
        Self(SharedAlignedBuffer::from_bytes(value.as_bytes()))
    }
}

impl From<Piece> for SharedAlignedBuffer<Align1> {
    #[inline]
    fn from(piece: Piece) -> Self {
        piece.0
    }
}

impl Deref for Piece {
    type Target = InnerPiece;

    #[inline]
    fn deref(&self) -> &Self::Target {
        // SAFETY: Piece bytes are always `InnerPiece::SIZE` long (checked in constructors),
        // `InnerPiece` has alignment of 1 and any bit pattern is a valid value (both checked
        // next to its definition)
        unsafe { InnerPiece::from_bytes_unchecked(self.as_ref()) }
    }
}

impl DerefMut for Piece {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: Piece bytes are always `InnerPiece::SIZE` long (checked in constructors),
        // `InnerPiece` has alignment of 1 and any bit pattern is a valid value (both checked
        // next to its definition)
        unsafe { InnerPiece::from_bytes_mut_unchecked(self.as_mut()) }
    }
}

impl AsRef<[u8]> for Piece {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl AsMut<[u8]> for Piece {
    /// Copies the piece into a new allocation first if its allocation is still shared
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        self.0.make_mut()
    }
}

impl Piece {
    /// Size of a piece (in bytes)
    pub const SIZE: usize = InnerPiece::SIZE;
    /// [`Self::SIZE`] as `u32`, the type of lengths of buffers
    pub(super) const SIZE_U32: u32 = {
        assert!(Self::SIZE <= u32::MAX as usize);
        Self::SIZE as u32
    };

    /// Create a piece from its header and record.
    ///
    /// Contents are written into a new allocation directly, without zeroing it first or going
    /// through the stack.
    pub fn from_parts(header: &PieceHeader, record: &Record) -> Self {
        let mut buffer = OwnedAlignedBuffer::<Align1>::with_capacity(Self::SIZE_U32);
        let inner_piece = buffer
            .spare_capacity_mut()
            .as_mut_ptr()
            .cast::<InnerPiece>();
        // SAFETY: Capacity is sufficient for `InnerPiece`, whose alignment is `1`. Both fields are
        // written, which initializes the whole `InnerPiece` since it has no padding.
        unsafe {
            (&raw mut (*inner_piece).header).write(*header);
            (&raw mut (*inner_piece).record).copy_from_nonoverlapping(record, 1);
            buffer.set_len(Self::SIZE_U32);
        }

        Self(buffer.into_shared())
    }

    /// Shared buffer with the contents of the piece
    #[inline(always)]
    pub fn buffer(&self) -> &SharedAlignedBuffer<Align1> {
        &self.0
    }
}
