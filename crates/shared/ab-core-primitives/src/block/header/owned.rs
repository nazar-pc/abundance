//! Data structures related to the owned version of [`BlockHeader`]

#[cfg(test)]
mod tests;

use crate::block::header::{
    BeaconChainHeader, BlockHeader, BlockHeaderBeaconChainInfo, BlockHeaderConsensusInfo,
    BlockHeaderConsensusParameters, BlockHeaderFixedConsensusParameters, BlockHeaderPrefix,
    BlockHeaderResult, BlockHeaderSeal, BlockHeaderSealType, GenericBlockHeader,
    IntermediateShardHeader, LeafShardHeader,
};
use crate::hashes::Blake3Hash;
use crate::shard::{NumShardsUnchecked, RealShardKind};
use ab_aligned_buffer::{OwnedAlignedBuffer, SharedAlignedBuffer};
use ab_io_type::trivial_type::TrivialType;
use core::fmt;
use derive_more::From;
use triomphe::Arc;
use yoke::Yoke;

/// Generic owned block header
pub trait GenericOwnedBlockHeader:
    Clone + fmt::Debug + Send + Sync + Into<OwnedBlockHeader> + 'static
{
    /// Shard kind
    const SHARD_KIND: RealShardKind;

    /// Block header
    type Header<'a>: GenericBlockHeader<'a>
    where
        Self: 'a;

    /// Create an owned header from a buffer
    fn from_buffer(buffer: SharedAlignedBuffer) -> Result<Self, SharedAlignedBuffer>;

    /// Inner buffer with block header contents
    fn buffer(&self) -> &SharedAlignedBuffer;

    /// Number of clones in memory
    fn ref_count(&self) -> usize;

    /// Get a regular block header out of the owned version
    fn header(&self) -> &Self::Header<'_>;
}

fn append_seal(buffer: &mut OwnedAlignedBuffer, seal: BlockHeaderSeal<'_>) {
    match seal {
        BlockHeaderSeal::Ed25519(seal) => {
            let true = buffer.append(&[BlockHeaderSealType::Ed25519 as u8]) else {
                unreachable!("Unsealed headers are only created when the largest seal fits; qed");
            };
            let true = buffer.append(seal.as_bytes()) else {
                unreachable!("Unsealed headers are only created when the largest seal fits; qed");
            };
        }
    }
}

/// Errors for [`OwnedBeaconChainHeader`]
#[derive(Debug, thiserror::Error)]
pub enum OwnedBeaconChainHeaderError {
    /// Header with the largest possible consensus parameters and seal doesn't fit into the buffer
    /// after the bytes already there
    #[error("The header is too large")]
    HeaderTooLarge,
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
    const SHARD_KIND: RealShardKind = RealShardKind::BeaconChain;

    type Header<'a> = BeaconChainHeader<'a>;

    #[inline(always)]
    fn from_buffer(buffer: SharedAlignedBuffer) -> Result<Self, SharedAlignedBuffer> {
        Self::from_buffer(buffer)
    }

    #[inline(always)]
    fn buffer(&self) -> &SharedAlignedBuffer {
        self.buffer()
    }

    #[inline(always)]
    fn ref_count(&self) -> usize {
        self.ref_count()
    }

    #[inline(always)]
    fn header(&self) -> &Self::Header<'_> {
        self.header()
    }
}

impl OwnedBeaconChainHeader {
    /// Max allocation needed by this header
    pub const MAX_ALLOCATION: u32 = BlockHeaderPrefix::SIZE
        + BlockHeaderResult::SIZE
        + BlockHeaderConsensusInfo::SIZE
        + <Blake3Hash as TrivialType>::SIZE
        + BlockHeaderConsensusParameters::MAX_SIZE
        + BlockHeaderSeal::MAX_SIZE;

