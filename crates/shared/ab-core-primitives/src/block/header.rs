//! Block header primitives

use crate::block::{BlockHash, BlockNumber};
use crate::hashes::Blake3Hash;
use crate::pot::{PotOutput, PotParametersChange, SlotNumber};
use crate::segments::SuperSegmentRoot;
use crate::shard::{ShardIndex, ShardKind};
use crate::solutions::{Solution, SolutionRange};
#[cfg(feature = "serde")]
use ::serde::{Deserialize, Serialize};
use ab_io_type::trivial_type::TrivialType;
use ab_merkle_tree::unbalanced_hashed::UnbalancedHashedMerkleTree;
use core::num::NonZeroU32;
use core::ops::Deref;
use core::slice;
use derive_more::{Deref, From};
#[cfg(feature = "scale-codec")]
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
#[cfg(feature = "scale-codec")]
use scale_info::TypeInfo;
#[cfg(feature = "serde")]
use serde_big_array::BigArray;

/// Block header prefix.
///
/// The prefix contains generic information known about the block before block creation starts.
#[derive(Debug, Copy, Clone, Eq, PartialEq, TrivialType)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[repr(C)]
pub struct BlockHeaderPrefix {
    /// Block version
    pub version: u64,
    /// Block number
    pub number: BlockNumber,
    /// Shard index
    pub shard_index: ShardIndex,
    /// Padding for data structure alignment
    pub padding: [u8; 4],
    /// Unix timestamp in ms
    // TODO: New type?
    pub timestamp: u64,
    /// Hash of the parent block
    pub parent_hash: BlockHash,
    /// MMR root of all block hashes, including `parent_hash`
    // TODO: New type?
    pub mmr_root: Blake3Hash,
}

impl BlockHeaderPrefix {
    /// The only supported block version right now
    pub const BLOCK_VERSION: u64 = 0;

    /// Hash of the block header prefix, part of the eventual block hash
    pub fn hash(&self) -> Blake3Hash {
        // TODO: Keyed hash
        Blake3Hash::from(blake3::hash(self.as_bytes()))
    }
}

/// Consensus information in block header
#[derive(Debug, Copy, Clone, Eq, PartialEq, TrivialType)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[repr(C)]
pub struct BlockHeaderConsensusInfo {
    /// Slot number
    pub slot: SlotNumber,
    /// Proof of time for this slot
    pub proof_of_time: PotOutput,
    /// Future proof of time
    pub future_proof_of_time: PotOutput,
    /// Solution
    pub solution: Solution,
}

impl BlockHeaderConsensusInfo {
    /// Hash of the consensus info, part of the eventual block hash
    pub fn hash(&self) -> Blake3Hash {
        // TODO: Keyed hash
        Blake3Hash::from(blake3::hash(self.as_bytes()))
    }
}

/// Beacon chain info
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, TrivialType)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[repr(C)]
pub struct BlockHeaderBeaconChainInfo {
    /// Beacon chain block number
    pub number: BlockNumber,
    /// Beacon chain block hash
    pub hash: BlockHash,
}

impl BlockHeaderBeaconChainInfo {
    /// Hash of the beacon chain info, part of the eventual block hash
    pub fn hash(&self) -> Blake3Hash {
        // TODO: Keyed hash
        Blake3Hash::from(blake3::hash(self.as_bytes()))
    }
}

/// Consensus parameters (on the beacon chain)
#[derive(Debug, Copy, Clone)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct BlockHeaderFixedConsensusParameters {
    /// Solution range for this block/era
    pub solution_range: SolutionRange,
    /// The number of iterations for proof of time per slot.
    ///
    /// Corresponds to the slot that is right after the parent block's slot.
    /// It can change before the slot of this block (see [`PotParametersChange`]).
    pub pot_slot_iterations: NonZeroU32,
}

