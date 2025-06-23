//! Data structures related to the owned version of [`BlockHeader`]

use crate::block::BlockRoot;
use crate::block::header::{
    BeaconChainHeader, BlockHeader, BlockHeaderBeaconChainInfo, BlockHeaderConsensusInfo,
    BlockHeaderConsensusParameters, BlockHeaderPrefix, BlockHeaderResult, BlockHeaderSeal,
    BlockHeaderSealType, GenericBlockHeader, IntermediateShardHeader, LeafShardHeader,
};
use crate::hashes::Blake3Hash;
use crate::shard::ShardKind;
use ab_aligned_buffer::{OwnedAlignedBuffer, SharedAlignedBuffer};
use ab_io_type::trivial_type::TrivialType;
use core::fmt;
use derive_more::From;
use rclite::Arc;
use yoke::Yoke;

/// Generic owned block header
pub trait GenericOwnedBlockHeader: Clone + fmt::Debug + 'static {
    /// Block header
    type Header<'a>: GenericBlockHeader<'a>
    where
        Self: 'a;

    /// Get regular block header out of the owned version
    fn header(&self) -> &Self::Header<'_>;
}

fn append_seal(buffer: &mut OwnedAlignedBuffer, seal: BlockHeaderSeal<'_>) {
    match seal {
        BlockHeaderSeal::Ed25519(seal) => {
            let true = buffer.append(&[BlockHeaderSealType::Ed25519 as u8]) else {
                unreachable!("Fixed size data structures that are guaranteed to fit; qed");
            };
            let true = buffer.append(seal.as_bytes()) else {
                unreachable!("Fixed size data structures that are guaranteed to fit; qed");
            };
        }
    }
}

/// Errors for [`OwnedBeaconChainHeader`]
#[derive(Debug, thiserror::Error)]
pub enum OwnedBeaconChainHeaderError {
    /// Too many child shard blocks
    #[error("Too many child shard blocks: {actual}")]
    TooManyChildShardBlocks {
        /// Actual number of child shard blocks
        actual: usize,
    },
}

/// An owned version of [`BeaconChainHeader`].
///
/// It is correctly aligned in memory and well suited for sending and receiving over the network
/// efficiently or storing in memory or on disk.
#[derive(Debug, Clone)]
pub struct OwnedBeaconChainHeader {
    inner: Arc<Yoke<BeaconChainHeader<'static>, SharedAlignedBuffer>>,
}

impl GenericOwnedBlockHeader for OwnedBeaconChainHeader {
    type Header<'a> = BeaconChainHeader<'a>;

    #[inline(always)]
    fn header(&self) -> &Self::Header<'_> {
        self.header()
    }
}

impl OwnedBeaconChainHeader {
    /// Max allocation needed by this header
    #[inline(always)]
    pub const fn max_allocation_for(child_shard_blocks: &[BlockRoot]) -> u32 {
        BlockHeaderPrefix::SIZE
            + BlockHeaderResult::SIZE
            + BlockHeaderConsensusInfo::SIZE
            + (
                // Number of child shard blocks
                u16::SIZE
                // Padding
                + <[u8; 2]>::SIZE
                + size_of_val(child_shard_blocks) as u32
            )
            + BlockHeaderConsensusParameters::MAX_SIZE
            + BlockHeaderSeal::MAX_SIZE
    }

    /// Create new [`OwnedBeaconChainHeader`] from its parts
    pub fn from_parts(
        prefix: &BlockHeaderPrefix,
        result: &BlockHeaderResult,
        consensus_info: &BlockHeaderConsensusInfo,
        child_shard_blocks: &[BlockRoot],
        consensus_parameters: BlockHeaderConsensusParameters<'_>,
    ) -> Result<OwnedBeaconChainHeaderUnsealed, OwnedBeaconChainHeaderError> {
        let mut buffer =
            OwnedAlignedBuffer::with_capacity(Self::max_allocation_for(child_shard_blocks));

        Self::from_parts_into(
            prefix,
            result,
            consensus_info,
            child_shard_blocks,
            consensus_parameters,
            &mut buffer,
        )?;

        Ok(OwnedBeaconChainHeaderUnsealed { buffer })
    }