    /// Create a new [`OwnedBeaconChainHeader`] from its parts
    pub fn from_parts(
        prefix: &BlockHeaderPrefix,
        result: &BlockHeaderResult,
        consensus_info: &BlockHeaderConsensusInfo,
        intermediate_shard_blocks_root: &Blake3Hash,
        consensus_parameters: &BlockHeaderConsensusParameters<'_>,
    ) -> OwnedBeaconChainHeaderUnsealed {
        let mut buffer = OwnedAlignedBuffer::with_capacity(Self::MAX_ALLOCATION);

        Self::append_unsealed(
            prefix,
            result,
            consensus_info,
            intermediate_shard_blocks_root,
            consensus_parameters,
            &mut buffer,
        )
        .expect("The header with the largest seal fits into an empty buffer; qed");

        OwnedBeaconChainHeaderUnsealed { buffer }
    }

    /// Write a header from its parts into the provided buffer, after the bytes already there.
    ///
    /// The seal is not attached, the header in the buffer is complete only after
    /// [`OwnedBeaconChainHeaderUnsealedRef::with_seal()`] attaches it. Nothing is written when an
    /// error is returned, otherwise the seal is guaranteed to fit.
    pub fn from_parts_into<'a>(
        prefix: &BlockHeaderPrefix,
        result: &BlockHeaderResult,
        consensus_info: &BlockHeaderConsensusInfo,
        intermediate_shard_blocks_root: &Blake3Hash,
        consensus_parameters: &BlockHeaderConsensusParameters<'_>,
        buffer: &'a mut OwnedAlignedBuffer,
    ) -> Result<OwnedBeaconChainHeaderUnsealedRef<'a>, OwnedBeaconChainHeaderError> {
        let header_offset = buffer.len();

        Self::append_unsealed(
            prefix,
            result,
            consensus_info,
            intermediate_shard_blocks_root,
            consensus_parameters,
            buffer,
        )?;

        Ok(OwnedBeaconChainHeaderUnsealedRef {
            buffer,
            header_offset,
        })
    }

    /// Append everything except the seal to the buffer
    fn append_unsealed(
        prefix: &BlockHeaderPrefix,
        result: &BlockHeaderResult,
        consensus_info: &BlockHeaderConsensusInfo,
        intermediate_shard_blocks_root: &Blake3Hash,
        consensus_parameters: &BlockHeaderConsensusParameters<'_>,
        buffer: &mut OwnedAlignedBuffer,
    ) -> Result<(), OwnedBeaconChainHeaderError> {
        let BlockHeaderConsensusParameters {
            fixed_parameters,
            super_segment_root,
            next_solution_range,
            pot_parameters_change,
        } = consensus_parameters;
        let BlockHeaderFixedConsensusParameters {
            solution_range,
            slot_iterations,
            num_shards,
        } = fixed_parameters;

        // The header with the largest seal must fit after the bytes already in the buffer, so
        // that neither the appends below nor the seal appended later can fail
        if buffer.len().checked_add(Self::MAX_ALLOCATION).is_none() {
            return Err(OwnedBeaconChainHeaderError::HeaderTooLarge);
        }
        let true = buffer.append(prefix.as_bytes()) else {
            unreachable!("Checked size above; qed");
        };
        let true = buffer.append(result.as_bytes()) else {
            unreachable!("Checked size above; qed");
        };
        let true = buffer.append(consensus_info.as_bytes()) else {
            unreachable!("Checked size above; qed");
        };
        let true = buffer.append(intermediate_shard_blocks_root.as_bytes()) else {
            unreachable!("Checked size above; qed");
        };
        // TODO: Would be nice for `BlockHeaderConsensusParameters` to have API to write this by
        //  itself
        {
            let true = buffer.append(&solution_range.to_bytes()) else {
                unreachable!("Checked size above; qed");
            };
            let true = buffer.append(&slot_iterations.get().to_le_bytes()) else {
                unreachable!("Checked size above; qed");
            };
            let true = buffer.append(NumShardsUnchecked::from(*num_shards).as_bytes()) else {
                unreachable!("Checked size above; qed");
            };

            let bitflags = {
                let mut bitflags = 0u8;

                if super_segment_root.is_some() {
                    bitflags |= BlockHeaderConsensusParameters::SUPER_SEGMENT_ROOT_MASK;
                }
                if next_solution_range.is_some() {
                    bitflags |= BlockHeaderConsensusParameters::NEXT_SOLUTION_RANGE_MASK;
                }
                if pot_parameters_change.is_some() {
                    bitflags |= BlockHeaderConsensusParameters::POT_PARAMETERS_CHANGE_MASK;
                }

                bitflags
            };

            let true = buffer.append(&[bitflags]) else {
                unreachable!("Checked size above; qed");
            };

            if let Some(super_segment_root) = super_segment_root {
                let true = buffer.append(super_segment_root.as_ref()) else {
                    unreachable!("Checked size above; qed");
                };
            }

            if let Some(next_solution_range) = next_solution_range {
                let true = buffer.append(&next_solution_range.to_bytes()) else {
                    unreachable!("Checked size above; qed");
                };
            }

            if let Some(pot_parameters_change) = pot_parameters_change {
                let true = buffer.append(&pot_parameters_change.slot.to_bytes()) else {
                    unreachable!("Checked size above; qed");
                };
                let true =
                    buffer.append(&pot_parameters_change.slot_iterations.get().to_le_bytes())
                else {
                    unreachable!("Checked size above; qed");
                };
                let true = buffer.append(pot_parameters_change.entropy.as_ref()) else {
                    unreachable!("Checked size above; qed");
                };
            }
        }

        Ok(())
    }

    /// Create an owned header from a buffer
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

    /// Number of clones in memory
    #[inline(always)]
    pub fn ref_count(&self) -> usize {
        Arc::count(&self.inner)
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

/// Beacon chain block header written into a buffer by
/// [`OwnedBeaconChainHeader::from_parts_into()`], which is not sealed yet.
///
/// The header in the buffer is not complete until [`Self::with_seal()`] attaches the seal.
#[derive(Debug)]
#[must_use = "The header in the buffer is not complete until `with_seal()` attaches the seal"]
pub struct OwnedBeaconChainHeaderUnsealedRef<'a> {
    buffer: &'a mut OwnedAlignedBuffer,
    header_offset: u32,
}