impl BlockHeaderFixedConsensusParameters {
    /// Create an instance from provided bytes.
    ///
    /// `bytes` do not need to be aligned.
    ///
    /// Returns an instance and remaining bytes on success.
    #[inline]
    pub fn try_from_bytes(mut bytes: &[u8]) -> Option<(Self, &[u8])> {
        // Layout here is as follows:
        // * solution range: SolutionRange as unaligned bytes
        // * PoT slot iterations: NonZeroU32 as unaligned little-endian bytes

        let solution_range = bytes.split_off(..size_of::<SolutionRange>())?;
        let solution_range = SolutionRange::from_bytes([
            solution_range[0],
            solution_range[1],
            solution_range[2],
            solution_range[3],
            solution_range[4],
            solution_range[5],
            solution_range[6],
            solution_range[7],
        ]);

        let pot_slot_iterations = bytes.split_off(..size_of::<u32>())?;
        let pot_slot_iterations = u32::from_le_bytes([
            pot_slot_iterations[0],
            pot_slot_iterations[1],
            pot_slot_iterations[2],
            pot_slot_iterations[3],
        ]);
        let pot_slot_iterations = NonZeroU32::new(pot_slot_iterations)?;

        Some((
            Self {
                solution_range,
                pot_slot_iterations,
            },
            bytes,
        ))
    }
}

/// A mirror of [`PotParametersChange`] for block header purposes.
///
/// Use [`From`] or [`Into`] for converting into [`PotParametersChange`] before use.
#[derive(Debug, Copy, Clone)]
#[repr(C, packed)]
pub struct BlockHeaderPotParametersChange {
    // TODO: Reduce this to `u16` or even `u8` since it is always an offset relatively to current
    //  block's slot number
    /// At which slot change of parameters takes effect
    slot: SlotNumber,
    /// New number of slot iterations
    slot_iterations: NonZeroU32,
    /// Entropy that should be injected at this time
    entropy: Blake3Hash,
}

impl From<BlockHeaderPotParametersChange> for PotParametersChange {
    #[inline(always)]
    fn from(value: BlockHeaderPotParametersChange) -> Self {
        let BlockHeaderPotParametersChange {
            slot,
            slot_iterations,
            entropy,
        } = value;

        PotParametersChange {
            slot,
            slot_iterations,
            entropy,
        }
    }
}

impl BlockHeaderPotParametersChange {
    /// Get instance reference from provided bytes.
    ///
    /// `bytes` do not need to be aligned.
    ///
    /// Returns an instance and remaining bytes on success.
    #[inline]
    pub fn try_from_bytes(mut bytes: &[u8]) -> Option<(&Self, &[u8])> {
        // Layout here is as follows:
        // * slot number: SlotNumber as unaligned bytes
        // * slot iterations: NonZeroU32 as unaligned little-endian bytes
        // * entropy: Blake3Hash

        let _slot = bytes.split_off(..size_of::<SlotNumber>())?;

        let slot_iterations = bytes.split_off(..size_of::<u32>())?;
        if slot_iterations == [0, 0, 0, 0] {
            return None;
        }
        let _entropy = bytes.split_off(..size_of::<Blake3Hash>())?;

        // SAFETY: Not null, packed, bit pattern for `NonZeroU32` checked above
        let pot_parameters_change = unsafe { bytes.as_ptr().cast::<Self>().as_ref_unchecked() };

        Some((pot_parameters_change, bytes))
    }
}

/// Consensus parameters (on the beacon chain)
#[derive(Debug, Copy, Clone)]
pub struct BlockHeaderBeaconChainParameters<'a> {
    /// Consensus parameters that are always present
    pub fixed_parameters: BlockHeaderFixedConsensusParameters,
    /// Super segment root
    pub super_segment_root: Option<&'a SuperSegmentRoot>,
    /// Solution range for the next block/era (if any)
    pub next_solution_range: Option<SolutionRange>,
    /// Change of parameters to apply to the proof of time chain (if any)
    pub pot_parameters_change: Option<&'a BlockHeaderPotParametersChange>,
}