    /// Create owned header from its parts and write it into provided buffer
    pub fn from_parts_into(
        prefix: &BlockHeaderPrefix,
        result: &BlockHeaderResult,
        consensus_info: &BlockHeaderConsensusInfo,
        child_shard_blocks: &[BlockRoot],
        consensus_parameters: BlockHeaderConsensusParameters<'_>,
        buffer: &mut OwnedAlignedBuffer,
    ) -> Result<(), OwnedBeaconChainHeaderError> {
        let num_blocks = child_shard_blocks.len();
        let num_blocks = u16::try_from(num_blocks).map_err(|_error| {
            OwnedBeaconChainHeaderError::TooManyChildShardBlocks { actual: num_blocks }
        })?;
        let true = buffer.append(prefix.as_bytes()) else {
            unreachable!("Fixed size data structures that are guaranteed to fit; qed");
        };
        let true = buffer.append(result.as_bytes()) else {
            unreachable!("Fixed size data structures that are guaranteed to fit; qed");
        };
        let true = buffer.append(consensus_info.as_bytes()) else {
            unreachable!("Fixed size data structures that are guaranteed to fit; qed");
        };
        // TODO: Would be nice for `BlockHeaderChildShardBlocks` to have API to write this by itself
        {
            let true = buffer.append(&num_blocks.to_le_bytes()) else {
                unreachable!("Fixed size data structures that are guaranteed to fit; qed");
            };
            let true = buffer.append(&[0; 2]) else {
                unreachable!("Fixed size data structures that are guaranteed to fit; qed");
            };
            let true = buffer.append(BlockRoot::repr_from_slice(child_shard_blocks).as_flattened())
            else {
                unreachable!("Checked size above; qed");
            };
        }
        // TODO: Would be nice for `BlockHeaderBeaconChainParameters` to have API to write this by
        //  itself
        {
            let true = buffer.append(
                &consensus_parameters
                    .fixed_parameters
                    .solution_range
                    .to_bytes(),
            ) else {
                unreachable!("Fixed size data structures that are guaranteed to fit; qed");
            };
            let true = buffer.append(
                &consensus_parameters
                    .fixed_parameters
                    .slot_iterations
                    .get()
                    .to_le_bytes(),
            ) else {
                unreachable!("Fixed size data structures that are guaranteed to fit; qed");
            };

            let bitflags = {
                let mut bitflags = 0u8;

                if consensus_parameters.super_segment_root.is_some() {
                    bitflags |= BlockHeaderConsensusParameters::SUPER_SEGMENT_ROOT_MASK;
                }
                if consensus_parameters.next_solution_range.is_some() {
                    bitflags |= BlockHeaderConsensusParameters::NEXT_SOLUTION_RANGE_MASK;
                }
                if consensus_parameters.pot_parameters_change.is_some() {
                    bitflags |= BlockHeaderConsensusParameters::POT_PARAMETERS_CHANGE_MASK;
                }

                bitflags
            };

            let true = buffer.append(&[bitflags]) else {
                unreachable!("Fixed size data structures that are guaranteed to fit; qed");
            };

            if let Some(super_segment_root) = consensus_parameters.super_segment_root {
                let true = buffer.append(super_segment_root.as_ref()) else {
                    unreachable!("Fixed size data structures that are guaranteed to fit; qed");
                };
            }

            if let Some(next_solution_range) = consensus_parameters.next_solution_range {
                let true = buffer.append(&next_solution_range.to_bytes()) else {
                    unreachable!("Fixed size data structures that are guaranteed to fit; qed");
                };
            }

            if let Some(pot_parameters_change) = consensus_parameters.pot_parameters_change {
                let true = buffer.append(&pot_parameters_change.slot.to_bytes()) else {
                    unreachable!("Fixed size data structures that are guaranteed to fit; qed");
                };
                let true =
                    buffer.append(&pot_parameters_change.slot_iterations.get().to_le_bytes())
                else {
                    unreachable!("Fixed size data structures that are guaranteed to fit; qed");
                };
                let true = buffer.append(pot_parameters_change.entropy.as_ref()) else {
                    unreachable!("Fixed size data structures that are guaranteed to fit; qed");
                };
            }
        }

        Ok(())
    }