impl OwnedBeaconChainHeaderUnsealedRef<'_> {
    /// Hash of the block before seal is applied to it
    #[inline(always)]
    pub fn pre_seal_hash(&self) -> Blake3Hash {
        let header_bytes = self
            .buffer
            .as_slice()
            .get(self.header_offset as usize..)
            .expect("The buffer only grew since `header_offset` was its length; qed");
        // TODO: Keyed hash with `block_header_seal` as a key
        Blake3Hash::from(blake3::hash(header_bytes))
    }

    /// Attach the seal after the header in the buffer, which completes the header
    #[inline(always)]
    pub fn with_seal(self, seal: BlockHeaderSeal<'_>) {
        append_seal(self.buffer, seal);
    }
}

/// Errors for [`OwnedIntermediateShardHeader`]
#[derive(Debug, thiserror::Error)]
pub enum OwnedIntermediateShardHeaderError {
    /// Header doesn't fit into the buffer after the bytes already there
    #[error("The header is too large")]
    HeaderTooLarge,
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
    const SHARD_KIND: RealShardKind = RealShardKind::IntermediateShard;

    type Header<'a> = IntermediateShardHeader<'a>;

    #[inline(always)]
    fn from_buffer(buffer: SharedAlignedBuffer) -> Result<Self, SharedAlignedBuffer> {
        Self::from_buffer(buffer)
    }

    #[inline(always)]
    fn buffer(&self) -> &SharedAlignedBuffer {
        self.buffer()
    }

    #[inline(always)]
    fn ref_count(&self) -> usize {
        self.ref_count()
    }