impl<'a> BlockHeaderBeaconChainParameters<'a> {
    /// Bitmask for presence of `super_segment_root` field
    pub const SUPER_SEGMENT_ROOT_MASK: u8 = 0b_0000_0001;
    /// Bitmask for presence of `next_solution_range` field
    pub const NEXT_SOLUTION_RANGE_MASK: u8 = 0b_0000_0010;
    /// Bitmask for presence of `pot_parameters_change` field
    pub const POT_PARAMETERS_CHANGE_MASK: u8 = 0b_0000_0100;

    /// Create an instance from provided bytes.
    ///
    /// `bytes` do not need to be aligned.
    ///
    /// Returns an instance and remaining bytes on success.
    #[inline]
    pub fn try_from_bytes(bytes: &'a [u8]) -> Option<(Self, &'a [u8])> {
        // Layout here is as follows:
        // * fixed parameters: BlockHeaderFixedConsensusParameters
        // * bitflags: u8
        // * (optional, depends on bitflags) super segment root: SuperSegmentRoot
        // * (optional, depends on bitflags) next solution range: SolutionRange as unaligned bytes
        // * (optional, depends on bitflags) PoT parameters change: BlockHeaderPotParametersChange

        let (fixed_parameters, mut remainder) =
            BlockHeaderFixedConsensusParameters::try_from_bytes(bytes)?;

        let bitflags = remainder.split_off(..size_of::<u8>())?;
        let bitflags = bitflags[0];

        let super_segment_root = if bitflags & Self::SUPER_SEGMENT_ROOT_MASK != 0 {
            let super_segment_root = remainder.split_off(..size_of::<SuperSegmentRoot>())?;
            // SAFETY: All bit patterns are valid
            let super_segment_root = unsafe { SuperSegmentRoot::from_bytes(super_segment_root) }?;

            Some(super_segment_root)
        } else {
            None
        };

        let next_solution_range = if bitflags & Self::NEXT_SOLUTION_RANGE_MASK != 0 {
            let next_solution_range = remainder.split_off(..size_of::<SolutionRange>())?;
            // Not guaranteed to be aligned
            let next_solution_range = SolutionRange::from_bytes([
                next_solution_range[0],
                next_solution_range[1],
                next_solution_range[2],
                next_solution_range[3],
                next_solution_range[4],
                next_solution_range[5],
                next_solution_range[6],
                next_solution_range[7],
            ]);

            Some(next_solution_range)
        } else {
            None
        };

        let pot_parameters_change = if bitflags & Self::POT_PARAMETERS_CHANGE_MASK != 0 {
            let pot_parameters_change;
            (pot_parameters_change, remainder) =
                BlockHeaderPotParametersChange::try_from_bytes(remainder)?;

            Some(pot_parameters_change)
        } else {
            None
        };

        Some((
            Self {
                super_segment_root,
                fixed_parameters,
                next_solution_range,
                pot_parameters_change,
            },
            remainder,
        ))
    }

    /// Hash of the block consensus parameters, part of the eventual block hash
    pub fn hash(&self) -> Blake3Hash {
        let Self {
            super_segment_root,
            fixed_parameters,
            next_solution_range,
            pot_parameters_change,
        } = self;
        let BlockHeaderFixedConsensusParameters {
            solution_range,
            pot_slot_iterations,
        } = fixed_parameters;

        // TODO: Keyed hash
        let mut hasher = blake3::Hasher::new();
        hasher.update(solution_range.as_bytes());
        hasher.update(&pot_slot_iterations.get().to_le_bytes());

        if let Some(super_segment_root) = super_segment_root {
            hasher.update(super_segment_root.as_bytes());
        }
        if let Some(next_solution_range) = next_solution_range {
            hasher.update(next_solution_range.as_bytes());
        }
        if let Some(pot_parameters_change) = pot_parameters_change.copied() {
            let BlockHeaderPotParametersChange {
                slot,
                slot_iterations,
                entropy,
            } = pot_parameters_change;
            hasher.update(slot.as_bytes());
            hasher.update(&slot_iterations.get().to_le_bytes());
            hasher.update(entropy.as_bytes());
        }

        Blake3Hash::from(hasher.finalize())
    }
}