    /// Create owned header from a buffer
    #[inline]
    pub fn from_buffer(buffer: SharedAlignedBuffer) -> Result<Self, SharedAlignedBuffer> {
        // TODO: Cloning is cheap, but will not be necessary if/when this is resolved:
        //  https://github.com/unicode-org/icu4x/issues/6665
        let inner = Yoke::try_attach_to_cart(buffer.clone(), |buffer| {
            let Some((header, extra_bytes)) = BeaconChainHeader::try_from_bytes(buffer) else {
                return Err(());
            };
            if !extra_bytes.is_empty() {
                return Err(());
            }

            Ok(header)
        })
        .map_err(move |()| buffer)?;

        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Inner buffer with block header contents
    #[inline(always)]
    pub fn buffer(&self) -> &SharedAlignedBuffer {
        self.inner.backing_cart()
    }

    /// Get [`BeaconChainHeader`] out of [`OwnedBeaconChainHeader`]
    #[inline(always)]
    pub fn header(&self) -> &BeaconChainHeader<'_> {
        self.inner.get()
    }
}

/// Owned beacon chain block header, which is not sealed yet
#[derive(Debug, Clone)]
pub struct OwnedBeaconChainHeaderUnsealed {
    buffer: OwnedAlignedBuffer,
}

impl OwnedBeaconChainHeaderUnsealed {
    /// Hash of the block before seal is applied to it
    #[inline(always)]
    pub fn pre_seal_hash(&self) -> Blake3Hash {
        // TODO: Keyed hash with `block_header_seal` as a key
        Blake3Hash::from(blake3::hash(self.buffer.as_slice()))
    }

    /// Add seal and return [`OwnedBeaconChainHeader`]
    pub fn with_seal(self, seal: BlockHeaderSeal<'_>) -> OwnedBeaconChainHeader {
        let Self { mut buffer } = self;
        append_seal(&mut buffer, seal);

        // TODO: Avoid extra parsing here, for this `OwnedBeaconChainHeader::from_parts_into()` must
        //  return references to parts. Or at least add unchecked version of `from_buffer()`
        OwnedBeaconChainHeader::from_buffer(buffer.into_shared())
            .expect("Known to be created correctly; qed")
    }
}

/// Errors for [`OwnedIntermediateShardHeader`]
#[derive(Debug, thiserror::Error)]
pub enum OwnedIntermediateShardHeaderError {
    /// Too many child shard blocks
    #[error("Too many child shard blocks: {actual}")]
    TooManyChildShardBlocks {
        /// Actual number of child shard blocks
        actual: usize,
    },
}

/// An owned version of [`IntermediateShardHeader`].
///
/// It is correctly aligned in memory and well suited for sending and receiving over the network
/// efficiently or storing in memory or on disk.
#[derive(Debug, Clone)]
pub struct OwnedIntermediateShardHeader {
    inner: Arc<Yoke<IntermediateShardHeader<'static>, SharedAlignedBuffer>>,
}

impl GenericOwnedBlockHeader for OwnedIntermediateShardHeader {
    type Header<'a> = IntermediateShardHeader<'a>;

    #[inline(always)]
    fn header(&self) -> &Self::Header<'_> {
        self.header()
    }
}

impl OwnedIntermediateShardHeader {
    /// Max allocation needed by this header
    #[inline(always)]
    pub const fn max_allocation_for(child_shard_blocks: &[BlockRoot]) -> u32 {
        BlockHeaderPrefix::SIZE
            + BlockHeaderResult::SIZE
            + BlockHeaderConsensusInfo::SIZE
            + BlockHeaderBeaconChainInfo::SIZE
            + (
                // Number of child shard blocks
                u16::SIZE
                // Padding
                + <[u8; 2]>::SIZE
                + size_of_val(child_shard_blocks) as u32
            )
            + BlockHeaderSeal::MAX_SIZE
    }

