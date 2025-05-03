//! Data structures related to the owned version of [`Transaction`]

mod builder_buffer;

use crate::transaction::owned::builder_buffer::BuilderBuffer;
use crate::transaction::{Transaction, TransactionHeader, TransactionSlot};
use ab_aligned_buffer::SharedAlignedBuffer;
use ab_io_type::trivial_type::TrivialType;
use core::slice;

/// Lengths of various components inside [`OwnedTransaction`]
#[derive(Debug, Default, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct OwnedTransactionLengths {
    /// Number of read-only slots
    pub read_slots: u16,
    /// Number of read-write slots
    pub write_slots: u16,
    /// Payload length
    pub payload: u32,
    /// Seal length
    pub seal: u32,
    /// Not used and must be set to `0`
    pub padding: [u8; 4],
}

/// Errors for [`OwnedTransaction`]
#[derive(Debug, thiserror::Error)]
pub enum OwnedTransactionError {
    /// Not enough bytes
    #[error("Not enough bytes")]
    NotEnoughBytes,
    /// Invalid padding
    #[error("Invalid padding")]
    InvalidPadding,
    /// Payload is not a multiple of `u128`
    #[error("Payload is not a multiple of `u128`")]
    PayloadIsNotMultipleOfU128,
    /// Expected number of bytes
    #[error("Expected number of bytes: {actual} != {expected}")]
    UnexpectedNumberOfBytes {
        /// Actual number of bytes
        actual: u32,
        /// Expected number of bytes
        expected: u32,
    },
}

/// An owned version of [`Transaction`].
///
/// It is correctly aligned in memory and well suited for sending and receiving over the network
/// efficiently or storing in memory or on disk.
///
/// The internal layout of the owned transaction is following data structures concatenated as bytes
/// (they are carefully picked to ensure alignment):
/// * [`TransactionHeader`]
/// * [`OwnedTransactionLengths`] (with values set to correspond to below contents
/// * All read [`TransactionSlot`]
/// * All write [`TransactionSlot`]
/// * Payload as `u128`s
/// * Seal as `u8`s
#[derive(Debug, Clone)]
pub struct OwnedTransaction {
    buffer: SharedAlignedBuffer,
}

impl OwnedTransaction {
    /// Create transaction builder with provided transaction header
    pub fn build(header: &TransactionHeader) -> OwnedTransactionBuilder {
        OwnedTransactionBuilder {
            buffer: BuilderBuffer::new(header),
        }
    }

    /// Create an owned transaction from a buffer
    pub fn from_buffer(buffer: SharedAlignedBuffer) -> Result<Self, OwnedTransactionError> {
        if (buffer.len() as usize)
            < size_of::<TransactionHeader>() + size_of::<OwnedTransactionLengths>()
        {
            return Err(OwnedTransactionError::NotEnoughBytes);
        }

        // SAFETY: Checked above that there are enough bytes and they are correctly aligned
        let lengths = unsafe {
            buffer
                .as_ptr()
                .add(size_of::<TransactionHeader>())
                .cast::<OwnedTransactionLengths>()
                .read()
        };
        let OwnedTransactionLengths {
            read_slots,
            write_slots,
            payload,
            seal,
            padding,
        } = lengths;

        if padding != [0; _] {
            return Err(OwnedTransactionError::InvalidPadding);
        }

        if payload % u128::SIZE != 0 {
            return Err(OwnedTransactionError::PayloadIsNotMultipleOfU128);
        }

        let expected = (size_of::<TransactionHeader>() as u32
            + size_of::<OwnedTransactionLengths>() as u32)
            .saturating_add(u32::from(read_slots))
            .saturating_add(u32::from(write_slots))
            .saturating_add(payload)
            .saturating_add(seal);

        if buffer.len() != expected {
            return Err(OwnedTransactionError::UnexpectedNumberOfBytes {
                actual: buffer.len(),
                expected,
            });
        }

        Ok(Self { buffer })
    }

    // TODO: Implement
    // pub fn from_transaction(transaction: Transaction<'_>) -> Result<Self, OwnedTransactionError> {
    //     todo!()
    // }

    /// Inner buffer with owned transaction contents
    pub fn buffer(&self) -> &SharedAlignedBuffer {
        &self.buffer
    }