/// Information about child shard blocks
#[derive(Debug, Copy, Clone, Deref)]
pub struct BlockHeaderChildShardBlocks<'a> {
    /// Child shards blocks
    pub child_shard_blocks: &'a [BlockHash],
}

impl<'a> BlockHeaderChildShardBlocks<'a> {
    /// Create an instance from provided correctly aligned bytes.
    ///
    /// `bytes` should be 2-bytes aligned.
    ///
    /// Returns an instance and remaining bytes on success.
    #[inline]
    pub fn try_from_bytes(mut bytes: &'a [u8]) -> Option<(Self, &'a [u8])> {
        // Layout here is as follows:
        // * number of blocks: u16 as aligned little-endian bytes
        // * for each block:
        //   * child shard block: BlockHash

        let length = bytes.split_off(..size_of::<u16>())?;
        // SAFETY: All bit patterns are valid
        let num_blocks = usize::from(*unsafe { <u16 as TrivialType>::from_bytes(length) }?);

        let padding = bytes.split_off(..size_of::<[u8; 2]>())?;

        // Padding must be zero
        if padding != [0, 0] {
            return None;
        }

        let child_shard_blocks = bytes.split_off(..num_blocks * BlockHash::SIZE)?;
        // SAFETY: Valid pointer and size, no alignment requirements
        let child_shard_blocks = unsafe {
            slice::from_raw_parts(
                child_shard_blocks.as_ptr().cast::<[u8; BlockHash::SIZE]>(),
                num_blocks,
            )
        };
        let child_shard_blocks = BlockHash::slice_from_repr(child_shard_blocks);

        Some((Self { child_shard_blocks }, bytes))
    }

    /// Compute Merkle Tree with child shard blocks, part of the eventual block hash.
    ///
    /// `None` is returned if there are no child shard blocks.
    pub fn root(&self) -> Option<Blake3Hash> {
        let root = UnbalancedHashedMerkleTree::compute_root_only::<'_, { u32::MAX as usize }, _, _>(
            // TODO: Keyed hash
            self.child_shard_blocks.iter().map(|child_shard_block| {
                // Hash the hash again so we can prove it, otherwise headers root is
                // indistinguishable from individual block hashes and can be used to confuse
                // verifier

                blake3::hash(child_shard_block.as_ref())
            }),
        )?;
        Some(Blake3Hash::new(root))
    }
}

/// Block header result.
///
/// The result contains information that can only be computed after the block was created.
#[derive(Debug, Copy, Clone, Eq, PartialEq, TrivialType)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[repr(C)]
pub struct BlockHeaderResult {
    /// Root of the block body
    // TODO: New type
    pub body_root: Blake3Hash,
    /// Root of the state tree
    // TODO: New type?
    pub state_root: Blake3Hash,
}

impl BlockHeaderResult {
    /// Hash of the block header result, part of the eventual block hash
    pub fn hash(&self) -> Blake3Hash {
        // TODO: Keyed hash
        Blake3Hash::from(blake3::hash(self.as_bytes()))
    }
}

/// Block header seal type
#[derive(Debug, Copy, Clone, Eq, PartialEq, TrivialType)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[repr(u8)]
pub enum BlockHeaderSealType {
    /// Sr25519 signature
    #[cfg_attr(feature = "scale-codec", codec(index = 0))]
    Sr25519 = 0,
}

impl BlockHeaderSealType {
    /// Create an instance from bytes if valid
    #[inline(always)]
    pub const fn try_from_byte(byte: u8) -> Option<Self> {
        if byte == Self::Sr25519 as u8 {
            Some(Self::Sr25519)
        } else {
            None
        }
    }
}

/// Sr25519 seal
#[derive(Debug, Copy, Clone, Eq, PartialEq, TrivialType)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[repr(C)]
pub struct BlockHeaderSr25519Seal {
    /// Sr25519 public key
    /// TODO: Probably use constant from `schnorrkel`
    pub public_key: [u8; 32],
    /// Sr25519 signature
    /// TODO: Probably use constant from `schnorrkel`
    #[cfg_attr(feature = "serde", serde(with = "BigArray"))]
    pub signature: [u8; 64],
}