    /// Create new [`OwnedIntermediateShardHeader`] from its parts
    pub fn from_parts(
        prefix: &BlockHeaderPrefix,
        result: &BlockHeaderResult,
        consensus_info: &BlockHeaderConsensusInfo,
        beacon_chain_info: &BlockHeaderBeaconChainInfo,
        child_shard_blocks: &[BlockRoot],
    ) -> Result<OwnedIntermediateShardHeaderUnsealed, OwnedIntermediateShardHeaderError> {
        let mut buffer =
            OwnedAlignedBuffer::with_capacity(Self::max_allocation_for(child_shard_blocks));

        Self::from_parts_into(
            prefix,
            result,
            consensus_info,
            beacon_chain_info,
            child_shard_blocks,
            &mut buffer,
        )?;

        Ok(OwnedIntermediateShardHeaderUnsealed { buffer })
    }

    /// Create owned header from its parts and write it into provided buffer
    pub fn from_parts_into(
        prefix: &BlockHeaderPrefix,
        result: &BlockHeaderResult,
        consensus_info: &BlockHeaderConsensusInfo,
        beacon_chain_info: &BlockHeaderBeaconChainInfo,
        child_shard_blocks: &[BlockRoot],
        buffer: &mut OwnedAlignedBuffer,
    ) -> Result<(), OwnedIntermediateShardHeaderError> {
        let num_blocks = child_shard_blocks.len();
        let num_blocks = u16::try_from(num_blocks).map_err(|_error| {
            OwnedIntermediateShardHeaderError::TooManyChildShardBlocks { actual: num_blocks }
        })?;
        let true = buffer.append(prefix.as_bytes()) else {
            unreachable!("Fixed size data structures that are guaranteed to fit; qed");
        };
        let true = buffer.append(result.as_bytes()) else {
            unreachable!("Fixed size data structures that are guaranteed to fit; qed");
        };
        let true = buffer.append(consensus_info.as_bytes()) else {
            unreachable!("Fixed size data structures that are guaranteed to fit; qed");
        };
        let true = buffer.append(beacon_chain_info.as_bytes()) else {
            unreachable!("Fixed size data structures that are guaranteed to fit; qed");
        };
        // TODO: Would be nice for `BlockHeaderChildShardBlocks` to have API to write this by itself
        {
            let true = buffer.append(&num_blocks.to_le_bytes()) else {
                unreachable!("Fixed size data structures that are guaranteed to fit; qed");
            };
            let true = buffer.append(&[0; 2]) else {
                unreachable!("Fixed size data structures that are guaranteed to fit; qed");
            };
            let true = buffer.append(BlockRoot::repr_from_slice(child_shard_blocks).as_flattened())
            else {
                unreachable!("Checked size above; qed");
            };
        }

        Ok(())
    }

    /// Create owned header from a buffer
    #[inline]
    pub fn from_buffer(buffer: SharedAlignedBuffer) -> Result<Self, SharedAlignedBuffer> {
        // TODO: Cloning is cheap, but will not be necessary if/when this is resolved:
        //  https://github.com/unicode-org/icu4x/issues/6665
        let inner = Yoke::try_attach_to_cart(buffer.clone(), |buffer| {
            let Some((header, extra_bytes)) = IntermediateShardHeader::try_from_bytes(buffer)
            else {
                return Err(());
            };
            if !extra_bytes.is_empty() {
                return Err(());
            }

            Ok(header)
        })
        .map_err(move |()| buffer)?;

        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Inner buffer with block header contents
    #[inline(always)]
    pub fn buffer(&self) -> &SharedAlignedBuffer {
        self.inner.backing_cart()
    }
    /// Get [`IntermediateShardHeader`] out of [`OwnedIntermediateShardHeader`]
    #[inline(always)]
    pub fn header(&self) -> &IntermediateShardHeader<'_> {
        self.inner.get()
    }
}

/// Owned intermediate shard block header, which is not sealed yet
#[derive(Debug, Clone)]
pub struct OwnedIntermediateShardHeaderUnsealed {
    buffer: OwnedAlignedBuffer,
}