    /// Get [`Transaction`] out of owned transaction
    pub fn transaction(&self) -> Transaction<'_> {
        // SAFETY: All constructors ensure there are enough bytes and they are correctly aligned
        let lengths = unsafe {
            self.buffer
                .as_ptr()
                .add(size_of::<TransactionHeader>())
                .cast::<OwnedTransactionLengths>()
                .read()
        };
        let OwnedTransactionLengths {
            read_slots,
            write_slots,
            payload,
            seal,
            padding: _,
        } = lengths;

        Transaction {
            // SAFETY: Any bytes are valid for `TransactionHeader` and all constructors ensure there
            // are enough bytes for header in the buffer
            header: unsafe {
                self.buffer
                    .as_ptr()
                    .cast::<TransactionHeader>()
                    .as_ref_unchecked()
            },
            // SAFETY: Any bytes are valid for `TransactionSlot` and all constructors ensure there
            // are enough bytes for read slots in the buffer
            read_slots: unsafe {
                slice::from_raw_parts(
                    self.buffer
                        .as_ptr()
                        .add(size_of::<TransactionHeader>())
                        .add(size_of::<OwnedTransactionLengths>())
                        .cast::<TransactionSlot>(),
                    usize::from(read_slots),
                )
            },
            // SAFETY: Any bytes are valid for `TransactionSlot` and all constructors ensure there
            // are enough bytes for write slots in the buffer
            write_slots: unsafe {
                slice::from_raw_parts(
                    self.buffer
                        .as_ptr()
                        .add(size_of::<TransactionHeader>())
                        .add(size_of::<OwnedTransactionLengths>())
                        .cast::<TransactionSlot>()
                        .add(usize::from(read_slots)),
                    usize::from(write_slots),
                )
            },
            // SAFETY: Any bytes are valid for `payload` and all constructors ensure there are
            // enough bytes for payload in the buffer
            payload: unsafe {
                slice::from_raw_parts(
                    self.buffer
                        .as_ptr()
                        .add(size_of::<TransactionHeader>())
                        .add(size_of::<OwnedTransactionLengths>())
                        .add(
                            size_of::<TransactionSlot>()
                                * (usize::from(read_slots) + usize::from(write_slots)),
                        )
                        .cast::<u128>(),
                    payload as usize,
                )
            },
            // SAFETY: Any bytes are valid for `seal` and all constructors ensure there are
            // enough bytes for seal in the buffer
            seal: unsafe {
                slice::from_raw_parts(
                    self.buffer
                        .as_ptr()
                        .add(size_of::<TransactionHeader>())
                        .add(size_of::<OwnedTransactionLengths>())
                        .add(
                            size_of::<TransactionSlot>()
                                * (usize::from(read_slots) + usize::from(write_slots))
                                + payload as usize,
                        ),
                    seal as usize,
                )
            },
        }
    }
}

/// Errors for [`OwnedTransactionBuilder`]
#[derive(Debug, thiserror::Error)]
pub enum OwnedTransactionBuilderError {
    /// Too many read slots
    #[error("Too many read slots")]
    TooManyReadSlots,
    /// Too many write slots
    #[error("Too many write slots")]
    TooManyWriteSlots,
    /// Payload too large
    #[error("Payload too large")]
    PayloadTooLarge,
    /// Payload is not a multiple of `u128`
    #[error("Payload is not a multiple of `u128`")]
    PayloadIsNotMultipleOfU128,
    /// Seal too large
    #[error("Seal too large")]
    SealTooLarge,
    /// Transaction too large
    #[error("Transaction too large")]
    TransactionTooLarge,
}

/// Builder for [`OwnedTransaction`]
#[derive(Debug, Clone)]
pub struct OwnedTransactionBuilder {
    buffer: BuilderBuffer,
}

impl OwnedTransactionBuilder {
    /// Add read-only slot to the transaction
    pub fn with_read_slot(
        mut self,
        slot: &TransactionSlot,
    ) -> Result<OwnedTransactionBuilder, OwnedTransactionBuilderError> {
        self.buffer.append_read_slots(slice::from_ref(slot))?;
        Ok(OwnedTransactionBuilder {
            buffer: self.buffer,
        })
    }

    /// Add many read-only slots to the transaction
    pub fn with_read_slots(
        mut self,
        slots: &[TransactionSlot],
    ) -> Result<OwnedTransactionBuilder, OwnedTransactionBuilderError> {
        self.buffer.append_read_slots(slots)?;
        Ok(OwnedTransactionBuilder {
            buffer: self.buffer,
        })
    }

