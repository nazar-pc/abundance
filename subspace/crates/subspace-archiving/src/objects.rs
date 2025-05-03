//! Data structures related to objects (useful data) stored on Subspace Network.
//!
//! There are two kinds of mappings:
//! * for objects within a block
//! * for global objects in the global history of the blockchain (inside a piece)

use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pieces::PieceIndex;
use alloc::vec::Vec;
use parity_scale_codec::{Decode, Encode};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Object stored inside the block
#[derive(Debug, Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Encode, Decode)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct BlockObject {
    /// Object hash
    pub hash: Blake3Hash,
    /// Offset of the object in the encoded block
    pub offset: u32,
}

/// Mapping of objects stored inside the block
#[derive(Debug, Default, Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Encode, Decode)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "serde", serde(rename_all_fields = "camelCase"))]
pub struct BlockObjectMapping {
    /// Objects stored inside the block
    pub objects: Vec<BlockObject>,
}

/// Object stored in the history of the blockchain
#[derive(Debug, Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Encode, Decode)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct GlobalObject {
    /// Object hash.
    ///
    /// We order objects by hash, so object hash lookups can be performed efficiently.
    pub hash: Blake3Hash,
    /// Piece index where the object is contained (at least its beginning, might not fit fully)
    pub piece_index: PieceIndex,
    /// Raw record offset of the object in that piece, for use with `Record::to_raw_record_bytes`
    pub offset: u32,
}

/// Mapping of objects stored in the history of the blockchain
#[derive(Debug, Default, Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Encode, Decode)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "serde", serde(rename_all_fields = "camelCase"))]
pub struct GlobalObjectMapping {
    /// Objects stored in the history of the blockchain
    pub objects: Vec<GlobalObject>,
}