impl OwnedIntermediateShardHeaderUnsealed {
    /// Hash of the block before seal is applied to it
    #[inline(always)]
    pub fn pre_seal_hash(&self) -> Blake3Hash {
        // TODO: Keyed hash with `block_header_seal` as a key
        Blake3Hash::from(blake3::hash(self.buffer.as_slice()))
    }

    /// Add seal and return [`OwnedIntermediateShardHeader`]
    pub fn with_seal(self, seal: BlockHeaderSeal<'_>) -> OwnedIntermediateShardHeader {
        let Self { mut buffer } = self;
        append_seal(&mut buffer, seal);

        // TODO: Avoid extra parsing here, for this
        //  `OwnedIntermediateShardHeader::from_parts_into()` must return references to parts. Or
        //  at least add unchecked version of `from_buffer()`
        OwnedIntermediateShardHeader::from_buffer(buffer.into_shared())
            .expect("Known to be created correctly; qed")
    }
}

/// An owned version of [`LeafShardHeader`].
///
/// It is correctly aligned in memory and well suited for sending and receiving over the network
/// efficiently or storing in memory or on disk.
#[derive(Debug, Clone)]
pub struct OwnedLeafShardHeader {
    inner: Arc<Yoke<LeafShardHeader<'static>, SharedAlignedBuffer>>,
}

impl GenericOwnedBlockHeader for OwnedLeafShardHeader {
    type Header<'a> = LeafShardHeader<'a>;

    #[inline(always)]
    fn header(&self) -> &Self::Header<'_> {
        self.header()
    }
}

impl OwnedLeafShardHeader {
    /// Max allocation needed by this header
    pub const MAX_ALLOCATION: u32 = BlockHeaderPrefix::SIZE
        + BlockHeaderResult::SIZE
        + BlockHeaderConsensusInfo::SIZE
        + BlockHeaderBeaconChainInfo::SIZE
        + BlockHeaderSeal::MAX_SIZE;

    /// Create new [`OwnedLeafShardHeader`] from its parts
    pub fn from_parts(
        prefix: &BlockHeaderPrefix,
        result: &BlockHeaderResult,
        consensus_info: &BlockHeaderConsensusInfo,
        beacon_chain_info: &BlockHeaderBeaconChainInfo,
    ) -> OwnedLeafShardHeaderUnsealed {
        let mut buffer = OwnedAlignedBuffer::with_capacity(Self::MAX_ALLOCATION);

        Self::from_parts_into(
            prefix,
            result,
            consensus_info,
            beacon_chain_info,
            &mut buffer,
        );

        OwnedLeafShardHeaderUnsealed { buffer }
    }

    /// Create owned header from its parts and write it into provided buffer
    pub fn from_parts_into(
        prefix: &BlockHeaderPrefix,
        result: &BlockHeaderResult,
        consensus_info: &BlockHeaderConsensusInfo,
        beacon_chain_info: &BlockHeaderBeaconChainInfo,
        buffer: &mut OwnedAlignedBuffer,
    ) {
        let true = buffer.append(prefix.as_bytes()) else {
            unreachable!("Fixed size data structures that are guaranteed to fit; qed");
        };
        let true = buffer.append(result.as_bytes()) else {
            unreachable!("Fixed size data structures that are guaranteed to fit; qed");
        };
        let true = buffer.append(consensus_info.as_bytes()) else {
            unreachable!("Fixed size data structures that are guaranteed to fit; qed");
        };
        let true = buffer.append(beacon_chain_info.as_bytes()) else {
            unreachable!("Fixed size data structures that are guaranteed to fit; qed");
        };
    }

