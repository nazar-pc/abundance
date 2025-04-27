//! Data structures related to objects (useful data) stored on Subspace Network.
//!
//! There are two kinds of mappings:
//! * for objects within a block
//! * for global objects in the global history of the blockchain (inside a piece)

use crate::hashes::Blake3Hash;
use crate::pieces::PieceIndex;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;
#[cfg(feature = "alloc")]
use core::default::Default;
#[cfg(feature = "scale-codec")]
use parity_scale_codec::{Decode, Encode};
#[cfg(feature = "scale-codec")]
use scale_info::TypeInfo;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Object stored inside of the block
#[derive(Debug, Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, TypeInfo))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct BlockObject {
    /// Object hash
    pub hash: Blake3Hash,
    /// Offset of object in the encoded block.
    pub offset: u32,
}

/// Mapping of objects stored inside of the block
#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, TypeInfo))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "serde", serde(rename_all_fields = "camelCase"))]
#[cfg(feature = "alloc")]
pub enum BlockObjectMapping {
    /// V0 of object mapping data structure
    #[cfg_attr(feature = "scale-codec", codec(index = 0))]
    V0 {
        /// Objects stored inside of the block
        objects: Vec<BlockObject>,
    },
}

#[cfg(feature = "alloc")]
impl Default for BlockObjectMapping {
    #[inline(always)]
    fn default() -> Self {
        Self::V0 {
            objects: Vec::new(),
        }
    }
}

#[cfg(feature = "alloc")]
impl BlockObjectMapping {
    /// Returns a newly created BlockObjectMapping from a list of object mappings
    #[inline(always)]
    pub fn from_objects(objects: impl IntoIterator<Item = BlockObject>) -> Self {
        Self::V0 {
            objects: objects.into_iter().collect(),
        }
    }

    /// Returns the object mappings
    #[inline(always)]
    pub fn objects(&self) -> &[BlockObject] {
        match self {
            Self::V0 { objects, .. } => objects,
        }
    }

    /// Returns the object mappings as a mutable slice
    #[inline(always)]
    pub fn objects_mut(&mut self) -> &mut Vec<BlockObject> {
        match self {
            Self::V0 { objects, .. } => objects,
        }
    }
}

/// Object stored in the history of the blockchain
#[derive(Debug, Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, TypeInfo))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct GlobalObject {
    /// Object hash.
    /// We order by hash, so object hash lookups can be performed efficiently.
    pub hash: Blake3Hash,
    /// Piece index where object is contained (at least its beginning, might not fit fully)
    pub piece_index: PieceIndex,
    /// Raw record offset of the object in that piece, for use with `Record::to_raw_record_bytes`
    pub offset: u32,
}

/// Mapping of objects stored in the history of the blockchain
#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, TypeInfo))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "serde", serde(rename_all_fields = "camelCase"))]
#[cfg(feature = "alloc")]
pub enum GlobalObjectMapping {
    /// V0 of object mapping data structure.
    #[cfg_attr(feature = "scale-codec", codec(index = 0))]
    V0 {
        /// Objects stored in the history of the blockchain.
        objects: Vec<GlobalObject>,
    },
}

#[cfg(feature = "alloc")]
impl Default for GlobalObjectMapping {
    #[inline(always)]
    fn default() -> Self {
        Self::V0 {
            objects: Vec::new(),
        }
    }
}

#[cfg(feature = "alloc")]
impl GlobalObjectMapping {
    /// Returns a newly created GlobalObjectMapping from a list of object mappings
    #[inline(always)]
    pub fn from_objects(objects: impl IntoIterator<Item = GlobalObject>) -> Self {
        Self::V0 {
            objects: objects.into_iter().collect(),
        }
    }

    /// Returns the object mappings
    #[inline(always)]
    pub fn objects(&self) -> &[GlobalObject] {
        match self {
            Self::V0 { objects, .. } => objects,
        }
    }

    /// Returns the object mappings as a mutable slice
    #[inline(always)]
    pub fn objects_mut(&mut self) -> &mut Vec<GlobalObject> {
        match self {
            Self::V0 { objects, .. } => objects,
        }
    }
}
