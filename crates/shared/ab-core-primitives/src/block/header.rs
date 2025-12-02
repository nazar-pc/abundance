//! Block header primitives

#[cfg(feature = "alloc")]
pub mod owned;

#[cfg(feature = "alloc")]
use crate::block::header::owned::{
    GenericOwnedBlockHeader, OwnedBeaconChainHeader, OwnedBlockHeader,
    OwnedIntermediateShardHeader, OwnedLeafShardHeader,
};
use crate::block::{BlockNumber, BlockRoot, BlockTimestamp};
use crate::ed25519::{Ed25519PublicKey, Ed25519Signature};
use crate::hashes::Blake3Hash;
use crate::pot::{PotOutput, PotParametersChange, SlotNumber};
use crate::segments::SuperSegmentRoot;
use crate::shard::{ShardIndex, ShardKind};
use crate::solutions::{Solution, SolutionRange};
use ab_blake3::{BLOCK_LEN, single_block_hash, single_chunk_hash};
use ab_io_type::trivial_type::TrivialType;
use ab_merkle_tree::unbalanced::UnbalancedMerkleTree;
use blake3::CHUNK_LEN;
use core::num::NonZeroU32;
use core::ops::Deref;
use core::{fmt, slice};
use derive_more::{Deref, From};
#[cfg(feature = "scale-codec")]
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
#[cfg(feature = "scale-codec")]
use scale_info::TypeInfo;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use yoke::Yokeable;

/// Generic block header
pub trait GenericBlockHeader<'a>
where
    Self: Clone
        + fmt::Debug
        + Deref<Target = SharedBlockHeader<'a>>
        + Into<BlockHeader<'a>>
        + Send
        + Sync,
{
    /// Shard kind
    const SHARD_KIND: ShardKind;

    /// Owned block header
    #[cfg(feature = "alloc")]
    type Owned: GenericOwnedBlockHeader<Header<'a> = Self>
    where
        Self: 'a;

    /// Turn into an owned version
    #[cfg(feature = "alloc")]
    fn to_owned(self) -> Self::Owned;

    /// Compute block root out of this header.
    ///
    /// Block root is a Merkle Tree Root. The leaves are derived from individual fields in
    /// [`SharedBlockHeader`] and other fields of this enum in the declaration order.
    ///
    /// Note that this method does a bunch of hashing and if root is often needed, should be cached.
    fn root(&self) -> impl Deref<Target = BlockRoot> + Send + Sync;

    /// Hash of the block before seal is applied to it
    fn pre_seal_hash(&self) -> Blake3Hash;
}

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
    /// Block number
    pub number: BlockNumber,
    /// Shard index
    pub shard_index: ShardIndex,
    /// Padding for data structure alignment, contents must be all zeroes
    pub padding_0: [u8; 4],
    /// Block timestamp
    pub timestamp: BlockTimestamp,
    /// Root of the parent block
    pub parent_root: BlockRoot,
    /// MMR root of all block roots, including `parent_root`
    // TODO: New type?
    pub mmr_root: Blake3Hash,
}

impl BlockHeaderPrefix {
    /// Hash of the block header prefix, part of the eventual block root
    pub fn hash(&self) -> Blake3Hash {
        const {
            assert!(size_of::<Self>() <= CHUNK_LEN);
        }
        // TODO: Keyed hash
        Blake3Hash::new(
            single_chunk_hash(self.as_bytes())
                .expect("Less than a single chunk worth of bytes; qed"),
        )
    }
}

/// Consensus information in the block header
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
    /// Hash of the consensus info, part of the eventual block root
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
    /// Beacon chain block root
    pub root: BlockRoot,
}

impl BlockHeaderBeaconChainInfo {
    /// Hash of the beacon chain info, part of the eventual block root
    pub fn hash(&self) -> Blake3Hash {
        const {
            assert!(size_of::<Self>() <= BLOCK_LEN);
        }
        // TODO: Keyed hash
        Blake3Hash::new(
            single_block_hash(self.as_bytes())
                .expect("Less than a single block worth of bytes; qed"),
        )
    }
}

/// Consensus parameters (on the beacon chain)
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
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
    pub slot_iterations: NonZeroU32,
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
        let slot_iterations = u32::from_le_bytes([
            pot_slot_iterations[0],
            pot_slot_iterations[1],
            pot_slot_iterations[2],
            pot_slot_iterations[3],
        ]);
        let slot_iterations = NonZeroU32::new(slot_iterations)?;

        Some((
            Self {
                solution_range,
                slot_iterations,
            },
            bytes,
        ))
    }
}

/// A mirror of [`PotParametersChange`] for block header purposes.
///
/// Use [`From`] or [`Into`] for converting into [`PotParametersChange`] before use.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
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