/// Block header seal
#[derive(Debug, Copy, Clone)]
pub enum BlockHeaderSeal<'a> {
    /// Sr25519 seal
    Sr25519(&'a BlockHeaderSr25519Seal),
}

impl<'a> BlockHeaderSeal<'a> {
    /// Create an instance from provided bytes.
    ///
    /// `bytes` do not need to be aligned.
    ///
    /// Returns an instance and remaining bytes on success.
    #[inline]
    pub fn try_from_bytes(mut bytes: &'a [u8]) -> Option<(Self, &'a [u8])> {
        // The layout here is as follows:
        // * seal type: u8
        // * seal (depends on a seal type): BlockHeaderSr25519Seal

        let seal_type = bytes.split_off(..size_of::<u8>())?;
        let seal_type = BlockHeaderSealType::try_from_byte(seal_type[0])?;

        match seal_type {
            BlockHeaderSealType::Sr25519 => {
                let seal = bytes.split_off(..size_of::<BlockHeaderSr25519Seal>())?;
                // SAFETY: All bit patterns are valid
                let seal = unsafe { BlockHeaderSr25519Seal::from_bytes(seal) }?;
                Some((Self::Sr25519(seal), bytes))
            }
        }
    }

    /// Hash of the block header seal, part of the eventual block hash
    pub fn hash(&self) -> Blake3Hash {
        match self {
            BlockHeaderSeal::Sr25519(seal) => {
                // TODO: Keyed hash
                let mut hasher = blake3::Hasher::new();
                hasher.update(&[BlockHeaderSealType::Sr25519 as u8]);
                hasher.update(seal.as_bytes());

                Blake3Hash::from(hasher.finalize())
            }
        }
    }
}

/// Generic block header, shared for different kinds of shards
#[derive(Debug, Copy, Clone)]
pub struct GenericBlockHeader<'a> {
    /// Block header prefix
    pub prefix: &'a BlockHeaderPrefix,
    /// Block header result
    pub result: &'a BlockHeaderResult,
    /// Consensus information
    pub consensus_info: &'a BlockHeaderConsensusInfo,
    /// Block header seal
    pub seal: BlockHeaderSeal<'a>,
}

/// Block header that corresponds to the beacon chain
#[derive(Debug, Copy, Clone)]
pub struct BeaconChainBlockHeader<'a> {
    /// Generic block header
    pub generic: GenericBlockHeader<'a>,
    /// Information about child shard blocks
    pub child_shard_blocks: BlockHeaderChildShardBlocks<'a>,
    /// Consensus parameters (on the beacon chain)
    pub consensus_parameters: BlockHeaderBeaconChainParameters<'a>,
}

impl<'a> Deref for BeaconChainBlockHeader<'a> {
    type Target = GenericBlockHeader<'a>;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.generic
    }
}

impl<'a> BeaconChainBlockHeader<'a> {
    /// Try to create a new instance from provided bytes.
    ///
    /// `bytes` should be 8-bytes aligned.
    ///
    /// Returns an instance and remaining bytes on success, `None` if too few bytes were given,
    /// bytes are not properly aligned or input is otherwise invalid.
    #[inline]
    pub fn try_from_bytes(bytes: &'a [u8]) -> Option<(Self, &'a [u8])> {
        // The layout here is as follows:
        // * block header prefix: BlockHeaderPrefix
        // * block header result: BlockHeaderResult
        // * consensus info: BlockHeaderConsensusInfo
        // * child shard blocks: BlockHeaderChildShardBlocks
        // * beacon chain parameters: BlockHeaderBeaconChainParameters
        // * block header seal: BlockHeaderSeal

        let (prefix, consensus_info, result, remainder) =
            BlockHeader::try_from_bytes_shared(bytes)?;

        let (child_shard_blocks, remainder) =
            BlockHeaderChildShardBlocks::try_from_bytes(remainder)?;

        let (consensus_parameters, remainder) =
            BlockHeaderBeaconChainParameters::try_from_bytes(remainder)?;

        let (seal, remainder) = BlockHeaderSeal::try_from_bytes(remainder)?;

        let generic = GenericBlockHeader {
            prefix,
            result,
            consensus_info,
            seal,
        };

        Some((
            Self {
                generic,
                child_shard_blocks,
                consensus_parameters,
            },
            remainder,
        ))
    }