    #[inline(always)]
    fn header(&self) -> &Self::Header<'_> {
        self.header()
    }
}

impl OwnedIntermediateShardHeader {
    /// Max allocation needed by this header
    pub const MAX_ALLOCATION: u32 = BlockHeaderPrefix::SIZE
        + BlockHeaderResult::SIZE
        + BlockHeaderConsensusInfo::SIZE
        + BlockHeaderBeaconChainInfo::SIZE
        + <Blake3Hash as TrivialType>::SIZE
        + BlockHeaderSeal::MAX_SIZE;

    /// Create a new [`OwnedIntermediateShardHeader`] from its parts
    pub fn from_parts(
        prefix: &BlockHeaderPrefix,
        result: &BlockHeaderResult,
        consensus_info: &BlockHeaderConsensusInfo,
        beacon_chain_info: &BlockHeaderBeaconChainInfo,
        leaf_shard_blocks_root: &Blake3Hash,
    ) -> OwnedIntermediateShardHeaderUnsealed {
        let mut buffer = OwnedAlignedBuffer::with_capacity(Self::MAX_ALLOCATION);

        Self::append_unsealed(
            prefix,
            result,
            consensus_info,
            beacon_chain_info,
            leaf_shard_blocks_root,
            &mut buffer,
        )
        .expect("The header with the largest seal fits into an empty buffer; qed");

        OwnedIntermediateShardHeaderUnsealed { buffer }
    }

    /// Write a header from its parts into the provided buffer, after the bytes already there.
    ///
    /// The seal is not attached, the header in the buffer is complete only after
    /// [`OwnedIntermediateShardHeaderUnsealedRef::with_seal()`] attaches it. Nothing is written
    /// when an error is returned, otherwise the seal is guaranteed to fit.
    pub fn from_parts_into<'a>(
        prefix: &BlockHeaderPrefix,
        result: &BlockHeaderResult,
        consensus_info: &BlockHeaderConsensusInfo,
        beacon_chain_info: &BlockHeaderBeaconChainInfo,
        leaf_shard_blocks_root: &Blake3Hash,
        buffer: &'a mut OwnedAlignedBuffer,
    ) -> Result<OwnedIntermediateShardHeaderUnsealedRef<'a>, OwnedIntermediateShardHeaderError>
    {
        let header_offset = buffer.len();

        Self::append_unsealed(
            prefix,
            result,
            consensus_info,
            beacon_chain_info,
            leaf_shard_blocks_root,
            buffer,
        )?;

        Ok(OwnedIntermediateShardHeaderUnsealedRef {
            buffer,
            header_offset,
        })
    }

    /// Append everything except the seal to the buffer
    fn append_unsealed(
        prefix: &BlockHeaderPrefix,
        result: &BlockHeaderResult,
        consensus_info: &BlockHeaderConsensusInfo,
        beacon_chain_info: &BlockHeaderBeaconChainInfo,
        leaf_shard_blocks_root: &Blake3Hash,
        buffer: &mut OwnedAlignedBuffer,
    ) -> Result<(), OwnedIntermediateShardHeaderError> {
        // The header with the largest seal must fit after the bytes already in the buffer, so
        // that neither the appends below nor the seal appended later can fail
        if buffer.len().checked_add(Self::MAX_ALLOCATION).is_none() {
            return Err(OwnedIntermediateShardHeaderError::HeaderTooLarge);
        }
        let true = buffer.append(prefix.as_bytes()) else {
            unreachable!("Checked size above; qed");
        };
        let true = buffer.append(result.as_bytes()) else {
            unreachable!("Checked size above; qed");
        };
        let true = buffer.append(consensus_info.as_bytes()) else {
            unreachable!("Checked size above; qed");
        };
        let true = buffer.append(beacon_chain_info.as_bytes()) else {
            unreachable!("Checked size above; qed");
        };
        let true = buffer.append(leaf_shard_blocks_root.as_bytes()) else {
            unreachable!("Checked size above; qed");
        };

        Ok(())
    }

    /// Create an owned header from a buffer
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

    /// Number of clones in memory
    #[inline(always)]
    pub fn ref_count(&self) -> usize {
        Arc::count(&self.inner)
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

/// Intermediate shard block header written into a buffer by
/// [`OwnedIntermediateShardHeader::from_parts_into()`], which is not sealed yet.
///
/// The header in the buffer is not complete until [`Self::with_seal()`] attaches the seal.
#[derive(Debug)]
#[must_use = "The header in the buffer is not complete until `with_seal()` attaches the seal"]
pub struct OwnedIntermediateShardHeaderUnsealedRef<'a> {
    buffer: &'a mut OwnedAlignedBuffer,
    header_offset: u32,
}

