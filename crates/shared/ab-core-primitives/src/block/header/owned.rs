//! Data structures related to the owned version of [`BlockHeader`]

// Slow under Miri and these features don't change what the tests exercise, so under Miri they only
// run with just `alloc` or with all features (like when testing the whole workspace)
#[cfg(all(
    test,
    any(
        not(miri),
        not(any(
            feature = "ed25519",
            feature = "parallel",
            feature = "scale-codec",
            feature = "serde"
        )),
        all(
            feature = "ed25519",
            feature = "parallel",
            feature = "scale-codec",
            feature = "serde"
        ),
    ),
))]
mod tests;

use crate::block::header::{
    BeaconChainHeader, BlockHeader, BlockHeaderBeaconChainInfo, BlockHeaderConsensusInfo,
    BlockHeaderConsensusParameters, BlockHeaderConsistencyError,
    BlockHeaderFixedConsensusParameters, BlockHeaderPrefix, BlockHeaderResult, BlockHeaderSeal,
    BlockHeaderSealType, GenericBlockHeader, IntermediateShardHeader, LeafShardHeader,
};
use crate::hashes::Blake3Hash;
use crate::shard::{NumShardsUnchecked, RealShardKind};
use ab_aligned_buffer::alignment::Align16;
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
    /// Header parts are not consistent with each other or with the shard kind
    #[error("Invalid header parts: {error}")]
    InvalidParts {
        /// Block header consistency error
        #[from]
        error: BlockHeaderConsistencyError,
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

    /// Create a new [`OwnedBeaconChainHeader`] from its parts.
    ///
    /// Returns an error if the parts are not consistent with each other or with the shard kind.
    pub fn from_parts(
        prefix: &BlockHeaderPrefix,
        result: &BlockHeaderResult,
        consensus_info: &BlockHeaderConsensusInfo,
        intermediate_shard_blocks_root: &Blake3Hash,
        consensus_parameters: &BlockHeaderConsensusParameters<'_>,
    ) -> Result<OwnedBeaconChainHeaderUnsealed, BlockHeaderConsistencyError> {
        BlockHeader::check_shared_parts(prefix, consensus_info, RealShardKind::BeaconChain)?;

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

        Ok(OwnedBeaconChainHeaderUnsealed {
            buffer,
            public_key_hash: consensus_info.solution.public_key_hash,
        })
    }

    /// Write a header from its parts into the provided buffer, after the bytes already there.
    ///
    /// The length of the buffer must be a multiple of 8 for the header to parse.
    /// Returns an error if the parts are not consistent with each other or with the shard kind.
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
        BlockHeader::check_shared_parts(prefix, consensus_info, RealShardKind::BeaconChain)?;

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
            public_key_hash: consensus_info.solution.public_key_hash,
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
        // that neither the appends below nor the seal appended later can fail. The length of a
        // buffer is never larger than its maximum capacity.
        if OwnedAlignedBuffer::<Align16>::MAX_CAPACITY - buffer.len() < Self::MAX_ALLOCATION {
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
    /// Public key hash of the solution, the seal must correspond to it
    public_key_hash: Blake3Hash,
}

impl OwnedBeaconChainHeaderUnsealed {
    /// Hash of the block before seal is applied to it
    #[inline(always)]
    pub fn pre_seal_hash(&self) -> Blake3Hash {
        // TODO: Keyed hash with `block_header_seal` as a key
        Blake3Hash::from(blake3::hash(self.buffer.as_slice()))
    }

    /// Add seal and return [`OwnedBeaconChainHeader`].
    ///
    /// Returns an error if the seal doesn't correspond to the public key hash of the solution.
    pub fn with_seal(
        self,
        seal: BlockHeaderSeal<'_>,
    ) -> Result<OwnedBeaconChainHeader, BlockHeaderConsistencyError> {
        let Self {
            mut buffer,
            public_key_hash,
        } = self;
        BlockHeader::check_seal(&public_key_hash, seal)?;
        append_seal(&mut buffer, seal);

        let header = OwnedBeaconChainHeader::from_buffer(buffer.into_shared())
            .expect("`from_parts()` checked the parts it wrote, the seal is checked above; qed");

        Ok(header)
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
    /// Public key hash of the solution, the seal must correspond to it
    public_key_hash: Blake3Hash,
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

    /// Attach the seal after the header in the buffer, which completes the header.
    ///
    /// Returns an error if the seal doesn't correspond to the public key hash of the solution. The
    /// seal is not attached then, and the buffer ends with the unsealed header, which must not be
    /// treated as a complete header.
    #[inline(always)]
    pub fn with_seal(self, seal: BlockHeaderSeal<'_>) -> Result<(), BlockHeaderConsistencyError> {
        let Self {
            buffer,
            header_offset: _,
            public_key_hash,
        } = self;
        BlockHeader::check_seal(&public_key_hash, seal)?;
        append_seal(buffer, seal);

        Ok(())
    }
}

/// Errors for [`OwnedIntermediateShardHeader`]
#[derive(Debug, thiserror::Error)]
pub enum OwnedIntermediateShardHeaderError {
    /// Header doesn't fit into the buffer after the bytes already there
    #[error("The header is too large")]
    HeaderTooLarge,
    /// Header parts are not consistent with each other or with the shard kind
    #[error("Invalid header parts: {error}")]
    InvalidParts {
        /// Block header consistency error
        #[from]
        error: BlockHeaderConsistencyError,
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

    /// Create a new [`OwnedIntermediateShardHeader`] from its parts.
    ///
    /// Returns an error if the parts are not consistent with each other or with the shard kind.
    pub fn from_parts(
        prefix: &BlockHeaderPrefix,
        result: &BlockHeaderResult,
        consensus_info: &BlockHeaderConsensusInfo,
        beacon_chain_info: &BlockHeaderBeaconChainInfo,
        leaf_shard_blocks_root: &Blake3Hash,
    ) -> Result<OwnedIntermediateShardHeaderUnsealed, BlockHeaderConsistencyError> {
        BlockHeader::check_shared_parts(prefix, consensus_info, RealShardKind::IntermediateShard)?;

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

        Ok(OwnedIntermediateShardHeaderUnsealed {
            buffer,
            public_key_hash: consensus_info.solution.public_key_hash,
        })
    }

    /// Write a header from its parts into the provided buffer, after the bytes already there.
    ///
    /// The length of the buffer must be a multiple of 8 for the header to parse.
    /// Returns an error if the parts are not consistent with each other or with the shard kind.
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
        BlockHeader::check_shared_parts(prefix, consensus_info, RealShardKind::IntermediateShard)?;

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
            public_key_hash: consensus_info.solution.public_key_hash,
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
        // that neither the appends below nor the seal appended later can fail. The length of a
        // buffer is never larger than its maximum capacity.
        if OwnedAlignedBuffer::<Align16>::MAX_CAPACITY - buffer.len() < Self::MAX_ALLOCATION {
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
    /// Public key hash of the solution, the seal must correspond to it
    public_key_hash: Blake3Hash,
}

impl OwnedIntermediateShardHeaderUnsealed {
    /// Hash of the block before seal is applied to it
    #[inline(always)]
    pub fn pre_seal_hash(&self) -> Blake3Hash {
        // TODO: Keyed hash with `block_header_seal` as a key
        Blake3Hash::from(blake3::hash(self.buffer.as_slice()))
    }

    /// Add seal and return [`OwnedIntermediateShardHeader`].
    ///
    /// Returns an error if the seal doesn't correspond to the public key hash of the solution.
    pub fn with_seal(
        self,
        seal: BlockHeaderSeal<'_>,
    ) -> Result<OwnedIntermediateShardHeader, BlockHeaderConsistencyError> {
        let Self {
            mut buffer,
            public_key_hash,
        } = self;
        BlockHeader::check_seal(&public_key_hash, seal)?;
        append_seal(&mut buffer, seal);

        let header = OwnedIntermediateShardHeader::from_buffer(buffer.into_shared())
            .expect("`from_parts()` checked the parts it wrote, the seal is checked above; qed");

        Ok(header)
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
    /// Public key hash of the solution, the seal must correspond to it
    public_key_hash: Blake3Hash,
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

    /// Attach the seal after the header in the buffer, which completes the header.
    ///
    /// Returns an error if the seal doesn't correspond to the public key hash of the solution. The
    /// seal is not attached then, and the buffer ends with the unsealed header, which must not be
    /// treated as a complete header.
    #[inline(always)]
    pub fn with_seal(self, seal: BlockHeaderSeal<'_>) -> Result<(), BlockHeaderConsistencyError> {
        let Self {
            buffer,
            header_offset: _,
            public_key_hash,
        } = self;
        BlockHeader::check_seal(&public_key_hash, seal)?;
        append_seal(buffer, seal);

        Ok(())
    }
}

/// Errors for [`OwnedLeafShardHeader`]
#[derive(Debug, thiserror::Error)]
pub enum OwnedLeafShardHeaderError {
    /// Header doesn't fit into the buffer after the bytes already there
    #[error("The header is too large")]
    HeaderTooLarge,
    /// Header parts are not consistent with each other or with the shard kind
    #[error("Invalid header parts: {error}")]
    InvalidParts {
        /// Block header consistency error
        #[from]
        error: BlockHeaderConsistencyError,
    },
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

    /// Create a new [`OwnedLeafShardHeader`] from its parts.
    ///
    /// Returns an error if the parts are not consistent with each other or with the shard kind.
    pub fn from_parts(
        prefix: &BlockHeaderPrefix,
        result: &BlockHeaderResult,
        consensus_info: &BlockHeaderConsensusInfo,
        beacon_chain_info: &BlockHeaderBeaconChainInfo,
    ) -> Result<OwnedLeafShardHeaderUnsealed, BlockHeaderConsistencyError> {
        BlockHeader::check_shared_parts(prefix, consensus_info, RealShardKind::LeafShard)?;

        let mut buffer = OwnedAlignedBuffer::with_capacity(Self::MAX_ALLOCATION);

        Self::append_unsealed(
            prefix,
            result,
            consensus_info,
            beacon_chain_info,
            &mut buffer,
        )
        .expect("The header with the largest seal fits into an empty buffer; qed");

        Ok(OwnedLeafShardHeaderUnsealed {
            buffer,
            public_key_hash: consensus_info.solution.public_key_hash,
        })
    }

    /// Write a header from its parts into the provided buffer, after the bytes already there.
    ///
    /// The length of the buffer must be a multiple of 8 for the header to parse.
    /// Returns an error if the parts are not consistent with each other or with the shard kind.
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
        BlockHeader::check_shared_parts(prefix, consensus_info, RealShardKind::LeafShard)?;

        let header_offset = buffer.len();

        Self::append_unsealed(prefix, result, consensus_info, beacon_chain_info, buffer)?;

        Ok(OwnedLeafShardHeaderUnsealedRef {
            buffer,
            header_offset,
            public_key_hash: consensus_info.solution.public_key_hash,
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
        // that neither the appends below nor the seal appended later can fail. The length of a
        // buffer is never larger than its maximum capacity.
        if OwnedAlignedBuffer::<Align16>::MAX_CAPACITY - buffer.len() < Self::MAX_ALLOCATION {
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
    /// Public key hash of the solution, the seal must correspond to it
    public_key_hash: Blake3Hash,
}

impl OwnedLeafShardHeaderUnsealed {
    /// Hash of the block before seal is applied to it
    #[inline(always)]
    pub fn pre_seal_hash(&self) -> Blake3Hash {
        // TODO: Keyed hash with `block_header_seal` as a key
        Blake3Hash::from(blake3::hash(self.buffer.as_slice()))
    }

    /// Add seal and return [`OwnedLeafShardHeader`].
    ///
    /// Returns an error if the seal doesn't correspond to the public key hash of the solution.
    pub fn with_seal(
        self,
        seal: BlockHeaderSeal<'_>,
    ) -> Result<OwnedLeafShardHeader, BlockHeaderConsistencyError> {
        let Self {
            mut buffer,
            public_key_hash,
        } = self;
        BlockHeader::check_seal(&public_key_hash, seal)?;
        append_seal(&mut buffer, seal);

        let header = OwnedLeafShardHeader::from_buffer(buffer.into_shared())
            .expect("`from_parts()` checked the parts it wrote, the seal is checked above; qed");

        Ok(header)
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
    /// Public key hash of the solution, the seal must correspond to it
    public_key_hash: Blake3Hash,
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

    /// Attach the seal after the header in the buffer, which completes the header.
    ///
    /// Returns an error if the seal doesn't correspond to the public key hash of the solution. The
    /// seal is not attached then, and the buffer ends with the unsealed header, which must not be
    /// treated as a complete header.
    #[inline(always)]
    pub fn with_seal(self, seal: BlockHeaderSeal<'_>) -> Result<(), BlockHeaderConsistencyError> {
        let Self {
            buffer,
            header_offset: _,
            public_key_hash,
        } = self;
        BlockHeader::check_seal(&public_key_hash, seal)?;
        append_seal(buffer, seal);

        Ok(())
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