    /// Compute block hash out of this header.
    ///
    /// Block hash is actually a Merkle Tree Root. The leaves are derived from individual fields in
    /// [`GenericBlockHeader`] and other fields of this enum in the declaration order.
    ///
    /// Note that this method does a bunch of hashing and if hash is needed often, should be cached.
    #[inline]
    pub fn hash(&self) -> BlockHash {
        let Self {
            generic,
            child_shard_blocks,
            consensus_parameters,
        } = self;
        let GenericBlockHeader {
            prefix,
            result,
            consensus_info,
            seal,
        } = generic;

        const MAX_N: usize = 6;
        let leaves: [_; MAX_N] = [
            prefix.hash(),
            result.hash(),
            consensus_info.hash(),
            seal.hash(),
            child_shard_blocks.root().unwrap_or_default(),
            consensus_parameters.hash(),
        ];
        let block_hash = UnbalancedHashedMerkleTree::compute_root_only::<MAX_N, _, _>(leaves)
            .expect("The list is not empty; qed");

        BlockHash::new(Blake3Hash::new(block_hash))
    }
}

/// Block header that corresponds to an intermediate shard
#[derive(Debug, Copy, Clone)]
pub struct IntermediateShardBlockHeader<'a> {
    /// Generic block header
    pub generic: GenericBlockHeader<'a>,
    /// Beacon chain info
    pub beacon_chain_info: &'a BlockHeaderBeaconChainInfo,
    /// Information about child shard blocks
    pub child_shard_blocks: BlockHeaderChildShardBlocks<'a>,
}

impl<'a> Deref for IntermediateShardBlockHeader<'a> {
    type Target = GenericBlockHeader<'a>;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.generic
    }
}

impl<'a> IntermediateShardBlockHeader<'a> {
    /// Try to create a new instance from provided bytes.
    ///
    /// `bytes` should be 8-bytes aligned.
    ///
    /// Returns an instance and remaining bytes on success, `None` if too few bytes were given,
    /// bytes are not properly aligned or input is otherwise invalid.
    #[inline]
    pub fn try_from_bytes(bytes: &'a [u8]) -> Option<(Self, &'a [u8])> {
        // The layout here is as follows:
        // * block header prefix: BlockHeaderPrefix
        // * block header result: BlockHeaderResult
        // * consensus info: BlockHeaderConsensusInfo
        // * beacon chain: BlockHeaderBeaconChainInfo
        // * child shard blocks: BlockHeaderBeaconChainInfo
        // * block header seal: BlockHeaderSeal

        let (prefix, consensus_info, result, mut remainder) =
            BlockHeader::try_from_bytes_shared(bytes)?;

        let beacon_chain_info = remainder.split_off(..size_of::<BlockHeaderBeaconChainInfo>())?;
        // SAFETY: All bit patterns are valid
        let beacon_chain_info =
            unsafe { BlockHeaderBeaconChainInfo::from_bytes(beacon_chain_info) }?;

        let (child_shard_blocks, remainder) =
            BlockHeaderChildShardBlocks::try_from_bytes(remainder)?;

        let (seal, remainder) = BlockHeaderSeal::try_from_bytes(remainder)?;

        let generic = GenericBlockHeader {
            prefix,
            result,
            consensus_info,
            seal,
        };

        Some((
            Self {
                generic,
                beacon_chain_info,
                child_shard_blocks,
            },
            remainder,
        ))
    }

