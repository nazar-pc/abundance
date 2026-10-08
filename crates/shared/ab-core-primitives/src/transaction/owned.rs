//! Data structures related to the owned version of [`Transaction`]

#[cfg(test)]
mod tests;

use crate::transaction::{
    SerializedTransactionLengths, Transaction, TransactionHeader, TransactionSlot,
};
use ab_aligned_buffer::{OwnedAlignedBuffer, SharedAlignedBuffer};
use ab_io_type::trivial_type::TrivialType;
use core::slice;

/// Errors for [`OwnedTransaction`]
#[derive(Debug, thiserror::Error)]
pub enum OwnedTransactionError {
    /// Too many read slots
    #[error("Too many read slots")]
    TooManyReadSlots,
    /// Too many write slots
    #[error("Too many write slots")]
    TooManyWriteSlots,
    /// Payload too large
    #[error("The payload is too large")]
    PayloadTooLarge,
    /// Payload is not a multiple of `u128`
    #[error("The payload is not a multiple of `u128`")]
    PayloadIsNotMultipleOfU128,
    /// Seal too large
    #[error("The leal too large")]
    SealTooLarge,
    /// Transaction too large
    #[error("The transaction is too large")]
    TransactionTooLarge,
    /// Not enough bytes
    #[error("Not enough bytes")]
    NotEnoughBytes,
    /// Invalid padding
    #[error("Invalid padding")]
    InvalidPadding,
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
    /// Create owned transaction from its parts
    pub fn from_parts(
        header: &TransactionHeader,
        read_slots: &[TransactionSlot],
        write_slots: &[TransactionSlot],
        payload: &[u128],
        seal: &[u8],
    ) -> Result<Self, OwnedTransactionError> {
        let mut buffer = OwnedAlignedBuffer::with_capacity(
            (TransactionHeader::SIZE + SerializedTransactionLengths::SIZE)
                .saturating_add(size_of_val(read_slots) as u32)
                .saturating_add(size_of_val(write_slots) as u32)
                .saturating_add(size_of_val(payload) as u32)
                .saturating_add(size_of_val(seal) as u32),
        );

        Self::from_parts_into(header, read_slots, write_slots, payload, seal, &mut buffer)?;

        Ok(Self {
            buffer: buffer.into_shared(),
        })
    }

    /// Create owned transaction from its parts and write it into provided buffer
    pub fn from_parts_into(
        header: &TransactionHeader,
        read_slots: &[TransactionSlot],
        write_slots: &[TransactionSlot],
        payload: &[u128],
        seal: &[u8],
        buffer: &mut OwnedAlignedBuffer,
    ) -> Result<(), OwnedTransactionError> {
        const {
            // Writing `OwnedTransactionLengths` after `TransactionHeader` must be aligned
            assert!(
                size_of::<TransactionHeader>()
                    .is_multiple_of(align_of::<SerializedTransactionLengths>())
            );
        }

        let transaction_lengths = SerializedTransactionLengths {
            read_slots: read_slots
                .len()
                .try_into()
                .map_err(|_error| OwnedTransactionError::TooManyReadSlots)?,
            write_slots: write_slots
                .len()
                .try_into()
                .map_err(|_error| OwnedTransactionError::TooManyWriteSlots)?,
            payload: size_of_val(payload)
                .try_into()
                .map_err(|_error| OwnedTransactionError::PayloadTooLarge)?,
            seal: seal
                .len()
                .try_into()
                .map_err(|_error| OwnedTransactionError::SealTooLarge)?,
            padding: [0; _],
        };

        let true = buffer.append(header.as_bytes()) else {
            unreachable!("Always fits into `u32`");
        };
        let true = buffer.append(transaction_lengths.as_bytes()) else {
            unreachable!("Always fits into `u32`");
        };

        const {
            // Writing `TransactionSlot` after `OwnedTransactionLengths` and `TransactionHeader`
            // must be aligned
            assert!(
                (size_of::<TransactionHeader>() + size_of::<SerializedTransactionLengths>())
                    .is_multiple_of(align_of::<TransactionSlot>())
            );
        }
        if transaction_lengths.read_slots > 0 {
            // SAFETY: `TransactionSlot` implements `TrivialType` and is safe to copy as bytes
            if !buffer.append(unsafe {
                slice::from_raw_parts(read_slots.as_ptr().cast::<u8>(), size_of_val(read_slots))
            }) {
                return Err(OwnedTransactionError::TransactionTooLarge);
            }
        }
        if transaction_lengths.write_slots > 0 {
            // SAFETY: `TransactionSlot` implements `TrivialType` and is safe to copy as bytes
            if !buffer.append(unsafe {
                slice::from_raw_parts(write_slots.as_ptr().cast::<u8>(), size_of_val(write_slots))
            }) {
                return Err(OwnedTransactionError::TransactionTooLarge);
            }
        }

        const {
            // Writing after `OwnedTransactionLengths`, `TransactionHeader` and (optionally)
            // `TransactionSlot` must be aligned to `u128`
            assert!(
                (size_of::<TransactionHeader>() + size_of::<SerializedTransactionLengths>())
                    .is_multiple_of(align_of::<u128>())
            );
            assert!(
                (size_of::<TransactionHeader>()
                    + size_of::<SerializedTransactionLengths>()
                    + size_of::<TransactionSlot>())
                .is_multiple_of(align_of::<u128>())
            );
        }
        if transaction_lengths.payload > 0 {
            if !transaction_lengths.payload.is_multiple_of(u128::SIZE) {
                return Err(OwnedTransactionError::PayloadIsNotMultipleOfU128);
            }

            // SAFETY: `u128` is safe to copy as bytes
            if !buffer.append(unsafe {
                slice::from_raw_parts(payload.as_ptr().cast::<u8>(), size_of_val(payload))
            }) {
                return Err(OwnedTransactionError::TransactionTooLarge);
            }
        }

        const {
            // Writing after `OwnedTransactionLengths`, `TransactionHeader` and (optionally)
            // `TransactionSlot` must be aligned to `u128`
            assert!(
                (size_of::<TransactionHeader>() + size_of::<SerializedTransactionLengths>())
                    .is_multiple_of(align_of::<u128>())
            );
            assert!(
                (size_of::<TransactionHeader>()
                    + size_of::<SerializedTransactionLengths>()
                    + size_of::<TransactionSlot>())
                .is_multiple_of(align_of::<u128>())
            );
        }
        if transaction_lengths.seal > 0 && !buffer.append(seal) {
            return Err(OwnedTransactionError::TransactionTooLarge);
        }

        Ok(())
    }

