//! Data structures related to the owned version of [`Transaction`]

mod builder_buffer;

use crate::transaction::owned::builder_buffer::BuilderBuffer;
use crate::transaction::{
    SerializedTransactionLengths, Transaction, TransactionHeader, TransactionSlot,
};
use ab_aligned_buffer::SharedAlignedBuffer;
use ab_io_type::trivial_type::TrivialType;
use core::slice;

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
            < size_of::<TransactionHeader>() + size_of::<SerializedTransactionLengths>()
        {
            return Err(OwnedTransactionError::NotEnoughBytes);
        }

        // SAFETY: Checked above that there are enough bytes and they are correctly aligned
        let lengths = unsafe {
            buffer
                .as_ptr()
                .add(size_of::<TransactionHeader>())
                .cast::<SerializedTransactionLengths>()
                .read()
        };
        let SerializedTransactionLengths {
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
            + size_of::<SerializedTransactionLengths>() as u32)
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
        // SAFETY: Size and alignment checked in constructor
        unsafe { Transaction::from_bytes_unchecked(self.buffer.as_slice()) }
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