    /// Compute block hash out of this header.
    ///
    /// Block hash is actually a Merkle Tree Root. The leaves are derived from individual fields in
    /// [`GenericBlockHeader`] and other fields of this enum in the declaration order.
    ///
    /// Note that this method does a bunch of hashing and if hash is needed often, should be cached.
    #[inline]
    pub fn hash(&self) -> BlockHash {
        let Self {
            generic,
            beacon_chain_info,
            child_shard_blocks,
        } = self;
        let GenericBlockHeader {
            prefix,
            result,
            consensus_info,
            seal,
        } = generic;

        const MAX_N: usize = 6;
        let leaves: [_; MAX_N] = [
            prefix.hash(),
            result.hash(),
            consensus_info.hash(),
            seal.hash(),
            beacon_chain_info.hash(),
            child_shard_blocks.root().unwrap_or_default(),
        ];
        let block_hash = UnbalancedHashedMerkleTree::compute_root_only::<MAX_N, _, _>(leaves)
            .expect("The list is not empty; qed");

        BlockHash::new(Blake3Hash::new(block_hash))
    }
}

/// Block header that corresponds to a leaf shard
#[derive(Debug, Copy, Clone)]
pub struct LeafShardBlockHeader<'a> {
    /// Generic block header
    pub generic: GenericBlockHeader<'a>,
    /// Beacon chain info
    pub beacon_chain_info: &'a BlockHeaderBeaconChainInfo,
}

impl<'a> Deref for LeafShardBlockHeader<'a> {
    type Target = GenericBlockHeader<'a>;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.generic
    }
}

impl<'a> LeafShardBlockHeader<'a> {
    /// Try to create a new instance from provided bytes.
    ///
    /// `bytes` should be 8-bytes aligned.
    ///
    /// Returns an instance and remaining bytes on success, `None` if too few bytes were given,
    /// bytes are not properly aligned or input is otherwise invalid.
    #[inline]
    pub fn try_from_bytes(bytes: &'a [u8]) -> Option<(Self, &'a [u8])> {
        // The layout here is as follows:
        // * block header result: BlockHeaderResult
        // * block header prefix: BlockHeaderPrefix
        // * consensus info: BlockHeaderConsensusInfo
        // * beacon chain: BlockHeaderBeaconChainInfo
        // * block header seal: BlockHeaderSeal

        let (prefix, consensus_info, result, mut remainder) =
            BlockHeader::try_from_bytes_shared(bytes)?;

        let beacon_chain_info = remainder.split_off(..size_of::<BlockHeaderBeaconChainInfo>())?;
        // SAFETY: All bit patterns are valid
        let beacon_chain_info =
            unsafe { BlockHeaderBeaconChainInfo::from_bytes(beacon_chain_info) }?;

        let (seal, remainder) = BlockHeaderSeal::try_from_bytes(remainder)?;

        let generic = GenericBlockHeader {
            prefix,
            result,
            consensus_info,
            seal,
        };

        Some((
            Self {
                generic,
                beacon_chain_info,
            },
            remainder,
        ))
    }

    /// Compute block hash out of this header.
    ///
    /// Block hash is actually a Merkle Tree Root. The leaves are derived from individual fields in
    /// [`GenericBlockHeader`] and other fields of this enum in the declaration order.
    ///
    /// Note that this method does a bunch of hashing and if hash is needed often, should be cached.
    #[inline]
    pub fn hash(&self) -> BlockHash {
        let Self {
            generic,
            beacon_chain_info,
        } = self;
        let GenericBlockHeader {
            prefix,
            result,
            consensus_info,
            seal,
        } = generic;

        const MAX_N: usize = 5;
        let leaves: [_; MAX_N] = [
            prefix.hash(),
            result.hash(),
            consensus_info.hash(),
            seal.hash(),
            beacon_chain_info.hash(),
        ];
        let block_hash = UnbalancedHashedMerkleTree::compute_root_only::<MAX_N, _, _>(leaves)
            .expect("The list is not empty; qed");

        BlockHash::new(Blake3Hash::new(block_hash))
    }
}