impl From<PotParametersChange> for BlockHeaderPotParametersChange {
    #[inline(always)]
    fn from(value: PotParametersChange) -> Self {
        let PotParametersChange {
            slot,
            slot_iterations,
            entropy,
        } = value;

        BlockHeaderPotParametersChange {
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

/// Owned version of [`BlockHeaderConsensusParameters`]
#[derive(Debug, Copy, Clone)]
pub struct OwnedBlockHeaderConsensusParameters {
    /// Consensus parameters that are always present
    pub fixed_parameters: BlockHeaderFixedConsensusParameters,
    /// Super segment root
    pub super_segment_root: Option<SuperSegmentRoot>,
    /// Solution range for the next block/era (if any)
    pub next_solution_range: Option<SolutionRange>,
    /// Change of parameters to apply to the proof of time chain (if any)
    pub pot_parameters_change: Option<BlockHeaderPotParametersChange>,
}

impl OwnedBlockHeaderConsensusParameters {
    /// Get a reference out of owned version
    #[inline]
    pub fn as_ref(&self) -> BlockHeaderConsensusParameters<'_> {
        BlockHeaderConsensusParameters {
            fixed_parameters: self.fixed_parameters,
            super_segment_root: self.super_segment_root.as_ref(),
            next_solution_range: self.next_solution_range,
            pot_parameters_change: self.pot_parameters_change.as_ref(),
        }
    }
}

/// Consensus parameters (on the beacon chain)
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct BlockHeaderConsensusParameters<'a> {
    /// Consensus parameters that are always present
    pub fixed_parameters: BlockHeaderFixedConsensusParameters,
    /// Super segment root
    pub super_segment_root: Option<&'a SuperSegmentRoot>,
    /// Solution range for the next block/era (if any)
    pub next_solution_range: Option<SolutionRange>,
    /// Change of parameters to apply to the proof of time chain (if any)
    pub pot_parameters_change: Option<&'a BlockHeaderPotParametersChange>,
}

impl<'a> BlockHeaderConsensusParameters<'a> {
    /// Max size of the allocation necessary for this data structure
    pub const MAX_SIZE: u32 = size_of::<BlockHeaderFixedConsensusParameters>() as u32
        + u8::SIZE
        + <SuperSegmentRoot as TrivialType>::SIZE
        + <SolutionRange as TrivialType>::SIZE
        + size_of::<BlockHeaderPotParametersChange>() as u32;
    /// Bitmask for presence of `super_segment_root` field
    pub const SUPER_SEGMENT_ROOT_MASK: u8 = 0b_0000_0001;
    /// Bitmask for presence of `next_solution_range` field
    pub const NEXT_SOLUTION_RANGE_MASK: u8 = 0b_0000_0010;
    /// Bitmask for presence of `pot_parameters_change` field
    pub const POT_PARAMETERS_CHANGE_MASK: u8 = 0b_0000_0100;
    /// All supported bitmask variants
    pub const MASK_ALL: u8 = Self::SUPER_SEGMENT_ROOT_MASK
        | Self::NEXT_SOLUTION_RANGE_MASK
        | Self::POT_PARAMETERS_CHANGE_MASK;

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

        if (bitflags & Self::MASK_ALL) != bitflags {
            // Unexpected bitflags were set
            return None;
        }

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

    /// Hash of the block consensus parameters, part of the eventual block root
    pub fn hash(&self) -> Blake3Hash {
        let Self {
            super_segment_root,
            fixed_parameters,
            next_solution_range,
            pot_parameters_change,
        } = self;
        let BlockHeaderFixedConsensusParameters {
            solution_range,
            slot_iterations,
        } = fixed_parameters;

        // TODO: Keyed hash
        let mut hasher = blake3::Hasher::new();
        hasher.update(solution_range.as_bytes());
        hasher.update(&slot_iterations.get().to_le_bytes());

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
    pub child_shard_blocks: &'a [BlockRoot],
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

        let child_shard_blocks = bytes.split_off(..num_blocks * BlockRoot::SIZE)?;
        // SAFETY: Valid pointer and size, no alignment requirements
        let child_shard_blocks = unsafe {
            slice::from_raw_parts(
                child_shard_blocks.as_ptr().cast::<[u8; BlockRoot::SIZE]>(),
                num_blocks,
            )
        };
        let child_shard_blocks = BlockRoot::slice_from_repr(child_shard_blocks);

        Some((Self { child_shard_blocks }, bytes))
    }

