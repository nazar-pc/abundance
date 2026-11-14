//! Data structures related to the owned version of [`Transaction`]

#[cfg(test)]
mod tests;

use crate::transaction::{
    SerializedTransactionLengths, Transaction, TransactionHash, TransactionHeader, TransactionSlot,
};
use ab_aligned_buffer::alignment::Align16;
use ab_aligned_buffer::{OwnedAlignedBuffer, SharedAlignedBuffer};
use ab_io_type::alignment::Alignment;
use ab_io_type::trivial_type::TrivialType;
use triomphe::Arc;
use yoke::{Yoke, Yokeable};

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
    /// Seal too large
    #[error("The seal is too large")]
    SealTooLarge,
    /// Transaction too large
    #[error("The transaction is too large")]
    TransactionTooLarge,
    /// Invalid transaction, see [`Transaction::try_from_bytes()`]
    #[error("Invalid transaction")]
    InvalidTransaction,
    /// Expected number of bytes
    #[error("Expected number of bytes: {actual} != {expected}")]
    UnexpectedNumberOfBytes {
        /// Actual number of bytes
        actual: u32,
        /// Expected number of bytes
        expected: u32,
    },
}

// TODO: Derive `Yokeable` for `Transaction` directly once `syn` can parse `mut` restrictions of its
//  fields: https://github.com/dtolnay/syn/issues/1987
#[derive(Debug, Yokeable)]
struct YokeableTransaction<'a>(Transaction<'a>);

/// An owned version of [`Transaction`].
///
/// It is correctly aligned in memory and well suited for sending and receiving over the network
/// efficiently or storing in memory or on disk.
#[derive(Debug, Clone)]
pub struct OwnedTransaction {
    inner: Arc<Yoke<YokeableTransaction<'static>, SharedAlignedBuffer>>,
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
        // The size of the transaction is known upfront, so a transaction that doesn't fit into a
        // buffer is rejected before anything is allocated or copied
        #[cfg_attr(
            target_pointer_width = "64",
            expect(
                clippy::absurd_extreme_comparisons,
                reason = "`MAX_CAPACITY` is `u32::MAX` where `usize` is 64-bit"
            )
        )]
        let size = (size_of::<TransactionHeader>() + size_of::<SerializedTransactionLengths>())
            .checked_add(size_of_val(read_slots))
            .and_then(|size| size.checked_add(size_of_val(write_slots)))
            .and_then(|size| size.checked_add(size_of_val(payload)))
            .and_then(|size| size.checked_add(size_of_val(seal)))
            .and_then(|size| u32::try_from(size).ok())
            .filter(|&size| size <= OwnedAlignedBuffer::<Align16>::MAX_CAPACITY)
            .ok_or(OwnedTransactionError::TransactionTooLarge)?;
        let mut buffer = OwnedAlignedBuffer::with_capacity(size);

        Self::from_parts_into(header, read_slots, write_slots, payload, seal, &mut buffer)?;

        let transaction = Self::from_buffer(buffer.into_shared())
            .expect("`from_parts_into()` wrote a valid transaction into an empty buffer; qed");

        Ok(transaction)
    }

    /// Create owned transaction from its parts and write it into provided buffer.
    ///
    /// On error, part of the transaction might have already been appended to the buffer.
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

        if !buffer.append(header.as_bytes()) {
            return Err(OwnedTransactionError::TransactionTooLarge);
        }
        if !buffer.append(transaction_lengths.as_bytes()) {
            return Err(OwnedTransactionError::TransactionTooLarge);
        }

        const {
            // Writing `TransactionSlot` after `OwnedTransactionLengths` and `TransactionHeader`
            // must be aligned
            assert!(
                (size_of::<TransactionHeader>() + size_of::<SerializedTransactionLengths>())
                    .is_multiple_of(align_of::<TransactionSlot>())
            );
        }
        if transaction_lengths.read_slots > 0
            && !buffer.append(TransactionSlot::slice_as_bytes(read_slots))
        {
            return Err(OwnedTransactionError::TransactionTooLarge);
        }
        if transaction_lengths.write_slots > 0
            && !buffer.append(TransactionSlot::slice_as_bytes(write_slots))
        {
            return Err(OwnedTransactionError::TransactionTooLarge);
        }

        const {
            // Writing after `OwnedTransactionLengths`, `TransactionHeader` and (optionally)
            // `TransactionSlot` must be aligned to `Alignment::MAX`
            assert!(
                (size_of::<TransactionHeader>() + size_of::<SerializedTransactionLengths>())
                    .is_multiple_of(usize::from(Alignment::MAX.get()))
            );
            assert!(
                (size_of::<TransactionHeader>()
                    + size_of::<SerializedTransactionLengths>()
                    + size_of::<TransactionSlot>())
                .is_multiple_of(usize::from(Alignment::MAX.get()))
            );
        }
        if transaction_lengths.payload > 0 && !buffer.append(u128::slice_as_bytes(payload)) {
            return Err(OwnedTransactionError::TransactionTooLarge);
        }

        if transaction_lengths.seal > 0 && !buffer.append(seal) {
            return Err(OwnedTransactionError::TransactionTooLarge);
        }

        Ok(())
    }

    /// Create owned transaction from a reference
    #[inline]
    pub fn from_transaction(transaction: Transaction<'_>) -> Result<Self, OwnedTransactionError> {
        // Appending to an empty buffer allocates exactly the necessary capacity
        let mut buffer = OwnedAlignedBuffer::with_capacity(0);
        if !buffer.append(transaction.bytes) {
            return Err(OwnedTransactionError::TransactionTooLarge);
        }

        let transaction = Self::from_buffer(buffer.into_shared()).expect(
            "The same bytes were already parsed with all checks, which only depend on the address \
            being 16-byte aligned, and owned buffers are 16-byte aligned; qed",
        );

        Ok(transaction)
    }

    /// Create owned transaction from a buffer.
    ///
    /// Returns [`OwnedTransactionError::InvalidTransaction`] if the buffer doesn't start with a
    /// valid transaction, and [`OwnedTransactionError::UnexpectedNumberOfBytes`] if it is longer
    /// than the transaction it encodes.
    pub fn from_buffer(buffer: SharedAlignedBuffer) -> Result<Self, OwnedTransactionError> {
        let buffer_len = buffer.len();
        let inner = Yoke::try_attach_to_cart(buffer, |bytes| {
            // Alignment is guaranteed by `SharedAlignedBuffer`, so this only fails due to the
            // contents
            let (transaction, remainder) = Transaction::try_from_bytes(bytes)
                .ok_or(OwnedTransactionError::InvalidTransaction)?;

            if !remainder.is_empty() {
                return Err(OwnedTransactionError::UnexpectedNumberOfBytes {
                    actual: buffer_len,
                    expected: u32::try_from(transaction.encoded_size())
                        .expect("Smaller than the buffer, whose length is `u32`; qed"),
                });
            }

            Ok(YokeableTransaction(transaction))
        })?;

        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Inner buffer with owned transaction contents
    #[inline(always)]
    pub fn buffer(&self) -> &SharedAlignedBuffer {
        self.inner.backing_cart()
    }

    /// Transaction hash, the same as [`Transaction::hash()`], computed once and shared by clones
    #[inline(always)]
    pub fn hash(&self) -> TransactionHash {
        *self.transaction().hash()
    }

    /// Get [`Transaction`] out of [`OwnedTransaction`]
    #[inline(always)]
    pub fn transaction(&self) -> &Transaction<'_> {
        let YokeableTransaction(transaction) = self.inner.get();
        transaction
    }
}
