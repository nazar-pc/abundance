//! Data structures related to the owned version of [`BlockBody`]

use crate::block::body::{
    BeaconChainBody, BlockBody, IntermediateShardBlockInfo, IntermediateShardBody,
    LeafShardBlockInfo, LeafShardBody,
};
use crate::block::header::owned::{
    OwnedIntermediateShardHeader, OwnedIntermediateShardHeaderError, OwnedLeafShardHeader,
};
use crate::pot::PotCheckpoints;
use crate::segments::SegmentRoot;
use crate::shard::ShardKind;
use crate::transaction::Transaction;
use crate::transaction::owned::{OwnedTransaction, OwnedTransactionError};
use ab_aligned_buffer::{OwnedAlignedBuffer, SharedAlignedBuffer};
use ab_io_type::trivial_type::TrivialType;
use core::iter::TrustedLen;
use derive_more::From;

/// Transaction addition error
#[derive(Debug, thiserror::Error)]
enum AddTransactionError {
    /// Block body is too large
    #[error("Block body is too large")]
    BlockBodyIsTooLarge,
    /// Too many transactions
    #[error("Too many transactions")]
    TooManyTransactions,
    /// Failed to add transaction
    #[error("Failed to add transaction: {error}")]
    FailedToAddTransaction {
        /// Inner error
        #[from]
        error: OwnedTransactionError,
    },
}

/// Transaction that can be written into the body
pub trait WritableBodyTransaction {
    /// Write this transaction into the body
    fn write_into(&self, buffer: &mut OwnedAlignedBuffer) -> Result<(), OwnedTransactionError>;
}

impl WritableBodyTransaction for Transaction<'_> {
    fn write_into(&self, buffer: &mut OwnedAlignedBuffer) -> Result<(), OwnedTransactionError> {
        OwnedTransaction::from_parts_into(
            self.header,
            self.read_slots,
            self.write_slots,
            self.payload,
            self.seal,
            buffer,
        )
    }
}

impl WritableBodyTransaction for &OwnedTransaction {
    fn write_into(&self, buffer: &mut OwnedAlignedBuffer) -> Result<(), OwnedTransactionError> {
        if buffer.append(self.buffer().as_slice()) {
            Ok(())
        } else {
            Err(OwnedTransactionError::TransactionTooLarge)
        }
    }
}

#[derive(Debug, Clone)]
struct TransactionBuilder {
    num_transactions_offset: usize,
    buffer: OwnedAlignedBuffer,
}

impl TransactionBuilder {
    fn new(num_transactions_offset: usize, buffer: OwnedAlignedBuffer) -> Self {
        Self {
            num_transactions_offset,
            buffer,
        }
    }

    /// Add transaction to the body
    fn add_transaction<T>(&mut self, transaction: T) -> Result<(), AddTransactionError>
    where
        T: WritableBodyTransaction,
    {
        // Transactions are aligned, but the very first might come after non-transaction fields that
        // were not aligned
        if self.inc_transaction_count()? == 1 && !align_to_16_bytes_with_padding(&mut self.buffer) {
            self.dec_transaction_count();
            return Err(AddTransactionError::BlockBodyIsTooLarge);
        }

        let old_buffer_len = self.buffer.len();

        transaction
            .write_into(&mut self.buffer)
            .inspect_err(|_error| {
                self.dec_transaction_count();
            })?;

        if !align_to_16_bytes_with_padding(&mut self.buffer) {
            self.dec_transaction_count();
            // Length was obtained from the same buffer before last write
            unsafe {
                self.buffer.set_len(old_buffer_len);
            }
            return Err(AddTransactionError::BlockBodyIsTooLarge);
        }

        Ok(())
    }

    /// Finish building block body
    #[inline(always)]
    fn finish(self) -> OwnedAlignedBuffer {
        self.buffer
    }