    /// Compute Merkle Tree with child shard blocks, part of the eventual block root.
    ///
    /// `None` is returned if there are no child shard blocks.
    pub fn root(&self) -> Option<Blake3Hash> {
        let root = UnbalancedMerkleTree::compute_root_only::<'_, { u32::MAX as u64 }, _, _>(
            // TODO: Keyed hash
            self.child_shard_blocks
                .iter()
                .map(|child_shard_block_root| {
                    // Hash the root again so we can prove it, otherwise headers root is
                    // indistinguishable from individual block roots and can be used to confuse
                    // verifier
                    single_block_hash(child_shard_block_root.as_ref())
                        .expect("Less than a single block worth of bytes; qed")
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
    /// Hash of the block header result, part of the eventual block root
    pub fn hash(&self) -> Blake3Hash {
        const {
            assert!(size_of::<Self>() <= BLOCK_LEN);
        }
        // TODO: Keyed hash
        Blake3Hash::new(
            single_block_hash(self.as_bytes())
                .expect("Less than a single block worth of bytes; qed"),
        )
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
#[non_exhaustive]
pub enum BlockHeaderSealType {
    /// Ed25519 signature
    #[cfg_attr(feature = "scale-codec", codec(index = 0))]
    Ed25519 = 0,
}

impl BlockHeaderSealType {
    /// Create an instance from bytes if valid
    #[inline(always)]
    pub const fn try_from_byte(byte: u8) -> Option<Self> {
        if byte == Self::Ed25519 as u8 {
            Some(Self::Ed25519)
        } else {
            None
        }
    }
}

/// Ed25519 seal
#[derive(Debug, Copy, Clone, Eq, PartialEq, TrivialType)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[repr(C)]
pub struct BlockHeaderEd25519Seal {
    /// Ed25519 public key
    pub public_key: Ed25519PublicKey,
    /// Ed25519 signature
    pub signature: Ed25519Signature,
}

/// Owned version of [`BlockHeaderSeal`]
#[derive(Debug, Copy, Clone)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[non_exhaustive]
pub enum OwnedBlockHeaderSeal {
    /// Ed25519 seal
    Ed25519(BlockHeaderEd25519Seal),
}

impl OwnedBlockHeaderSeal {
    /// Get a reference out of owned version
    #[inline(always)]
    pub fn as_ref(&self) -> BlockHeaderSeal<'_> {
        match self {
            Self::Ed25519(seal) => BlockHeaderSeal::Ed25519(seal),
        }
    }
}

/// Block header seal
#[derive(Debug, Copy, Clone)]
#[non_exhaustive]
pub enum BlockHeaderSeal<'a> {
    /// Ed25519 seal
    Ed25519(&'a BlockHeaderEd25519Seal),
}

impl<'a> BlockHeaderSeal<'a> {
    /// Max size of the allocation necessary for this data structure
    pub const MAX_SIZE: u32 = 1 + BlockHeaderEd25519Seal::SIZE;
    /// Create an instance from provided bytes.
    ///
    /// `bytes` do not need to be aligned.
    ///
    /// Returns an instance and remaining bytes on success.
    #[inline]
    pub fn try_from_bytes(mut bytes: &'a [u8]) -> Option<(Self, &'a [u8])> {
        // The layout here is as follows:
        // * seal type: u8
        // * seal (depends on a seal type): BlockHeaderEd25519Seal

        let seal_type = bytes.split_off(..size_of::<u8>())?;
        let seal_type = BlockHeaderSealType::try_from_byte(seal_type[0])?;

        match seal_type {
            BlockHeaderSealType::Ed25519 => {
                let seal = bytes.split_off(..size_of::<BlockHeaderEd25519Seal>())?;
                // SAFETY: All bit patterns are valid
                let seal = unsafe { BlockHeaderEd25519Seal::from_bytes(seal) }?;
                Some((Self::Ed25519(seal), bytes))
            }
        }
    }

    /// Verify seal against [`BlockHeader::pre_seal_hash()`]
    #[inline]
    pub fn is_seal_valid(&self, pre_seal_hash: &Blake3Hash) -> bool {
        match self {
            BlockHeaderSeal::Ed25519(seal) => seal
                .public_key
                .verify(&seal.signature, pre_seal_hash.as_bytes())
                .is_ok(),
        }
    }

    /// Derive public key hash from this seal
    #[inline]
    pub fn public_key_hash(&self) -> Blake3Hash {
        match self {
            BlockHeaderSeal::Ed25519(seal) => seal.public_key.hash(),
        }
    }

    /// Hash of the block header seal, part of the eventual block root
    #[inline]
    pub fn hash(&self) -> Blake3Hash {
        match self {
            BlockHeaderSeal::Ed25519(seal) => {
                // TODO: Keyed hash
                let mut hasher = blake3::Hasher::new();
                hasher.update(&[BlockHeaderSealType::Ed25519 as u8]);
                hasher.update(seal.as_bytes());

                Blake3Hash::from(hasher.finalize())
            }
        }
    }
}

/// Part of the block header, shared for different kinds of shards
#[derive(Debug, Copy, Clone)]
pub struct SharedBlockHeader<'a> {
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
#[derive(Debug, Clone, Yokeable)]
// Prevent creation of potentially broken invariants externally
#[non_exhaustive]
pub struct BeaconChainHeader<'a> {
    /// Shared block header
    shared: SharedBlockHeader<'a>,
    /// Information about child shard blocks
    child_shard_blocks: BlockHeaderChildShardBlocks<'a>,
    /// Consensus parameters (on the beacon chain)
    consensus_parameters: BlockHeaderConsensusParameters<'a>,
    /// All bytes of the header except the seal
    pre_seal_bytes: &'a [u8],
    #[cfg(all(feature = "alloc", any(target_os = "none", target_os = "unknown")))]
    cached_block_root: rclite::Arc<once_cell::race::OnceBox<BlockRoot>>,
    #[cfg(not(any(target_os = "none", target_os = "unknown")))]
    cached_block_root: rclite::Arc<std::sync::OnceLock<BlockRoot>>,
}

impl<'a> Deref for BeaconChainHeader<'a> {
    type Target = SharedBlockHeader<'a>;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.shared
    }
}

impl<'a> GenericBlockHeader<'a> for BeaconChainHeader<'a> {
    const SHARD_KIND: ShardKind = ShardKind::BeaconChain;

    #[cfg(feature = "alloc")]
    type Owned = OwnedBeaconChainHeader;

    #[cfg(feature = "alloc")]
    #[inline(always)]
    fn to_owned(self) -> Self::Owned {
        self.to_owned()
    }

    #[inline(always)]
    fn root(&self) -> impl Deref<Target = BlockRoot> + Send + Sync {
        self.root()
    }

    #[inline(always)]
    fn pre_seal_hash(&self) -> Blake3Hash {
        self.pre_seal_hash()
    }
}

impl<'a> BeaconChainHeader<'a> {
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

        if prefix.shard_index.shard_kind() != ShardKind::BeaconChain {
            return None;
        }

        let (child_shard_blocks, remainder) =
            BlockHeaderChildShardBlocks::try_from_bytes(remainder)?;

        let (consensus_parameters, remainder) =
            BlockHeaderConsensusParameters::try_from_bytes(remainder)?;

        let pre_seal_bytes = &bytes[..bytes.len() - remainder.len()];

        let (seal, remainder) = BlockHeaderSeal::try_from_bytes(remainder)?;

        let shared = SharedBlockHeader {
            prefix,
            result,
            consensus_info,
            seal,
        };

        let header = Self {
            shared,
            child_shard_blocks,
            consensus_parameters,
            pre_seal_bytes,
            #[cfg(any(feature = "alloc", not(any(target_os = "none", target_os = "unknown"))))]
            cached_block_root: rclite::Arc::default(),
        };

        if !header.is_internally_consistent() {
            return None;
        }

        Some((header, remainder))
    }