impl OwnedIntermediateShardHeaderUnsealedRef<'_> {
    /// Hash of the block before seal is applied to it
    #[inline(always)]
    pub fn pre_seal_hash(&self) -> Blake3Hash {
        let header_bytes = self
            .buffer
            .as_slice()
            .get(self.header_offset as usize..)
            .expect("The buffer only grew since `header_offset` was its length; qed");
        // TODO: Keyed hash with `block_header_seal` as a key
        Blake3Hash::from(blake3::hash(header_bytes))
    }

    /// Attach the seal after the header in the buffer, which completes the header
    #[inline(always)]
    pub fn with_seal(self, seal: BlockHeaderSeal<'_>) {
        append_seal(self.buffer, seal);
    }
}

/// Errors for [`OwnedLeafShardHeader`]
#[derive(Debug, thiserror::Error)]
pub enum OwnedLeafShardHeaderError {
    /// Header doesn't fit into the buffer after the bytes already there
    #[error("The header is too large")]
    HeaderTooLarge,
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
    const SHARD_KIND: RealShardKind = RealShardKind::LeafShard;

    type Header<'a> = LeafShardHeader<'a>;

    #[inline(always)]
    fn from_buffer(buffer: SharedAlignedBuffer) -> Result<Self, SharedAlignedBuffer> {
        Self::from_buffer(buffer)
    }

    #[inline(always)]
    fn buffer(&self) -> &SharedAlignedBuffer {
        self.buffer()
    }

    #[inline(always)]
    fn ref_count(&self) -> usize {
        self.ref_count()
    }

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

    /// Create a new [`OwnedLeafShardHeader`] from its parts
    pub fn from_parts(
        prefix: &BlockHeaderPrefix,
        result: &BlockHeaderResult,
        consensus_info: &BlockHeaderConsensusInfo,
        beacon_chain_info: &BlockHeaderBeaconChainInfo,
    ) -> OwnedLeafShardHeaderUnsealed {
        let mut buffer = OwnedAlignedBuffer::with_capacity(Self::MAX_ALLOCATION);

        Self::append_unsealed(
            prefix,
            result,
            consensus_info,
            beacon_chain_info,
            &mut buffer,
        )
        .expect("The header with the largest seal fits into an empty buffer; qed");

        OwnedLeafShardHeaderUnsealed { buffer }
    }