/// Block header that together with [`BlockBody`] form a [`Block`]
///
/// [`BlockBody`]: crate::block::body::BlockBody
/// [`Block`]: crate::block::Block
#[derive(Debug, Copy, Clone, From)]
pub enum BlockHeader<'a> {
    /// Block header corresponds to the beacon chain
    BeaconChain(BeaconChainBlockHeader<'a>),
    /// Block header corresponds to an intermediate shard
    IntermediateShard(IntermediateShardBlockHeader<'a>),
    /// Block header corresponds to a leaf shard
    LeafShard(LeafShardBlockHeader<'a>),
}

impl<'a> Deref for BlockHeader<'a> {
    type Target = GenericBlockHeader<'a>;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        match self {
            Self::BeaconChain(header) => header,
            Self::IntermediateShard(header) => header,
            Self::LeafShard(header) => header,
        }
    }
}

impl<'a> BlockHeader<'a> {
    /// Try to create a new instance from provided bytes for provided shard index.
    ///
    /// `bytes` should be 8-bytes aligned.
    ///
    /// Returns an instance and remaining bytes on success, `None` if too few bytes were given,
    /// bytes are not properly aligned or input is otherwise invalid.
    #[inline]
    pub fn try_from_bytes(bytes: &'a [u8], shard_kind: ShardKind) -> Option<(Self, &'a [u8])> {
        match shard_kind {
            ShardKind::BeaconChain => {
                let (header, remainder) = BeaconChainBlockHeader::try_from_bytes(bytes)?;
                Some((Self::BeaconChain(header), remainder))
            }
            ShardKind::IntermediateShard => {
                let (header, remainder) = IntermediateShardBlockHeader::try_from_bytes(bytes)?;
                Some((Self::IntermediateShard(header), remainder))
            }
            ShardKind::LeafShard => {
                let (header, remainder) = LeafShardBlockHeader::try_from_bytes(bytes)?;
                Some((Self::LeafShard(header), remainder))
            }
            ShardKind::Phantom | ShardKind::Invalid => {
                // Blocks for such shards do not exist
                None
            }
        }
    }

    #[inline]
    fn try_from_bytes_shared(
        mut bytes: &'a [u8],
    ) -> Option<(
        &'a BlockHeaderPrefix,
        &'a BlockHeaderConsensusInfo,
        &'a BlockHeaderResult,
        &'a [u8],
    )> {
        let prefix = bytes.split_off(..size_of::<BlockHeaderPrefix>())?;
        // SAFETY: All bit patterns are valid
        let prefix = unsafe { BlockHeaderPrefix::from_bytes(prefix) }?;

        if !(prefix.version == BlockHeaderPrefix::BLOCK_VERSION
            && prefix.padding == [0; _]
            && prefix.shard_index.as_u32() <= ShardIndex::MAX_SHARD_INDEX)
        {
            return None;
        }

        let result = bytes.split_off(..size_of::<BlockHeaderResult>())?;
        // SAFETY: All bit patterns are valid
        let result = unsafe { BlockHeaderResult::from_bytes(result) }?;

        let consensus_info = bytes.split_off(..size_of::<BlockHeaderConsensusInfo>())?;
        // SAFETY: All bit patterns are valid
        let consensus_info = unsafe { BlockHeaderConsensusInfo::from_bytes(consensus_info) }?;

        if consensus_info.solution.padding != [0; _] {
            return None;
        }

        Some((prefix, consensus_info, result, bytes))
    }

    /// Compute block hash out of this header.
    ///
    /// Block hash is actually a Merkle Tree Root. The leaves are derived from individual fields in
    /// [`GenericBlockHeader`] and other fields of this enum in the declaration order.
    ///
    /// Note that this method does a bunch of hashing and if hash is needed often, should be cached.
    #[inline]
    pub fn hash(&self) -> BlockHash {
        // TODO: Should unique keyed hash be used for different kinds of shards?
        match self {
            Self::BeaconChain(header) => header.hash(),
            Self::IntermediateShard(header) => header.hash(),
            Self::LeafShard(header) => header.hash(),
        }
    }
}