    /// Check block header's internal consistency.
    ///
    /// This is usually not necessary to be called explicitly since internal consistency is checked
    /// by [`Self::try_from_bytes()`] internally.
    #[inline]
    pub fn is_internally_consistent(&self) -> bool {
        let public_key_hash = match self.seal {
            BlockHeaderSeal::Ed25519(seal) => seal.public_key.hash(),
        };
        public_key_hash == self.shared.consensus_info.solution.public_key_hash
    }

    /// The same as [`Self::try_from_bytes()`], but for trusted input that skips some consistency
    /// checks
    #[inline]
    pub fn try_from_bytes_unchecked(bytes: &'a [u8]) -> Option<(Self, &'a [u8])> {
        // The layout here is as follows:
        // * block header prefix: BlockHeaderPrefix
        // * block header result: BlockHeaderResult
        // * consensus info: BlockHeaderConsensusInfo
        // * child shard blocks: BlockHeaderChildShardBlocks
        // * beacon chain parameters: BlockHeaderBeaconChainParameters
        // * block header seal: BlockHeaderSeal

        let (prefix, consensus_info, result, remainder) =
            BlockHeader::try_from_bytes_shared(bytes)?;

        if prefix.shard_index.shard_kind() != ShardKind::BeaconChain {
            return None;
        }

        let (child_shard_blocks, remainder) =
            BlockHeaderChildShardBlocks::try_from_bytes(remainder)?;

        let (consensus_parameters, remainder) =
            BlockHeaderConsensusParameters::try_from_bytes(remainder)?;

        let pre_seal_bytes = &bytes[..bytes.len() - remainder.len()];

        let (seal, remainder) = BlockHeaderSeal::try_from_bytes(remainder)?;

        let shared = SharedBlockHeader {
            prefix,
            result,
            consensus_info,
            seal,
        };

        Some((
            Self {
                shared,
                child_shard_blocks,
                consensus_parameters,
                pre_seal_bytes,
                #[cfg(any(
                    feature = "alloc",
                    not(any(target_os = "none", target_os = "unknown"))
                ))]
                cached_block_root: rclite::Arc::default(),
            },
            remainder,
        ))
    }

    /// Create an owned version of this header
    #[cfg(feature = "alloc")]
    #[inline(always)]
    pub fn to_owned(self) -> OwnedBeaconChainHeader {
        let unsealed = OwnedBeaconChainHeader::from_parts(
            self.shared.prefix,
            self.shared.result,
            self.shared.consensus_info,
            &self.child_shard_blocks,
            self.consensus_parameters,
        )
        .expect("`self` is always a valid invariant; qed");

        unsealed.with_seal(self.shared.seal)
    }

    /// Shared block header
    #[inline(always)]
    pub fn shared(&self) -> &SharedBlockHeader<'a> {
        &self.shared
    }

    /// Information about child shard blocks
    #[inline(always)]
    pub fn child_shard_blocks(&self) -> &BlockHeaderChildShardBlocks<'a> {
        &self.child_shard_blocks
    }

    /// Consensus parameters (on the beacon chain)
    #[inline(always)]
    pub fn consensus_parameters(&self) -> &BlockHeaderConsensusParameters<'a> {
        &self.consensus_parameters
    }

    /// Hash of the block before seal is applied to it
    #[inline]
    pub fn pre_seal_hash(&self) -> Blake3Hash {
        // TODO: Keyed hash with `block_header_seal` as a key
        Blake3Hash::from(blake3::hash(self.pre_seal_bytes))
    }

    /// Verify seal against [`BeaconChainHeader::pre_seal_hash()`] and check that its public key
    /// hash corresponds to the solution
    #[inline]
    pub fn is_sealed_correctly(&self) -> bool {
        self.consensus_info.solution.public_key_hash == self.seal.public_key_hash()
            && self.seal.is_seal_valid(&self.pre_seal_hash())
    }

    /// Compute block root out of this header.
    ///
    /// Block root is a Merkle Tree Root. The leaves are derived from individual fields in
    /// [`SharedBlockHeader`] and other fields of this enum in the declaration order.
    ///
    /// Note that this method computes root by doing a bunch of hashing. The result is then cached
    /// if `alloc` feature is enabled or when compiled for OS target that is not `none`.
    #[inline]
    pub fn root(&self) -> impl Deref<Target = BlockRoot> + Send + Sync {
        let Self {
            shared,
            child_shard_blocks,
            consensus_parameters,
            pre_seal_bytes: _,
            #[cfg(any(feature = "alloc", not(any(target_os = "none", target_os = "unknown"))))]
            cached_block_root,
        } = self;

        let compute_root = || {
            let SharedBlockHeader {
                prefix,
                result,
                consensus_info,
                seal,
            } = shared;

            const MAX_N: usize = 6;
            let leaves: [_; MAX_N] = [
                prefix.hash(),
                result.hash(),
                consensus_info.hash(),
                seal.hash(),
                child_shard_blocks.root().unwrap_or_default(),
                consensus_parameters.hash(),
            ];
            let block_root =
                UnbalancedMerkleTree::compute_root_only::<{ MAX_N as u64 }, _, _>(leaves)
                    .expect("The list is not empty; qed");

            BlockRoot::new(Blake3Hash::new(block_root))
        };

        #[cfg(not(any(target_os = "none", target_os = "unknown")))]
        {
            cached_block_root.get_or_init(compute_root)
        }
        #[cfg(all(feature = "alloc", any(target_os = "none", target_os = "unknown")))]
        {
            cached_block_root.get_or_init(|| alloc::boxed::Box::new(compute_root()))
        }
        #[cfg(all(not(feature = "alloc"), any(target_os = "none", target_os = "unknown")))]
        {
            struct Wrapper(BlockRoot);

            impl Deref for Wrapper {
                type Target = BlockRoot;

                #[inline(always)]
                fn deref(&self) -> &Self::Target {
                    &self.0
                }
            }

            Wrapper(compute_root())
        }
    }
}