    /// Write a header from its parts into the provided buffer, after the bytes already there.
    ///
    /// The seal is not attached, the header in the buffer is complete only after
    /// [`OwnedLeafShardHeaderUnsealedRef::with_seal()`] attaches it. Nothing is written when an
    /// error is returned, otherwise the seal is guaranteed to fit.
    pub fn from_parts_into<'a>(
        prefix: &BlockHeaderPrefix,
        result: &BlockHeaderResult,
        consensus_info: &BlockHeaderConsensusInfo,
        beacon_chain_info: &BlockHeaderBeaconChainInfo,
        buffer: &'a mut OwnedAlignedBuffer,
    ) -> Result<OwnedLeafShardHeaderUnsealedRef<'a>, OwnedLeafShardHeaderError> {
        let header_offset = buffer.len();

        Self::append_unsealed(prefix, result, consensus_info, beacon_chain_info, buffer)?;

        Ok(OwnedLeafShardHeaderUnsealedRef {
            buffer,
            header_offset,
        })
    }

    /// Append everything except the seal to the buffer
    fn append_unsealed(
        prefix: &BlockHeaderPrefix,
        result: &BlockHeaderResult,
        consensus_info: &BlockHeaderConsensusInfo,
        beacon_chain_info: &BlockHeaderBeaconChainInfo,
        buffer: &mut OwnedAlignedBuffer,
    ) -> Result<(), OwnedLeafShardHeaderError> {
        // The header with the largest seal must fit after the bytes already in the buffer, so
        // that neither the appends below nor the seal appended later can fail
        if buffer.len().checked_add(Self::MAX_ALLOCATION).is_none() {
            return Err(OwnedLeafShardHeaderError::HeaderTooLarge);
        }
        let true = buffer.append(prefix.as_bytes()) else {
            unreachable!("Checked size above; qed");
        };
        let true = buffer.append(result.as_bytes()) else {
            unreachable!("Checked size above; qed");
        };
        let true = buffer.append(consensus_info.as_bytes()) else {
            unreachable!("Checked size above; qed");
        };
        let true = buffer.append(beacon_chain_info.as_bytes()) else {
            unreachable!("Checked size above; qed");
        };

        Ok(())
    }

    /// Create an owned header from a buffer
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

    /// Number of clones in memory
    #[inline(always)]
    pub fn ref_count(&self) -> usize {
        Arc::count(&self.inner)
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

/// Leaf shard block header written into a buffer by [`OwnedLeafShardHeader::from_parts_into()`],
/// which is not sealed yet.
///
/// The header in the buffer is not complete until [`Self::with_seal()`] attaches the seal.
#[derive(Debug)]
#[must_use = "The header in the buffer is not complete until `with_seal()` attaches the seal"]
pub struct OwnedLeafShardHeaderUnsealedRef<'a> {
    buffer: &'a mut OwnedAlignedBuffer,
    header_offset: u32,
}

impl OwnedLeafShardHeaderUnsealedRef<'_> {
    /// Hash of the block before seal is applied to it
    #[inline(always)]
    pub fn pre_seal_hash(&self) -> Blake3Hash {
        let header_bytes = self
            .buffer
            .as_slice()
            .get(self.header_offset as usize..)
            .expect("The buffer only grew since `header_offset` was its length; qed");
        // TODO: Keyed hash with `block_header_seal` as a key
        Blake3Hash::from(blake3::hash(header_bytes))
    }

    /// Attach the seal after the header in the buffer, which completes the header
    #[inline(always)]
    pub fn with_seal(self, seal: BlockHeaderSeal<'_>) {
        append_seal(self.buffer, seal);
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
    /// Create an owned header from a buffer
    #[inline]
    pub fn from_buffer(
        buffer: SharedAlignedBuffer,
        shard_kind: RealShardKind,
    ) -> Result<Self, SharedAlignedBuffer> {
        Ok(match shard_kind {
            RealShardKind::BeaconChain => {
                Self::BeaconChain(OwnedBeaconChainHeader::from_buffer(buffer)?)
            }
            RealShardKind::IntermediateShard => {
                Self::IntermediateShard(OwnedIntermediateShardHeader::from_buffer(buffer)?)
            }
            RealShardKind::LeafShard => Self::LeafShard(OwnedLeafShardHeader::from_buffer(buffer)?),
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

    /// Number of clones in memory
    #[inline]
    pub fn ref_count(&self) -> usize {
        match self {
            Self::BeaconChain(owned_header) => owned_header.ref_count(),
            Self::IntermediateShard(owned_header) => owned_header.ref_count(),
            Self::LeafShard(owned_header) => owned_header.ref_count(),
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