    /// Increase the number of stored transactions and return the new value
    #[inline(always)]
    fn inc_transaction_count(&mut self) -> Result<u32, AddTransactionError> {
        // SAFETY: Constructor ensures the offset is valid and has space for `u32` (but not
        // necessarily aligned)
        unsafe {
            let num_transactions_ptr = self
                .buffer
                .as_mut_ptr()
                .add(self.num_transactions_offset)
                .cast::<u32>();
            let num_transactions = num_transactions_ptr.read_unaligned();
            let num_transactions = num_transactions
                .checked_add(1)
                .ok_or(AddTransactionError::TooManyTransactions)?;
            num_transactions_ptr.write_unaligned(num_transactions);
            Ok(num_transactions)
        }
    }

    /// Decrease the number of stored transactions
    #[inline(always)]
    fn dec_transaction_count(&mut self) {
        // SAFETY: Constructor ensures the offset is valid and has space for `u32` (but not
        // necessarily aligned)
        unsafe {
            let num_transactions_ptr = self
                .buffer
                .as_mut_ptr()
                .add(self.num_transactions_offset)
                .cast::<u32>();
            let num_transactions = num_transactions_ptr.read_unaligned();
            let num_transactions = num_transactions.saturating_sub(1);
            num_transactions_ptr.write_unaligned(num_transactions);
        }
    }
}

/// Errors for [`OwnedBeaconChainBody`]
#[derive(Debug, thiserror::Error)]
pub enum OwnedBeaconChainBodyError {
    /// Too many PoT checkpoints
    #[error("Too many PoT checkpoints: {actual}")]
    TooManyPotCheckpoints {
        /// Actual number of PoT checkpoints
        actual: usize,
    },
    /// Too many own segment roots
    #[error("Too many own segment roots: {actual}")]
    TooManyOwnSegmentRoots {
        /// Actual number of own segment roots
        actual: usize,
    },
    /// Too many intermediate shard blocks
    #[error("Too many intermediate shard blocks: {actual}")]
    TooManyIntermediateShardBlocks {
        /// Actual number of intermediate shard blocks
        actual: usize,
    },
    /// Too many intermediate shard own segment roots
    #[error("Too many intermediate shard own segment roots: {actual}")]
    TooManyIntermediateShardOwnSegmentRoots {
        /// Actual number of own segment roots
        actual: usize,
    },
    /// Too many intermediate shard child segment roots
    #[error("Too many intermediate shard child segment roots: {actual}")]
    TooManyIntermediateShardChildSegmentRoots {
        /// Actual number of child segment roots
        actual: usize,
    },
    /// Failed to intermediate shard header
    #[error("Failed to intermediate shard header: {error}")]
    FailedToAddTransaction {
        /// Inner error
        #[from]
        error: OwnedIntermediateShardHeaderError,
    },
    /// Block body is too large
    #[error("Block body is too large")]
    BlockBodyIsTooLarge,
}

/// An owned version of [`BeaconChainBody`].
///
/// It is correctly aligned in memory and well suited for sending and receiving over the network
/// efficiently or storing in memory or on disk.
#[derive(Debug, Clone)]
pub struct OwnedBeaconChainBody {
    buffer: SharedAlignedBuffer,
}