    /// Add read-write slot to the transaction
    pub fn with_write_slot(
        mut self,
        slot: &TransactionSlot,
    ) -> Result<OwnedTransactionBuilderWithWriteSlot, OwnedTransactionBuilderError> {
        self.buffer.append_write_slots(slice::from_ref(slot))?;
        Ok(OwnedTransactionBuilderWithWriteSlot {
            buffer: self.buffer,
        })
    }

    /// Add many read-write slots to the transaction
    pub fn with_write_slots(
        mut self,
        slots: &[TransactionSlot],
    ) -> Result<OwnedTransactionBuilderWithWriteSlot, OwnedTransactionBuilderError> {
        self.buffer.append_write_slots(slots)?;
        Ok(OwnedTransactionBuilderWithWriteSlot {
            buffer: self.buffer,
        })
    }

    /// Add transaction payload
    pub fn with_payload(
        mut self,
        payload: &[u8],
    ) -> Result<OwnedTransactionBuilderWithPayload, OwnedTransactionBuilderError> {
        self.buffer.append_payload(payload)?;
        Ok(OwnedTransactionBuilderWithPayload {
            buffer: self.buffer,
        })
    }

    /// Add transaction seal
    pub fn with_seal(
        mut self,
        seal: &[u8],
    ) -> Result<OwnedTransaction, OwnedTransactionBuilderError> {
        self.buffer.append_seal(seal)?;
        self.finish()
    }

    /// Get owned transaction
    pub fn finish(self) -> Result<OwnedTransaction, OwnedTransactionBuilderError> {
        let buffer = self.buffer.finish()?.into_shared();
        Ok(OwnedTransaction { buffer })
    }
}

/// Builder for [`OwnedTransaction`] with at least one read-write slot
#[derive(Debug, Clone)]
pub struct OwnedTransactionBuilderWithWriteSlot {
    buffer: BuilderBuffer,
}

impl OwnedTransactionBuilderWithWriteSlot {
    /// Add read-write slot to the transaction
    pub fn with_write_slot(
        mut self,
        slot: &TransactionSlot,
    ) -> Result<Self, OwnedTransactionBuilderError> {
        self.buffer.append_write_slots(slice::from_ref(slot))?;
        Ok(Self {
            buffer: self.buffer,
        })
    }

    /// Add many read-write slots to the transaction
    pub fn with_write_slots(
        mut self,
        slots: &[TransactionSlot],
    ) -> Result<Self, OwnedTransactionBuilderError> {
        self.buffer.append_write_slots(slots)?;
        Ok(Self {
            buffer: self.buffer,
        })
    }

    /// Add transaction payload
    pub fn with_payload(
        mut self,
        payload: &[u8],
    ) -> Result<OwnedTransactionBuilderWithPayload, OwnedTransactionBuilderError> {
        self.buffer.append_payload(payload)?;
        Ok(OwnedTransactionBuilderWithPayload {
            buffer: self.buffer,
        })
    }

    /// Add transaction seal
    pub fn with_seal(
        mut self,
        seal: &[u8],
    ) -> Result<OwnedTransaction, OwnedTransactionBuilderError> {
        self.buffer.append_seal(seal)?;
        self.finish()
    }

    /// Get owned transaction
    pub fn finish(self) -> Result<OwnedTransaction, OwnedTransactionBuilderError> {
        let buffer = self.buffer.finish()?.into_shared();
        Ok(OwnedTransaction { buffer })
    }
}

/// Builder for [`OwnedTransaction`] with payload
#[derive(Debug, Clone)]
pub struct OwnedTransactionBuilderWithPayload {
    buffer: BuilderBuffer,
}

impl OwnedTransactionBuilderWithPayload {
    /// Add transaction payload
    pub fn with_payload(mut self, payload: &[u8]) -> Result<Self, OwnedTransactionBuilderError> {
        self.buffer.append_payload(payload)?;
        Ok(Self {
            buffer: self.buffer,
        })
    }

    /// Add transaction seal
    pub fn with_seal(
        mut self,
        seal: &[u8],
    ) -> Result<OwnedTransaction, OwnedTransactionBuilderError> {
        self.buffer.append_seal(seal)?;
        self.finish()
    }

    /// Get owned transaction
    pub fn finish(self) -> Result<OwnedTransaction, OwnedTransactionBuilderError> {
        let buffer = self.buffer.finish()?.into_shared();
        Ok(OwnedTransaction { buffer })
    }
}