/// Block header that corresponds to an intermediate shard
#[derive(Debug, Clone, Yokeable)]
// Prevent creation of potentially broken invariants externally
#[non_exhaustive]
pub struct IntermediateShardHeader<'a> {
    /// Shared block header
    shared: SharedBlockHeader<'a>,
    /// Beacon chain info
    beacon_chain_info: &'a BlockHeaderBeaconChainInfo,
    /// Information about child shard blocks
    child_shard_blocks: BlockHeaderChildShardBlocks<'a>,
    /// All bytes of the header except the seal
    pre_seal_bytes: &'a [u8],
    #[cfg(all(feature = "alloc", any(target_os = "none", target_os = "unknown")))]
    cached_block_root: rclite::Arc<once_cell::race::OnceBox<BlockRoot>>,
    #[cfg(not(any(target_os = "none", target_os = "unknown")))]
    cached_block_root: rclite::Arc<std::sync::OnceLock<BlockRoot>>,
}

impl<'a> Deref for IntermediateShardHeader<'a> {
    type Target = SharedBlockHeader<'a>;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.shared
    }
}

impl<'a> GenericBlockHeader<'a> for IntermediateShardHeader<'a> {
    const SHARD_KIND: ShardKind = ShardKind::IntermediateShard;

    #[cfg(feature = "alloc")]
    type Owned = OwnedIntermediateShardHeader;

    #[cfg(feature = "alloc")]
    #[inline(always)]
    fn to_owned(self) -> Self::Owned {
        self.to_owned()
    }

    #[inline(always)]
    fn root(&self) -> impl Deref<Target = BlockRoot> + Send + Sync {
        self.root()
    }

    #[inline(always)]
    fn pre_seal_hash(&self) -> Blake3Hash {
        self.pre_seal_hash()
    }
}

impl<'a> IntermediateShardHeader<'a> {
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

        if prefix.shard_index.shard_kind() != ShardKind::IntermediateShard {
            return None;
        }

        let beacon_chain_info = remainder.split_off(..size_of::<BlockHeaderBeaconChainInfo>())?;
        // SAFETY: All bit patterns are valid
        let beacon_chain_info =
            unsafe { BlockHeaderBeaconChainInfo::from_bytes(beacon_chain_info) }?;

        let (child_shard_blocks, remainder) =
            BlockHeaderChildShardBlocks::try_from_bytes(remainder)?;

        let pre_seal_bytes = &bytes[..bytes.len() - remainder.len()];

        let (seal, remainder) = BlockHeaderSeal::try_from_bytes(remainder)?;

        let shared = SharedBlockHeader {
            prefix,
            result,
            consensus_info,
            seal,
        };

        let header = Self {
            shared,
            beacon_chain_info,
            child_shard_blocks,
            pre_seal_bytes,
            #[cfg(any(feature = "alloc", not(any(target_os = "none", target_os = "unknown"))))]
            cached_block_root: rclite::Arc::default(),
        };

        if !header.is_internally_consistent() {
            return None;
        }