impl OwnedBeaconChainBody {
    /// Initialize building of [`OwnedBeaconChainBody`]
    pub fn init<'a, ISB>(
        own_segment_roots: &[SegmentRoot],
        intermediate_shard_blocks: ISB,
        pot_checkpoints: &[PotCheckpoints],
    ) -> Result<Self, OwnedBeaconChainBodyError>
    where
        ISB: TrustedLen<Item = IntermediateShardBlockInfo<'a>> + Clone + 'a,
    {
        let num_pot_checkpoints = pot_checkpoints.len();
        let num_pot_checkpoints = u32::try_from(num_pot_checkpoints).map_err(|_error| {
            OwnedBeaconChainBodyError::TooManyPotCheckpoints {
                actual: num_pot_checkpoints,
            }
        })?;
        let num_own_segment_roots = own_segment_roots.len();
        let num_own_segment_roots = u8::try_from(num_own_segment_roots).map_err(|_error| {
            OwnedBeaconChainBodyError::TooManyOwnSegmentRoots {
                actual: num_own_segment_roots,
            }
        })?;
        let num_blocks = intermediate_shard_blocks.size_hint().0;
        let num_blocks = u8::try_from(num_blocks).map_err(|_error| {
            OwnedBeaconChainBodyError::TooManyIntermediateShardBlocks { actual: num_blocks }
        })?;

        let mut buffer = OwnedAlignedBuffer::with_capacity(
            u8::SIZE
                + size_of_val(own_segment_roots) as u32
                // This is only an estimate to get in the ballpark where reallocation should not be
                // necessary in many cases
                + u32::from(num_blocks) * OwnedIntermediateShardHeader::max_allocation_for(&[]) * 2,
        );

        let true = buffer.append(&num_pot_checkpoints.to_le_bytes()) else {
            unreachable!("Fixed size data structures that are guaranteed to fit; qed");
        };

        let true = buffer.append(&[num_own_segment_roots]) else {
            unreachable!("Fixed size data structures that are guaranteed to fit; qed");
        };
        let true = buffer.append(SegmentRoot::repr_from_slice(own_segment_roots).as_flattened())
        else {
            unreachable!("Checked size above; qed");
        };
        // TODO: Would be nice for `IntermediateShardBlocksInfo` to have API to write this by itself
        {
            let true = buffer.append(&num_blocks.to_le_bytes()) else {
                unreachable!("Fixed size data structures that are guaranteed to fit; qed");
            };
            let mut segments_roots_num_cursor = buffer.len() as usize;
            for _ in 0..num_blocks {
                let true = buffer.append(&[0, 0, 0]) else {
                    unreachable!("Checked size above; qed");
                };
            }
            let true = align_to_8_with_padding(&mut buffer) else {
                unreachable!("Checked size above; qed");
            };
            for intermediate_shard_block in intermediate_shard_blocks.clone() {
                if !intermediate_shard_block.own_segment_roots.is_empty()
                    || !intermediate_shard_block.child_segment_roots.is_empty()
                {
                    let num_own_segment_roots = intermediate_shard_block.own_segment_roots.len();
                    let num_own_segment_roots =
                        u8::try_from(num_own_segment_roots).map_err(|_error| {
                            OwnedBeaconChainBodyError::TooManyIntermediateShardOwnSegmentRoots {
                                actual: num_own_segment_roots,
                            }
                        })?;
                    let num_child_segment_roots =
                        intermediate_shard_block.child_segment_roots.len();
                    let num_child_segment_roots =
                        u16::try_from(num_child_segment_roots).map_err(|_error| {
                            OwnedBeaconChainBodyError::TooManyIntermediateShardChildSegmentRoots {
                                actual: num_child_segment_roots,
                            }
                        })?;
                    let num_child_segment_roots = num_child_segment_roots.to_le_bytes();
                    buffer.as_mut_slice()[segments_roots_num_cursor..][..3].copy_from_slice(&[
                        num_own_segment_roots,
                        num_child_segment_roots[0],
                        num_child_segment_roots[1],
                    ]);
                }
                segments_roots_num_cursor += 3;

                OwnedIntermediateShardHeader::from_parts_into(
                    intermediate_shard_block.header.prefix,
                    intermediate_shard_block.header.result,
                    intermediate_shard_block.header.consensus_info,
                    intermediate_shard_block.header.beacon_chain_info,
                    &intermediate_shard_block.header.child_shard_blocks,
                    &mut buffer,
                )?;
                if !align_to_8_with_padding(&mut buffer) {
                    return Err(OwnedBeaconChainBodyError::BlockBodyIsTooLarge);
                }
                if let Some(segment_roots_proof) = intermediate_shard_block.segment_roots_proof
                    && !buffer.append(segment_roots_proof)
                {
                    return Err(OwnedBeaconChainBodyError::BlockBodyIsTooLarge);
                }
                if !intermediate_shard_block.own_segment_roots.is_empty()
                    && !buffer.append(
                        SegmentRoot::repr_from_slice(intermediate_shard_block.own_segment_roots)
                            .as_flattened(),
                    )
                {
                    return Err(OwnedBeaconChainBodyError::BlockBodyIsTooLarge);
                }
                if !intermediate_shard_block.child_segment_roots.is_empty()
                    && !buffer.append(
                        SegmentRoot::repr_from_slice(intermediate_shard_block.child_segment_roots)
                            .as_flattened(),
                    )
                {
                    return Err(OwnedBeaconChainBodyError::BlockBodyIsTooLarge);
                }
            }
        }

        let true = buffer.append(PotCheckpoints::bytes_from_slice(pot_checkpoints).as_flattened())
        else {
            return Err(OwnedBeaconChainBodyError::BlockBodyIsTooLarge);
        };

        Ok(Self {
            buffer: buffer.into_shared(),
        })
    }

    /// Create owned block body from a reference
    #[inline]
    pub fn from_body(body: BeaconChainBody<'_>) -> Result<Self, OwnedBeaconChainBodyError> {
        Self::init(
            body.own_segment_roots,
            body.intermediate_shard_blocks.iter(),
            body.pot_checkpoints,
        )
    }

    /// Create owned body from a buffer
    #[inline]
    pub fn from_buffer(buffer: SharedAlignedBuffer) -> Result<Self, SharedAlignedBuffer> {
        let Some((_body, extra_bytes)) = BeaconChainBody::try_from_bytes(buffer.as_slice()) else {
            return Err(buffer);
        };
        if !extra_bytes.is_empty() {
            return Err(buffer);
        }

        Ok(Self { buffer })
    }

    /// Inner buffer with block body contents
    pub fn buffer(&self) -> &SharedAlignedBuffer {
        &self.buffer
    }

    /// Get [`BeaconChainBody`] out of [`OwnedBeaconChainBody`]
    pub fn body(&self) -> BeaconChainBody<'_> {
        BeaconChainBody::try_from_bytes_unchecked(self.buffer.as_slice())
            .expect("Constructor ensures validity; qed")
            .0
    }
}