    /// Create owned header from a buffer
    #[inline]
    pub fn from_buffer(buffer: SharedAlignedBuffer) -> Result<Self, SharedAlignedBuffer> {
        // TODO: Cloning is cheap, but will not be necessary if/when this is resolved:
        //  https://github.com/unicode-org/icu4x/issues/6665
        let inner = Yoke::try_attach_to_cart(buffer.clone(), |buffer| {
            let Some((header, extra_bytes)) = LeafShardHeader::try_from_bytes(buffer) else {
                return Err(());
            };
            if !extra_bytes.is_empty() {
                return Err(());
            }

            Ok(header)
        })
        .map_err(move |()| buffer)?;

        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Inner buffer with block header contents
    #[inline(always)]
    pub fn buffer(&self) -> &SharedAlignedBuffer {
        self.inner.backing_cart()
    }
    /// Get [`LeafShardHeader`] out of [`OwnedLeafShardHeader`]
    #[inline(always)]
    pub fn header(&self) -> &LeafShardHeader<'_> {
        self.inner.get()
    }
}

/// Owned leaf shard block header, which is not sealed yet
#[derive(Debug, Clone)]
pub struct OwnedLeafShardHeaderUnsealed {
    buffer: OwnedAlignedBuffer,
}

impl OwnedLeafShardHeaderUnsealed {
    /// Hash of the block before seal is applied to it
    #[inline(always)]
    pub fn pre_seal_hash(&self) -> Blake3Hash {
        // TODO: Keyed hash with `block_header_seal` as a key
        Blake3Hash::from(blake3::hash(self.buffer.as_slice()))
    }

    /// Add seal and return [`OwnedLeafShardHeader`]
    pub fn with_seal(self, seal: BlockHeaderSeal<'_>) -> OwnedLeafShardHeader {
        let Self { mut buffer } = self;
        append_seal(&mut buffer, seal);

        // TODO: Avoid extra parsing here, for this `OwnedLeafShardHeader::from_parts_into()` must
        //  return references to parts. Or at least add unchecked version of `from_buffer()`
        OwnedLeafShardHeader::from_buffer(buffer.into_shared())
            .expect("Known to be created correctly; qed")
    }
}

/// An owned version of [`BlockHeader`].
///
/// It is correctly aligned in memory and well suited for sending and receiving over the network
/// efficiently or storing in memory or on disk.
#[derive(Debug, Clone, From)]
pub enum OwnedBlockHeader {
    /// Block header corresponds to the beacon chain
    BeaconChain(OwnedBeaconChainHeader),
    /// Block header corresponds to an intermediate shard
    IntermediateShard(OwnedIntermediateShardHeader),
    /// Block header corresponds to a leaf shard
    LeafShard(OwnedLeafShardHeader),
}

impl OwnedBlockHeader {
    /// Create owned header from a buffer
    #[inline]
    pub fn from_buffer(
        buffer: SharedAlignedBuffer,
        shard_kind: ShardKind,
    ) -> Result<Self, SharedAlignedBuffer> {
        Ok(match shard_kind {
            ShardKind::BeaconChain => {
                Self::BeaconChain(OwnedBeaconChainHeader::from_buffer(buffer)?)
            }
            ShardKind::IntermediateShard => {
                Self::IntermediateShard(OwnedIntermediateShardHeader::from_buffer(buffer)?)
            }
            ShardKind::LeafShard => Self::LeafShard(OwnedLeafShardHeader::from_buffer(buffer)?),
            ShardKind::Phantom | ShardKind::Invalid => {
                // Blocks for such shards do not exist
                return Err(buffer);
            }
        })
    }

    /// Inner buffer block header contents
    #[inline]
    pub fn buffer(&self) -> &SharedAlignedBuffer {
        match self {
            Self::BeaconChain(owned_header) => owned_header.buffer(),
            Self::IntermediateShard(owned_header) => owned_header.buffer(),
            Self::LeafShard(owned_header) => owned_header.buffer(),
        }
    }

    /// Get [`BlockHeader`] out of [`OwnedBlockHeader`]
    #[inline]
    pub fn header(&self) -> BlockHeader<'_> {
        match self {
            Self::BeaconChain(owned_header) => {
                BlockHeader::BeaconChain(owned_header.header().clone())
            }
            Self::IntermediateShard(owned_header) => {
                BlockHeader::IntermediateShard(owned_header.header().clone())
            }
            Self::LeafShard(owned_header) => BlockHeader::LeafShard(owned_header.header().clone()),
        }
    }
}