        Some((header, remainder))
    }

    /// Check block header's internal consistency.
    ///
    /// This is usually not necessary to be called explicitly since internal consistency is checked
    /// by [`Self::try_from_bytes()`] internally.
    #[inline]
    pub fn is_internally_consistent(&self) -> bool {
        let public_key_hash = match self.seal {
            BlockHeaderSeal::Ed25519(seal) => seal.public_key.hash(),
        };
        public_key_hash == self.shared.consensus_info.solution.public_key_hash
    }

    /// The same as [`Self::try_from_bytes()`], but for trusted input that skips some consistency
    /// checks
    #[inline]
    pub fn try_from_bytes_unchecked(bytes: &'a [u8]) -> Option<(Self, &'a [u8])> {
        // The layout here is as follows:
        // * block header prefix: BlockHeaderPrefix
        // * block header result: BlockHeaderResult
        // * consensus info: BlockHeaderConsensusInfo
        // * beacon chain: BlockHeaderBeaconChainInfo
        // * child shard blocks: BlockHeaderBeaconChainInfo
        // * block header seal: BlockHeaderSeal

        let (prefix, consensus_info, result, mut remainder) =
            BlockHeader::try_from_bytes_shared(bytes)?;

        if prefix.shard_index.shard_kind() != ShardKind::IntermediateShard {
            return None;
        }

        let beacon_chain_info = remainder.split_off(..size_of::<BlockHeaderBeaconChainInfo>())?;
        // SAFETY: All bit patterns are valid
        let beacon_chain_info =
            unsafe { BlockHeaderBeaconChainInfo::from_bytes(beacon_chain_info) }?;

        let (child_shard_blocks, remainder) =
            BlockHeaderChildShardBlocks::try_from_bytes(remainder)?;

        let pre_seal_bytes = &bytes[..bytes.len() - remainder.len()];

        let (seal, remainder) = BlockHeaderSeal::try_from_bytes(remainder)?;

        let shared = SharedBlockHeader {
            prefix,
            result,
            consensus_info,
            seal,
        };

        Some((
            Self {
                shared,
                beacon_chain_info,
                child_shard_blocks,
                pre_seal_bytes,
                #[cfg(any(
                    feature = "alloc",
                    not(any(target_os = "none", target_os = "unknown"))
                ))]
                cached_block_root: rclite::Arc::default(),
            },
            remainder,
        ))
    }

    /// Create an owned version of this header
    #[cfg(feature = "alloc")]
    #[inline(always)]
    pub fn to_owned(self) -> OwnedIntermediateShardHeader {
        let unsealed = OwnedIntermediateShardHeader::from_parts(
            self.shared.prefix,
            self.shared.result,
            self.shared.consensus_info,
            self.beacon_chain_info,
            &self.child_shard_blocks,
        )
        .expect("`self` is always a valid invariant; qed");

        unsealed.with_seal(self.shared.seal)
    }

    /// Shared block header
    #[inline(always)]
    pub fn shared(&self) -> &SharedBlockHeader<'a> {
        &self.shared
    }

    /// Beacon chain info
    #[inline(always)]
    pub fn beacon_chain_info(&self) -> &'a BlockHeaderBeaconChainInfo {
        self.beacon_chain_info
    }

    /// Information about child shard blocks
    #[inline(always)]
    pub fn child_shard_blocks(&self) -> &BlockHeaderChildShardBlocks<'a> {
        &self.child_shard_blocks
    }

    /// Hash of the block before seal is applied to it
    #[inline]
    pub fn pre_seal_hash(&self) -> Blake3Hash {
        // TODO: Keyed hash with `block_header_seal` as a key
        Blake3Hash::from(blake3::hash(self.pre_seal_bytes))
    }

    /// Verify seal against [`IntermediateShardHeader::pre_seal_hash()`] and check that its public
    /// key hash corresponds to the solution
    #[inline]
    pub fn is_sealed_correctly(&self) -> bool {
        self.consensus_info.solution.public_key_hash == self.seal.public_key_hash()
            && self.seal.is_seal_valid(&self.pre_seal_hash())
    }

    /// Compute block root out of this header.
    ///
    /// Block root is a Merkle Tree Root. The leaves are derived from individual fields in
    /// [`SharedBlockHeader`] and other fields of this enum in the declaration order.
    ///
    /// Note that this method computes root by doing a bunch of hashing. The result is then cached
    /// if `alloc` feature is enabled or when compiled for OS target that is not `none`.
    #[inline]
    pub fn root(&self) -> impl Deref<Target = BlockRoot> + Send + Sync {
        let Self {
            shared,
            beacon_chain_info,
            child_shard_blocks,
            pre_seal_bytes: _,
            #[cfg(any(feature = "alloc", not(any(target_os = "none", target_os = "unknown"))))]
            cached_block_root,
        } = self;

        let compute_root = || {
            let SharedBlockHeader {
                prefix,
                result,
                consensus_info,
                seal,
            } = shared;

            const MAX_N: usize = 6;
            let leaves: [_; MAX_N] = [
                prefix.hash(),
                result.hash(),
                consensus_info.hash(),
                seal.hash(),
                beacon_chain_info.hash(),
                child_shard_blocks.root().unwrap_or_default(),
            ];
            let block_root =
                UnbalancedMerkleTree::compute_root_only::<{ MAX_N as u64 }, _, _>(leaves)
                    .expect("The list is not empty; qed");

            BlockRoot::new(Blake3Hash::new(block_root))
        };

        #[cfg(not(any(target_os = "none", target_os = "unknown")))]
        {
            cached_block_root.get_or_init(compute_root)
        }
        #[cfg(all(feature = "alloc", any(target_os = "none", target_os = "unknown")))]
        {
            cached_block_root.get_or_init(|| alloc::boxed::Box::new(compute_root()))
        }
        #[cfg(all(not(feature = "alloc"), any(target_os = "none", target_os = "unknown")))]
        {
            struct Wrapper(BlockRoot);

            impl Deref for Wrapper {
                type Target = BlockRoot;

                #[inline(always)]
                fn deref(&self) -> &Self::Target {
                    &self.0
                }
            }

            Wrapper(compute_root())
        }
    }
}

/// Block header that corresponds to a leaf shard
#[derive(Debug, Clone, Yokeable)]
// Prevent creation of potentially broken invariants externally
#[non_exhaustive]
pub struct LeafShardHeader<'a> {
    /// Shared block header
    shared: SharedBlockHeader<'a>,
    /// Beacon chain info
    beacon_chain_info: &'a BlockHeaderBeaconChainInfo,
    /// All bytes of the header except the seal
    pre_seal_bytes: &'a [u8],
    #[cfg(all(feature = "alloc", any(target_os = "none", target_os = "unknown")))]
    cached_block_root: rclite::Arc<once_cell::race::OnceBox<BlockRoot>>,
    #[cfg(not(any(target_os = "none", target_os = "unknown")))]
    cached_block_root: rclite::Arc<std::sync::OnceLock<BlockRoot>>,
}

impl<'a> Deref for LeafShardHeader<'a> {
    type Target = SharedBlockHeader<'a>;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.shared
    }
}

impl<'a> GenericBlockHeader<'a> for LeafShardHeader<'a> {
    const SHARD_KIND: ShardKind = ShardKind::LeafShard;

    #[cfg(feature = "alloc")]
    type Owned = OwnedLeafShardHeader;

    #[cfg(feature = "alloc")]
    #[inline(always)]
    fn to_owned(self) -> Self::Owned {
        self.to_owned()
    }

    #[inline(always)]
    fn root(&self) -> impl Deref<Target = BlockRoot> + Send + Sync {
        self.root()
    }

    #[inline(always)]
    fn pre_seal_hash(&self) -> Blake3Hash {
        self.pre_seal_hash()
    }
}

impl<'a> LeafShardHeader<'a> {
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

        if prefix.shard_index.shard_kind() != ShardKind::LeafShard {
            return None;
        }

        let beacon_chain_info = remainder.split_off(..size_of::<BlockHeaderBeaconChainInfo>())?;
        // SAFETY: All bit patterns are valid
        let beacon_chain_info =
            unsafe { BlockHeaderBeaconChainInfo::from_bytes(beacon_chain_info) }?;

        let pre_seal_bytes = &bytes[..bytes.len() - remainder.len()];