/// Errors for [`OwnedIntermediateShardBody`]
#[derive(Debug, thiserror::Error)]
pub enum OwnedIntermediateShardBodyError {
    /// Too many own segment roots
    #[error("Too many own segment roots: {actual}")]
    TooManyOwnSegmentRoots {
        /// Actual number of own segment roots
        actual: usize,
    },
    /// Too many leaf shard blocks
    #[error("Too many leaf shard blocks: {actual}")]
    TooManyLeafShardBlocks {
        /// Actual number of leaf shard blocks
        actual: usize,
    },
    /// Too many leaf shard own segment roots
    #[error("Too many leaf shard own segment roots: {actual}")]
    TooManyLeafShardOwnSegmentRoots {
        /// Actual number of own segment roots
        actual: usize,
    },
    /// Block body is too large
    #[error("Block body is too large")]
    BlockBodyIsTooLarge,
    /// Too many transactions
    #[error("Too many transactions")]
    TooManyTransactions,
    /// Failed to add transaction
    #[error("Failed to add transaction: {error}")]
    FailedToAddTransaction {
        /// Inner error
        error: OwnedTransactionError,
    },
}

impl From<AddTransactionError> for OwnedIntermediateShardBodyError {
    fn from(value: AddTransactionError) -> Self {
        match value {
            AddTransactionError::BlockBodyIsTooLarge => {
                OwnedIntermediateShardBodyError::BlockBodyIsTooLarge
            }
            AddTransactionError::TooManyTransactions => {
                OwnedIntermediateShardBodyError::TooManyTransactions
            }
            AddTransactionError::FailedToAddTransaction { error } => {
                OwnedIntermediateShardBodyError::FailedToAddTransaction { error }
            }
        }
    }
}

/// An owned version of [`IntermediateShardBody`].
///
/// It is correctly aligned in memory and well suited for sending and receiving over the network
/// efficiently or storing in memory or on disk.
#[derive(Debug, Clone)]
pub struct OwnedIntermediateShardBody {
    buffer: SharedAlignedBuffer,
}