    /// Create owned transaction from a reference
    #[inline(always)]
    pub fn from_transaction(transaction: Transaction<'_>) -> Result<Self, OwnedTransactionError> {
        Self::from_parts(
            transaction.header,
            transaction.read_slots,
            transaction.write_slots,
            transaction.payload,
            transaction.seal,
        )
    }

    /// Create owned transaction from a buffer.
    ///
    /// Returns [`OwnedTransactionError::NotEnoughBytes`] if the buffer is shorter than the
    /// transaction it encodes, and [`OwnedTransactionError::UnexpectedNumberOfBytes`] if it is
    /// longer.
    pub fn from_buffer(buffer: SharedAlignedBuffer) -> Result<Self, OwnedTransactionError> {
        if buffer.len() < TransactionHeader::SIZE + SerializedTransactionLengths::SIZE {
            return Err(OwnedTransactionError::NotEnoughBytes);
        }

        // SAFETY: Checked above that there are enough bytes, alignment is guaranteed by
        // `SharedAlignedBuffer`
        let lengths = unsafe {
            buffer
                .as_ptr()
                .byte_add(size_of::<TransactionHeader>())
                .cast::<SerializedTransactionLengths>()
                .read()
        };
        let SerializedTransactionLengths {
            read_slots: _,
            write_slots: _,
            payload,
            seal: _,
            padding,
        } = lengths;

        // `Transaction::try_from_bytes()` below checks these too, they are checked here first to
        // return a more specific error
        if padding != [0; _] {
            return Err(OwnedTransactionError::InvalidPadding);
        }

        if !payload.is_multiple_of(u128::SIZE) {
            return Err(OwnedTransactionError::PayloadIsNotMultipleOfU128);
        }

        let Some((transaction, remainder)) = Transaction::try_from_bytes(buffer.as_slice()) else {
            // Alignment is guaranteed by `SharedAlignedBuffer`, padding and payload length are
            // checked above, so insufficient size is the only remaining reason
            return Err(OwnedTransactionError::NotEnoughBytes);
        };

        if !remainder.is_empty() {
            return Err(OwnedTransactionError::UnexpectedNumberOfBytes {
                actual: buffer.len(),
                expected: u32::try_from(transaction.encoded_size())
                    .expect("Smaller than the buffer, whose length is `u32`; qed"),
            });
        }

        Ok(Self { buffer })
    }

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