        let (seal, remainder) = BlockHeaderSeal::try_from_bytes(remainder)?;

        let shared = SharedBlockHeader {
            prefix,
            result,
            consensus_info,
            seal,
        };

        let header = Self {
            shared,
            beacon_chain_info,
            pre_seal_bytes,
            #[cfg(any(feature = "alloc", not(any(target_os = "none", target_os = "unknown"))))]
            cached_block_root: rclite::Arc::default(),
        };

        if !header.is_internally_consistent() {
            return None;
        }

        Some((header, remainder))
    }

    /// Check block header's internal consistency.
    ///
    /// This is usually not necessary to be called explicitly since internal consistency is checked
    /// by [`Self::try_from_bytes()`] internally.
    #[inline]
    pub fn is_internally_consistent(&self) -> bool {
        let public_key_hash = match self.seal {
            BlockHeaderSeal::Ed25519(seal) => seal.public_key.hash(),
        };
        public_key_hash == self.shared.consensus_info.solution.public_key_hash
    }

    /// The same as [`Self::try_from_bytes()`], but for trusted input that skips some consistency
    /// checks
    #[inline]
    pub fn try_from_bytes_unchecked(bytes: &'a [u8]) -> Option<(Self, &'a [u8])> {
        // The layout here is as follows:
        // * block header result: BlockHeaderResult
        // * block header prefix: BlockHeaderPrefix
        // * consensus info: BlockHeaderConsensusInfo
        // * beacon chain: BlockHeaderBeaconChainInfo
        // * block header seal: BlockHeaderSeal

        let (prefix, consensus_info, result, mut remainder) =
            BlockHeader::try_from_bytes_shared(bytes)?;

        if prefix.shard_index.shard_kind() != ShardKind::LeafShard {
            return None;
        }

        let beacon_chain_info = remainder.split_off(..size_of::<BlockHeaderBeaconChainInfo>())?;
        // SAFETY: All bit patterns are valid
        let beacon_chain_info =
            unsafe { BlockHeaderBeaconChainInfo::from_bytes(beacon_chain_info) }?;

        let pre_seal_bytes = &bytes[..bytes.len() - remainder.len()];

        let (seal, remainder) = BlockHeaderSeal::try_from_bytes(remainder)?;

        let shared = SharedBlockHeader {
            prefix,
            result,
            consensus_info,
            seal,
        };

        Some((
            Self {
                shared,
                beacon_chain_info,
                pre_seal_bytes,
                #[cfg(any(
                    feature = "alloc",
                    not(any(target_os = "none", target_os = "unknown"))
                ))]
                cached_block_root: rclite::Arc::default(),
            },
            remainder,
        ))
    }

    /// Create an owned version of this header
    #[cfg(feature = "alloc")]
    #[inline(always)]
    pub fn to_owned(self) -> OwnedLeafShardHeader {
        let unsealed = OwnedLeafShardHeader::from_parts(
            self.shared.prefix,
            self.shared.result,
            self.shared.consensus_info,
            self.beacon_chain_info,
        );

        unsealed.with_seal(self.shared.seal)
    }

    /// Shared block header
    #[inline(always)]
    pub fn shared(&self) -> &SharedBlockHeader<'a> {
        &self.shared
    }

    /// Beacon chain info
    #[inline(always)]
    pub fn beacon_chain_info(&self) -> &'a BlockHeaderBeaconChainInfo {
        self.beacon_chain_info
    }

    /// Hash of the block before seal is applied to it
    #[inline]
    pub fn pre_seal_hash(&self) -> Blake3Hash {
        // TODO: Keyed hash with `block_header_seal` as a key
        Blake3Hash::from(blake3::hash(self.pre_seal_bytes))
    }

    /// Verify seal against [`LeafShardHeader::pre_seal_hash()`] and check that its public key hash
    /// corresponds to the solution
    #[inline]
    pub fn is_sealed_correctly(&self) -> bool {
        self.consensus_info.solution.public_key_hash == self.seal.public_key_hash()
            && self.seal.is_seal_valid(&self.pre_seal_hash())
    }

    /// Compute block root out of this header.
    ///
    /// Block root is a Merkle Tree Root. The leaves are derived from individual fields in
    /// [`SharedBlockHeader`] and other fields of this enum in the declaration order.
    ///
    /// Note that this method computes root by doing a bunch of hashing. The result is then cached
    /// if `alloc` feature is enabled or when compiled for OS target that is not `none`.
    #[inline]
    pub fn root(&self) -> impl Deref<Target = BlockRoot> + Send + Sync {
        let Self {
            shared,
            beacon_chain_info,
            pre_seal_bytes: _,
            #[cfg(any(feature = "alloc", not(any(target_os = "none", target_os = "unknown"))))]
            cached_block_root,
        } = self;

        let compute_root = || {
            let SharedBlockHeader {
                prefix,
                result,
                consensus_info,
                seal,
            } = shared;

            const MAX_N: usize = 5;
            let leaves: [_; MAX_N] = [
                prefix.hash(),
                result.hash(),
                consensus_info.hash(),
                seal.hash(),
                beacon_chain_info.hash(),
            ];
            let block_root =
                UnbalancedMerkleTree::compute_root_only::<{ MAX_N as u64 }, _, _>(leaves)
                    .expect("The list is not empty; qed");

            BlockRoot::new(Blake3Hash::new(block_root))
        };

        #[cfg(not(any(target_os = "none", target_os = "unknown")))]
        {
            cached_block_root.get_or_init(compute_root)
        }
        #[cfg(all(feature = "alloc", any(target_os = "none", target_os = "unknown")))]
        {
            cached_block_root.get_or_init(|| alloc::boxed::Box::new(compute_root()))
        }
        #[cfg(all(not(feature = "alloc"), any(target_os = "none", target_os = "unknown")))]
        {
            struct Wrapper(BlockRoot);

            impl Deref for Wrapper {
                type Target = BlockRoot;

                #[inline(always)]
                fn deref(&self) -> &Self::Target {
                    &self.0
                }
            }

            Wrapper(compute_root())
        }
    }
}