impl OwnedIntermediateShardBody {
    /// Initialize building of [`OwnedIntermediateShardBody`]
    pub fn init<'a, LSB>(
        own_segment_roots: &[SegmentRoot],
        leaf_shard_blocks: LSB,
    ) -> Result<OwnedIntermediateShardBlockBodyBuilder, OwnedIntermediateShardBodyError>
    where
        LSB: TrustedLen<Item = LeafShardBlockInfo<'a>> + Clone + 'a,
    {
        let num_own_segment_roots = own_segment_roots.len();
        let num_own_segment_roots = u8::try_from(num_own_segment_roots).map_err(|_error| {
            OwnedIntermediateShardBodyError::TooManyOwnSegmentRoots {
                actual: num_own_segment_roots,
            }
        })?;
        let num_blocks = leaf_shard_blocks.size_hint().0;
        let num_blocks = u8::try_from(num_blocks).map_err(|_error| {
            OwnedIntermediateShardBodyError::TooManyLeafShardBlocks { actual: num_blocks }
        })?;

        let mut buffer = OwnedAlignedBuffer::with_capacity(
            u8::SIZE
                + size_of_val(own_segment_roots) as u32
                // This is only an estimate to get in the ballpark where reallocation should not be
                // necessary if there are no transactions
                + u32::from(num_blocks) * OwnedLeafShardHeader::MAX_ALLOCATION * 2,
        );

        let true = buffer.append(&[num_own_segment_roots]) else {
            unreachable!("Fixed size data structures that are guaranteed to fit; qed");
        };
        let true = buffer.append(SegmentRoot::repr_from_slice(own_segment_roots).as_flattened())
        else {
            unreachable!("Checked size above; qed");
        };
        // TODO: Would be nice for `LeafShardBlocksInfo` to have API to write this by itself
        {
            let true = buffer.append(&num_blocks.to_le_bytes()) else {
                unreachable!("Fixed size data structures that are guaranteed to fit; qed");
            };
            let mut own_segments_roots_num_cursor = buffer.len() as usize;
            for _ in 0..num_blocks {
                let true = buffer.append(&[0]) else {
                    unreachable!("Checked size above; qed");
                };
            }
            let true = align_to_8_with_padding(&mut buffer) else {
                unreachable!("Checked size above; qed");
            };
            for leaf_shard_block in leaf_shard_blocks.clone() {
                if !leaf_shard_block.own_segment_roots.is_empty() {
                    let num_own_segment_roots = leaf_shard_block.own_segment_roots.len();
                    let num_own_segment_roots =
                        u8::try_from(num_own_segment_roots).map_err(|_error| {
                            OwnedIntermediateShardBodyError::TooManyLeafShardOwnSegmentRoots {
                                actual: num_own_segment_roots,
                            }
                        })?;
                    buffer.as_mut_slice()[own_segments_roots_num_cursor] = num_own_segment_roots;
                }
                own_segments_roots_num_cursor += 1;

                OwnedLeafShardHeader::from_parts_into(
                    leaf_shard_block.header.prefix,
                    leaf_shard_block.header.result,
                    leaf_shard_block.header.consensus_info,
                    leaf_shard_block.header.beacon_chain_info,
                    &mut buffer,
                );
                let true = align_to_8_with_padding(&mut buffer) else {
                    unreachable!("Checked size above; qed");
                };
                if let Some(segment_roots_proof) = leaf_shard_block.segment_roots_proof {
                    let true = buffer.append(segment_roots_proof) else {
                        unreachable!("Checked size above; qed");
                    };
                }
                if !leaf_shard_block.own_segment_roots.is_empty() {
                    let true = buffer.append(
                        SegmentRoot::repr_from_slice(leaf_shard_block.own_segment_roots)
                            .as_flattened(),
                    ) else {
                        unreachable!("Checked size above; qed");
                    };
                }
            }
        }
        let num_transactions_offset = buffer.len() as usize;
        let true = buffer.append(&0u32.to_le_bytes()) else {
            unreachable!("Checked size above; qed");
        };

        Ok(OwnedIntermediateShardBlockBodyBuilder {
            transaction_builder: TransactionBuilder::new(num_transactions_offset, buffer),
        })
    }

    /// Create owned block body from a reference
    #[inline]
    pub fn from_body(
        body: IntermediateShardBody<'_>,
    ) -> Result<Self, OwnedIntermediateShardBodyError> {
        let mut builder = Self::init(body.own_segment_roots, body.leaf_shard_blocks.iter())?;
        for transaction in body.transactions.iter() {
            builder.add_transaction(transaction)?;
        }

        Ok(builder.finish())
    }

    /// Create owned body from a buffer
    #[inline]
    pub fn from_buffer(buffer: SharedAlignedBuffer) -> Result<Self, SharedAlignedBuffer> {
        let Some((_body, extra_bytes)) = IntermediateShardBody::try_from_bytes(buffer.as_slice())
        else {
            return Err(buffer);
        };
        if !extra_bytes.is_empty() {
            return Err(buffer);
        }

        Ok(Self { buffer })
    }

    /// Inner buffer with block body contents
    pub fn buffer(&self) -> &SharedAlignedBuffer {
        &self.buffer
    }

    /// Get [`IntermediateShardBody`] out of [`OwnedIntermediateShardBody`]
    pub fn body(&self) -> IntermediateShardBody<'_> {
        IntermediateShardBody::try_from_bytes_unchecked(self.buffer.as_slice())
            .expect("Constructor ensures validity; qed")
            .0
    }
}

