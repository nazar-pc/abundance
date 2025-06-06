//! Data structures related to the owned version of [`BlockHeader`]

use crate::block::BlockRoot;
use crate::block::header::{
    BeaconChainHeader, BlockHeader, BlockHeaderBeaconChainInfo, BlockHeaderBeaconChainParameters,
    BlockHeaderConsensusInfo, BlockHeaderPrefix, BlockHeaderResult, BlockHeaderSealRef,
    BlockHeaderSealType, IntermediateShardHeader, LeafShardHeader,
};
use crate::hashes::Blake3Hash;
use crate::shard::ShardKind;
use ab_aligned_buffer::{OwnedAlignedBuffer, SharedAlignedBuffer};
use ab_io_type::trivial_type::TrivialType;
use derive_more::From;

fn append_seal(buffer: &mut OwnedAlignedBuffer, seal: BlockHeaderSealRef<'_>) {
    match seal {
        BlockHeaderSealRef::Ed25519(seal) => {
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
    buffer: SharedAlignedBuffer,
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
            + BlockHeaderBeaconChainParameters::MAX_SIZE
            + BlockHeaderSealRef::MAX_SIZE
    }

    /// Create new [`OwnedBeaconChainHeader`] from its parts
    pub fn from_parts(
        prefix: &BlockHeaderPrefix,
        result: &BlockHeaderResult,
        consensus_info: &BlockHeaderConsensusInfo,
        child_shard_blocks: &[BlockRoot],
        consensus_parameters: BlockHeaderBeaconChainParameters<'_>,
    ) -> Result<OwnedBeaconChainBlockHeaderUnsealed, OwnedBeaconChainHeaderError> {
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

        Ok(OwnedBeaconChainBlockHeaderUnsealed { buffer })
    }

    /// Create owned header from its parts and write it into provided buffer
    pub fn from_parts_into(
        prefix: &BlockHeaderPrefix,
        result: &BlockHeaderResult,
        consensus_info: &BlockHeaderConsensusInfo,
        child_shard_blocks: &[BlockRoot],
        consensus_parameters: BlockHeaderBeaconChainParameters<'_>,
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
                    .pot_slot_iterations
                    .get()
                    .to_le_bytes(),
            ) else {
                unreachable!("Fixed size data structures that are guaranteed to fit; qed");
            };

            let bitflags = {
                let mut bitflags = 0u8;

                if consensus_parameters.super_segment_root.is_some() {
                    bitflags |= BlockHeaderBeaconChainParameters::SUPER_SEGMENT_ROOT_MASK;
                }
                if consensus_parameters.next_solution_range.is_some() {
                    bitflags |= BlockHeaderBeaconChainParameters::NEXT_SOLUTION_RANGE_MASK;
                }
                if consensus_parameters.pot_parameters_change.is_some() {
                    bitflags |= BlockHeaderBeaconChainParameters::POT_PARAMETERS_CHANGE_MASK;
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

    /// Create owned block header from a reference
    #[inline]
    pub fn from_header(header: BeaconChainHeader<'_>) -> Result<Self, OwnedBeaconChainHeaderError> {
        let unsealed = Self::from_parts(
            header.generic.prefix,
            header.generic.result,
            header.generic.consensus_info,
            &header.child_shard_blocks,
            header.consensus_parameters,
        )?;

        Ok(unsealed.with_seal(header.generic.seal))
    }

    /// Create owned header from a buffer
    #[inline]
    pub fn from_buffer(buffer: SharedAlignedBuffer) -> Result<Self, SharedAlignedBuffer> {
        let Some((_header, extra_bytes)) = BeaconChainHeader::try_from_bytes(buffer.as_slice())
        else {
            return Err(buffer);
        };
        if !extra_bytes.is_empty() {
            return Err(buffer);
        }

        Ok(Self { buffer })
    }

    /// Inner buffer with block header contents
    pub fn buffer(&self) -> &SharedAlignedBuffer {
        &self.buffer
    }

    /// Get [`BeaconChainHeader`] out of [`OwnedBeaconChainHeader`]
    pub fn header(&self) -> BeaconChainHeader<'_> {
        BeaconChainHeader::try_from_bytes_unchecked(self.buffer.as_slice())
            .expect("Constructor ensures validity; qed")
            .0
    }
}

/// Owned beacon chain block header, which is not sealed yet
#[derive(Debug, Clone)]
pub struct OwnedBeaconChainBlockHeaderUnsealed {
    buffer: OwnedAlignedBuffer,
}

impl OwnedBeaconChainBlockHeaderUnsealed {
    /// Hash of the block before seal is applied to it
    #[inline(always)]
    pub fn pre_seal_hash(&self) -> Blake3Hash {
        // TODO: Keyed hash with `block_header_seal` as a key
        Blake3Hash::from(blake3::hash(self.buffer.as_slice()))
    }

    /// Add seal and return [`OwnedBeaconChainHeader`]
    pub fn with_seal(self, seal: BlockHeaderSealRef<'_>) -> OwnedBeaconChainHeader {
        let Self { mut buffer } = self;
        append_seal(&mut buffer, seal);

        OwnedBeaconChainHeader {
            buffer: buffer.into_shared(),
        }
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
    buffer: SharedAlignedBuffer,
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
            + BlockHeaderSealRef::MAX_SIZE
    }

    /// Create new [`OwnedIntermediateShardHeader`] from its parts
    pub fn from_parts(
        prefix: &BlockHeaderPrefix,
        result: &BlockHeaderResult,
        consensus_info: &BlockHeaderConsensusInfo,
        beacon_chain_info: &BlockHeaderBeaconChainInfo,
        child_shard_blocks: &[BlockRoot],
    ) -> Result<OwnedIntermediateShardBlockHeaderUnsealed, OwnedIntermediateShardHeaderError> {
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

        Ok(OwnedIntermediateShardBlockHeaderUnsealed { buffer })
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

    /// Create owned block header from a reference
    #[inline]
    pub fn from_header(
        header: IntermediateShardHeader<'_>,
    ) -> Result<Self, OwnedIntermediateShardHeaderError> {
        let unsealed = Self::from_parts(
            header.generic.prefix,
            header.generic.result,
            header.generic.consensus_info,
            header.beacon_chain_info,
            &header.child_shard_blocks,
        )?;

        Ok(unsealed.with_seal(header.generic.seal))
    }

    /// Create owned header from a buffer
    #[inline]
    pub fn from_buffer(buffer: SharedAlignedBuffer) -> Result<Self, SharedAlignedBuffer> {
        let Some((_header, extra_bytes)) =
            IntermediateShardHeader::try_from_bytes(buffer.as_slice())
        else {
            return Err(buffer);
        };
        if !extra_bytes.is_empty() {
            return Err(buffer);
        }

        Ok(Self { buffer })
    }

    /// Inner buffer with block header contents
    pub fn buffer(&self) -> &SharedAlignedBuffer {
        &self.buffer
    }

    /// Get [`IntermediateShardHeader`] out of [`OwnedIntermediateShardHeader`]
    pub fn header(&self) -> IntermediateShardHeader<'_> {
        IntermediateShardHeader::try_from_bytes_unchecked(self.buffer.as_slice())
            .expect("Constructor ensures validity; qed")
            .0
    }
}

/// Owned intermediate shard block header, which is not sealed yet
#[derive(Debug, Clone)]
pub struct OwnedIntermediateShardBlockHeaderUnsealed {
    buffer: OwnedAlignedBuffer,
}

impl OwnedIntermediateShardBlockHeaderUnsealed {
    /// Hash of the block before seal is applied to it
    #[inline(always)]
    pub fn pre_seal_hash(&self) -> Blake3Hash {
        // TODO: Keyed hash with `block_header_seal` as a key
        Blake3Hash::from(blake3::hash(self.buffer.as_slice()))
    }

    /// Add seal and return [`OwnedIntermediateShardHeader`]
    pub fn with_seal(self, seal: BlockHeaderSealRef<'_>) -> OwnedIntermediateShardHeader {
        let Self { mut buffer } = self;
        append_seal(&mut buffer, seal);

        OwnedIntermediateShardHeader {
            buffer: buffer.into_shared(),
        }
    }
}

/// An owned version of [`LeafShardHeader`].
///
/// It is correctly aligned in memory and well suited for sending and receiving over the network
/// efficiently or storing in memory or on disk.
#[derive(Debug, Clone)]
pub struct OwnedLeafShardHeader {
    buffer: SharedAlignedBuffer,
}

impl OwnedLeafShardHeader {
    /// Max allocation needed by this header
    pub const MAX_ALLOCATION: u32 = BlockHeaderPrefix::SIZE
        + BlockHeaderResult::SIZE
        + BlockHeaderConsensusInfo::SIZE
        + BlockHeaderBeaconChainInfo::SIZE
        + BlockHeaderSealRef::MAX_SIZE;

    /// Create new [`OwnedLeafShardHeader`] from its parts
    pub fn from_parts(
        prefix: &BlockHeaderPrefix,
        result: &BlockHeaderResult,
        consensus_info: &BlockHeaderConsensusInfo,
        beacon_chain_info: &BlockHeaderBeaconChainInfo,
    ) -> OwnedLeafShardBlockHeaderUnsealed {
        let mut buffer = OwnedAlignedBuffer::with_capacity(Self::MAX_ALLOCATION);

        Self::from_parts_into(
            prefix,
            result,
            consensus_info,
            beacon_chain_info,
            &mut buffer,
        );

        OwnedLeafShardBlockHeaderUnsealed { buffer }
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

    /// Create owned block header from a reference
    #[inline]
    pub fn from_header(header: LeafShardHeader<'_>) -> Self {
        let unsealed = Self::from_parts(
            header.generic.prefix,
            header.generic.result,
            header.generic.consensus_info,
            header.beacon_chain_info,
        );

        unsealed.with_seal(header.generic.seal)
    }

    /// Create owned header from a buffer
    #[inline]
    pub fn from_buffer(buffer: SharedAlignedBuffer) -> Result<Self, SharedAlignedBuffer> {
        let Some((_header, extra_bytes)) = LeafShardHeader::try_from_bytes(buffer.as_slice())
        else {
            return Err(buffer);
        };
        if !extra_bytes.is_empty() {
            return Err(buffer);
        }

        Ok(Self { buffer })
    }

    /// Inner buffer with block header contents
    pub fn buffer(&self) -> &SharedAlignedBuffer {
        &self.buffer
    }

    /// Get [`LeafShardHeader`] out of [`OwnedLeafShardHeader`]
    pub fn header(&self) -> LeafShardHeader<'_> {
        LeafShardHeader::try_from_bytes_unchecked(self.buffer.as_slice())
            .expect("Constructor ensures validity; qed")
            .0
    }
}

/// Owned leaf shard block header, which is not sealed yet
#[derive(Debug, Clone)]
pub struct OwnedLeafShardBlockHeaderUnsealed {
    buffer: OwnedAlignedBuffer,
}

impl OwnedLeafShardBlockHeaderUnsealed {
    /// Hash of the block before seal is applied to it
    #[inline(always)]
    pub fn pre_seal_hash(&self) -> Blake3Hash {
        // TODO: Keyed hash with `block_header_seal` as a key
        Blake3Hash::from(blake3::hash(self.buffer.as_slice()))
    }

    /// Add seal and return [`OwnedLeafShardHeader`]
    pub fn with_seal(self, seal: BlockHeaderSealRef<'_>) -> OwnedLeafShardHeader {
        let Self { mut buffer } = self;
        append_seal(&mut buffer, seal);

        OwnedLeafShardHeader {
            buffer: buffer.into_shared(),
        }
    }
}

/// Errors for [`OwnedBlockHeader`]
#[derive(Debug, thiserror::Error)]
pub enum OwnedBlockHeaderError {
    /// Beacon chain block header error
    #[error("Beacon chain block header error: {0}")]
    BeaconChain(#[from] OwnedBeaconChainHeaderError),
    /// Intermediate shard block header error
    #[error("Intermediate shard block header error: {0}")]
    IntermediateShard(#[from] OwnedIntermediateShardHeaderError),
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
    /// Create owned block header from a reference
    #[inline]
    pub fn from_header(header: BlockHeader<'_>) -> Result<Self, OwnedBlockHeaderError> {
        Ok(match header {
            BlockHeader::BeaconChain(header) => {
                Self::BeaconChain(OwnedBeaconChainHeader::from_header(header)?)
            }
            BlockHeader::IntermediateShard(header) => {
                Self::IntermediateShard(OwnedIntermediateShardHeader::from_header(header)?)
            }
            BlockHeader::LeafShard(header) => {
                Self::LeafShard(OwnedLeafShardHeader::from_header(header))
            }
        })
    }

    /// Create owned header from a buffer
    #[inline]
    pub fn from_buffer(
        buffer: SharedAlignedBuffer,
        shard_kind: ShardKind,
    ) -> Result<Self, SharedAlignedBuffer> {
        let Some((_header, extra_bytes)) =
            BlockHeader::try_from_bytes(buffer.as_slice(), shard_kind)
        else {
            return Err(buffer);
        };
        if !extra_bytes.is_empty() {
            return Err(buffer);
        }

        Ok(match shard_kind {
            ShardKind::BeaconChain => Self::BeaconChain(OwnedBeaconChainHeader { buffer }),
            ShardKind::IntermediateShard => {
                Self::IntermediateShard(OwnedIntermediateShardHeader { buffer })
            }
            ShardKind::LeafShard => Self::LeafShard(OwnedLeafShardHeader { buffer }),
            ShardKind::Phantom | ShardKind::Invalid => {
                // Blocks for such shards do not exist
                return Err(buffer);
            }
        })
    }

    /// Inner buffer block header contents
    pub fn buffer(&self) -> &SharedAlignedBuffer {
        match self {
            Self::BeaconChain(owned_header) => owned_header.buffer(),
            Self::IntermediateShard(owned_header) => owned_header.buffer(),
            Self::LeafShard(owned_header) => owned_header.buffer(),
        }
    }

    /// Get [`BlockHeader`] out of [`OwnedBlockHeader`]
    pub fn header(&self) -> BlockHeader<'_> {
        match self {
            Self::BeaconChain(owned_header) => BlockHeader::BeaconChain(owned_header.header()),
            Self::IntermediateShard(owned_header) => {
                BlockHeader::IntermediateShard(owned_header.header())
            }
            Self::LeafShard(owned_header) => BlockHeader::LeafShard(owned_header.header()),
        }
    }
}