/// Block header that together with [`BlockBody`] form a [`Block`]
///
/// [`BlockBody`]: crate::block::body::BlockBody
/// [`Block`]: crate::block::Block
#[derive(Debug, Clone, From)]
pub enum BlockHeader<'a> {
    /// Block header corresponds to the beacon chain
    BeaconChain(BeaconChainHeader<'a>),
    /// Block header corresponds to an intermediate shard
    IntermediateShard(IntermediateShardHeader<'a>),
    /// Block header corresponds to a leaf shard
    LeafShard(LeafShardHeader<'a>),
}

impl<'a> Deref for BlockHeader<'a> {
    type Target = SharedBlockHeader<'a>;

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
                let (header, remainder) = BeaconChainHeader::try_from_bytes(bytes)?;
                Some((Self::BeaconChain(header), remainder))
            }
            ShardKind::IntermediateShard => {
                let (header, remainder) = IntermediateShardHeader::try_from_bytes(bytes)?;
                Some((Self::IntermediateShard(header), remainder))
            }
            ShardKind::LeafShard => {
                let (header, remainder) = LeafShardHeader::try_from_bytes(bytes)?;
                Some((Self::LeafShard(header), remainder))
            }
            ShardKind::Phantom | ShardKind::Invalid => {
                // Blocks for such shards do not exist
                None
            }
        }
    }

    /// Check block header's internal consistency.
    ///
    /// This is usually not necessary to be called explicitly since internal consistency is checked
    /// by [`Self::try_from_bytes()`] internally.
    #[inline]
    pub fn is_internally_consistent(&self) -> bool {
        match self {
            Self::BeaconChain(header) => header.is_internally_consistent(),
            Self::IntermediateShard(header) => header.is_internally_consistent(),
            Self::LeafShard(header) => header.is_internally_consistent(),
        }
    }

    /// The same as [`Self::try_from_bytes()`], but for trusted input that skips some consistency
    /// checks
    #[inline]
    pub fn try_from_bytes_unchecked(
        bytes: &'a [u8],
        shard_kind: ShardKind,
    ) -> Option<(Self, &'a [u8])> {
        match shard_kind {
            ShardKind::BeaconChain => {
                let (header, remainder) = BeaconChainHeader::try_from_bytes_unchecked(bytes)?;
                Some((Self::BeaconChain(header), remainder))
            }
            ShardKind::IntermediateShard => {
                let (header, remainder) = IntermediateShardHeader::try_from_bytes_unchecked(bytes)?;
                Some((Self::IntermediateShard(header), remainder))
            }
            ShardKind::LeafShard => {
                let (header, remainder) = LeafShardHeader::try_from_bytes_unchecked(bytes)?;
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

        if !(prefix.padding_0 == [0; _]
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

    /// Create an owned version of this header
    #[cfg(feature = "alloc")]
    #[inline(always)]
    pub fn to_owned(self) -> OwnedBlockHeader {
        match self {
            Self::BeaconChain(header) => header.to_owned().into(),
            Self::IntermediateShard(header) => header.to_owned().into(),
            Self::LeafShard(header) => header.to_owned().into(),
        }
    }

    /// Hash of the block before seal is applied to it
    #[inline]
    pub fn pre_seal_hash(&self) -> Blake3Hash {
        match self {
            Self::BeaconChain(header) => header.pre_seal_hash(),
            Self::IntermediateShard(header) => header.pre_seal_hash(),
            Self::LeafShard(header) => header.pre_seal_hash(),
        }
    }

    /// Verify seal against [`BlockHeader::pre_seal_hash()`] and check that its public key hash
    /// corresponds to the solution
    #[inline]
    pub fn is_sealed_correctly(&self) -> bool {
        match self {
            Self::BeaconChain(header) => header.is_sealed_correctly(),
            Self::IntermediateShard(header) => header.is_sealed_correctly(),
            Self::LeafShard(header) => header.is_sealed_correctly(),
        }
    }

    /// Compute block root out of this header.
    ///
    /// Block root is a Merkle Tree Root. The leaves are derived from individual fields in
    /// [`SharedBlockHeader`] and other fields of this enum in the declaration order.
    ///
    /// Note that this method computes root by doing a bunch of hashing. The result is then cached
    /// if `alloc` feature is enabled.
    #[inline]
    pub fn root(&self) -> impl Deref<Target = BlockRoot> + Send + Sync {
        enum Wrapper<B, I, L> {
            BeaconChain(B),
            IntermediateShard(I),
            LeafShard(L),
        }

        impl<B, I, L> Deref for Wrapper<B, I, L>
        where
            B: Deref<Target = BlockRoot>,
            I: Deref<Target = BlockRoot>,
            L: Deref<Target = BlockRoot>,
        {
            type Target = BlockRoot;

            #[inline(always)]
            fn deref(&self) -> &Self::Target {
                match self {
                    Wrapper::BeaconChain(block_root) => block_root,
                    Wrapper::IntermediateShard(block_root) => block_root,
                    Wrapper::LeafShard(block_root) => block_root,
                }
            }
        }

        // TODO: Should unique keyed hash be used for different kinds of shards?
        match self {
            Self::BeaconChain(header) => Wrapper::BeaconChain(header.root()),
            Self::IntermediateShard(header) => Wrapper::IntermediateShard(header.root()),
            Self::LeafShard(header) => Wrapper::LeafShard(header.root()),
        }
    }
}