/// Builder for [`OwnedIntermediateShardBody`] that allows to add more transactions
#[derive(Debug, Clone)]
pub struct OwnedIntermediateShardBlockBodyBuilder {
    transaction_builder: TransactionBuilder,
}

impl OwnedIntermediateShardBlockBodyBuilder {
    /// Add transaction to the body
    #[inline(always)]
    pub fn add_transaction<T>(
        &mut self,
        transaction: T,
    ) -> Result<(), OwnedIntermediateShardBodyError>
    where
        T: WritableBodyTransaction,
    {
        self.transaction_builder.add_transaction(transaction)?;

        Ok(())
    }

    /// Finish building block body
    pub fn finish(self) -> OwnedIntermediateShardBody {
        OwnedIntermediateShardBody {
            buffer: self.transaction_builder.finish().into_shared(),
        }
    }
}

/// Errors for [`OwnedLeafShardBody`]
#[derive(Debug, thiserror::Error)]
pub enum OwnedLeafShardBodyError {
    /// Too many own segment roots
    #[error("Too many own segment roots: {actual}")]
    TooManyOwnSegmentRoots {
        /// Actual number of own segment roots
        actual: usize,
    },
    /// Block body is too large
    #[error("Block body is too large")]
    BlockBodyIsTooLarge,
    /// Too many transactions
    #[error("Too many transactions")]
    TooManyTransactions,
    /// Failed to add transaction
    #[error("Failed to add transaction: {error}")]
    FailedToAddTransaction {
        /// Inner error
        error: OwnedTransactionError,
    },
}

impl From<AddTransactionError> for OwnedLeafShardBodyError {
    fn from(value: AddTransactionError) -> Self {
        match value {
            AddTransactionError::BlockBodyIsTooLarge => {
                OwnedLeafShardBodyError::BlockBodyIsTooLarge
            }
            AddTransactionError::TooManyTransactions => {
                OwnedLeafShardBodyError::TooManyTransactions
            }
            AddTransactionError::FailedToAddTransaction { error } => {
                OwnedLeafShardBodyError::FailedToAddTransaction { error }
            }
        }
    }
}

/// An owned version of [`LeafShardBody`].
///
/// It is correctly aligned in memory and well suited for sending and receiving over the network
/// efficiently or storing in memory or on disk.
#[derive(Debug, Clone)]
pub struct OwnedLeafShardBody {
    buffer: SharedAlignedBuffer,
}

impl OwnedLeafShardBody {
    /// Initialize building of [`OwnedLeafShardBody`]
    pub fn init(
        own_segment_roots: &[SegmentRoot],
    ) -> Result<OwnedLeafShardBlockBodyBuilder, OwnedLeafShardBodyError> {
        let num_own_segment_roots = own_segment_roots.len();
        let num_own_segment_roots = u8::try_from(num_own_segment_roots).map_err(|_error| {
            OwnedLeafShardBodyError::TooManyOwnSegmentRoots {
                actual: num_own_segment_roots,
            }
        })?;

        let mut buffer =
            OwnedAlignedBuffer::with_capacity(u8::SIZE + size_of_val(own_segment_roots) as u32);

        let true = buffer.append(&[num_own_segment_roots]) else {
            unreachable!("Fixed size data structures that are guaranteed to fit; qed");
        };
        let true = buffer.append(SegmentRoot::repr_from_slice(own_segment_roots).as_flattened())
        else {
            unreachable!("Checked size above; qed");
        };

        let num_transactions_offset = buffer.len() as usize;
        let true = buffer.append(&0u32.to_le_bytes()) else {
            unreachable!("Checked size above; qed");
        };

        Ok(OwnedLeafShardBlockBodyBuilder {
            transaction_builder: TransactionBuilder::new(num_transactions_offset, buffer),
        })
    }

    /// Create owned block body from a reference
    #[inline]
    pub fn from_body(body: LeafShardBody<'_>) -> Result<Self, OwnedLeafShardBodyError> {
        let mut builder = Self::init(body.own_segment_roots)?;
        for transaction in body.transactions.iter() {
            builder.add_transaction(transaction)?;
        }

        Ok(builder.finish())
    }

    /// Create owned body from a buffer
    #[inline]
    pub fn from_buffer(buffer: SharedAlignedBuffer) -> Result<Self, SharedAlignedBuffer> {
        let Some((_body, extra_bytes)) = LeafShardBody::try_from_bytes(buffer.as_slice()) else {
            return Err(buffer);
        };
        if !extra_bytes.is_empty() {
            return Err(buffer);
        }

        Ok(Self { buffer })
    }

    /// Inner buffer with block body contents
    pub fn buffer(&self) -> &SharedAlignedBuffer {
        &self.buffer
    }

    /// Get [`LeafShardBody`] out of [`OwnedLeafShardBody`]
    pub fn body(&self) -> LeafShardBody<'_> {
        LeafShardBody::try_from_bytes_unchecked(self.buffer.as_slice())
            .expect("Constructor ensures validity; qed")
            .0
    }
}

/// Builder for [`OwnedLeafShardBody`] that allows to add more transactions
#[derive(Debug, Clone)]
pub struct OwnedLeafShardBlockBodyBuilder {
    transaction_builder: TransactionBuilder,
}

impl OwnedLeafShardBlockBodyBuilder {
    /// Add transaction to the body
    #[inline(always)]
    pub fn add_transaction<T>(&mut self, transaction: T) -> Result<(), OwnedLeafShardBodyError>
    where
        T: WritableBodyTransaction,
    {
        self.transaction_builder.add_transaction(transaction)?;

        Ok(())
    }

    /// Finish building block body
    pub fn finish(self) -> OwnedLeafShardBody {
        OwnedLeafShardBody {
            buffer: self.transaction_builder.finish().into_shared(),
        }
    }
}

/// Errors for [`OwnedBlockBody`]
#[derive(Debug, thiserror::Error)]
pub enum OwnedBlockBodyError {
    /// Beacon chain block body error
    #[error("Beacon chain block body error: {0}")]
    BeaconChain(#[from] OwnedBeaconChainBodyError),
    /// Intermediate shard block body error
    #[error("Intermediate shard block body error: {0}")]
    IntermediateShard(#[from] OwnedIntermediateShardBodyError),
    /// Leaf shard block body error
    #[error("Leaf shard block body error: {0}")]
    LeafShard(#[from] OwnedLeafShardBodyError),
}

/// An owned version of [`BlockBody`].
///
/// It is correctly aligned in memory and well suited for sending and receiving over the network
/// efficiently or storing in memory or on disk.
#[derive(Debug, Clone, From)]
pub enum OwnedBlockBody {
    /// Block body corresponds to the beacon chain
    BeaconChain(OwnedBeaconChainBody),
    /// Block body corresponds to an intermediate shard
    IntermediateShard(OwnedIntermediateShardBody),
    /// Block body corresponds to a leaf shard
    LeafShard(OwnedLeafShardBody),
}

impl OwnedBlockBody {
    /// Create owned block body from a reference
    #[inline]
    pub fn from_body(body: BlockBody<'_>) -> Result<Self, OwnedBlockBodyError> {
        Ok(match body {
            BlockBody::BeaconChain(body) => {
                Self::BeaconChain(OwnedBeaconChainBody::from_body(body)?)
            }
            BlockBody::IntermediateShard(body) => {
                Self::IntermediateShard(OwnedIntermediateShardBody::from_body(body)?)
            }
            BlockBody::LeafShard(body) => Self::LeafShard(OwnedLeafShardBody::from_body(body)?),
        })
    }

    /// Create owned body from a buffer
    #[inline]
    pub fn from_buffer(
        buffer: SharedAlignedBuffer,
        shard_kind: ShardKind,
    ) -> Result<Self, SharedAlignedBuffer> {
        let Some((_body, extra_bytes)) = BlockBody::try_from_bytes(buffer.as_slice(), shard_kind)
        else {
            return Err(buffer);
        };
        if !extra_bytes.is_empty() {
            return Err(buffer);
        }

        Ok(match shard_kind {
            ShardKind::BeaconChain => Self::BeaconChain(OwnedBeaconChainBody { buffer }),
            ShardKind::IntermediateShard => {
                Self::IntermediateShard(OwnedIntermediateShardBody { buffer })
            }
            ShardKind::LeafShard => Self::LeafShard(OwnedLeafShardBody { buffer }),
            ShardKind::Phantom | ShardKind::Invalid => {
                // Blocks for such shards do not exist
                return Err(buffer);
            }
        })
    }

    /// Inner buffer block body contents
    pub fn buffer(&self) -> &SharedAlignedBuffer {
        match self {
            Self::BeaconChain(owned_body) => owned_body.buffer(),
            Self::IntermediateShard(owned_body) => owned_body.buffer(),
            Self::LeafShard(owned_body) => owned_body.buffer(),
        }
    }

    /// Get [`BlockBody`] out of [`OwnedBlockBody`]
    pub fn body(&self) -> BlockBody<'_> {
        match self {
            Self::BeaconChain(owned_body) => BlockBody::BeaconChain(owned_body.body()),
            Self::IntermediateShard(owned_body) => BlockBody::IntermediateShard(owned_body.body()),
            Self::LeafShard(owned_body) => BlockBody::LeafShard(owned_body.body()),
        }
    }
}

/// Aligns buffer to 8 bytes by adding necessary padding zero bytes.
///
/// Returns `false` if buffer becomes too long.
#[inline(always)]
#[must_use]
fn align_to_8_with_padding(buffer: &mut OwnedAlignedBuffer) -> bool {
    let alignment = align_of::<u64>();
    // Optimized version of the following due to alignment being a power of 2:
    // let unaligned_by = self.payload.as_ptr().addr() % alignment;
    let unaligned_by = buffer.as_ptr().addr() & (alignment - 1);
    if unaligned_by > 0 {
        // SAFETY: Subtracted value is always smaller than alignment
        let padding_bytes = unsafe { alignment.unchecked_sub(unaligned_by) };

        if !buffer.append(&0u64.to_le_bytes()[..padding_bytes]) {
            return false;
        }
    }

    true
}

/// Aligns buffer to 16 bytes by adding necessary padding zero bytes.
///
/// Returns `false` if buffer becomes too long.
#[inline(always)]
#[must_use]
fn align_to_16_bytes_with_padding(buffer: &mut OwnedAlignedBuffer) -> bool {
    let alignment = align_of::<u128>();
    // Optimized version of the following due to alignment being a power of 2:
    // let unaligned_by = self.payload.as_ptr().addr() % alignment;
    let unaligned_by = buffer.as_ptr().addr() & (alignment - 1);
    if unaligned_by > 0 {
        // SAFETY: Subtracted value is always smaller than alignment
        let padding_bytes = unsafe { alignment.unchecked_sub(unaligned_by) };

        if !buffer.append(&0u128.to_le_bytes()[..padding_bytes]) {
            return false;
        }
    }

    true
}
