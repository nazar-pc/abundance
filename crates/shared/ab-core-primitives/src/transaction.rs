//! Transaction-related primitives

#[cfg(feature = "alloc")]
pub mod owned;

use crate::address::Address;
use crate::block::BlockRoot;
use crate::hashes::Blake3Hash;
#[cfg(feature = "alloc")]
use crate::transaction::owned::{OwnedTransaction, OwnedTransactionError};
use ab_io_type::alignment::Alignment;
use ab_io_type::trivial_type::TrivialType;
use blake3::Hasher;
use core::slice;
use derive_more::{Deref, DerefMut, Display, From, Into};

/// A measure of compute resources, 1 Gas == 1 ns of compute on reference hardware
#[derive(Debug, Default, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct Gas(u64);

/// Transaction hash
#[derive(
    Debug,
    Display,
    Default,
    Copy,
    Clone,
    Ord,
    PartialOrd,
    Eq,
    PartialEq,
    Hash,
    From,
    Into,
    Deref,
    DerefMut,
    TrivialType,
)]
#[repr(C)]
pub struct TransactionHash(Blake3Hash);

impl AsRef<[u8]> for TransactionHash {
    #[inline(always)]
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl AsMut<[u8]> for TransactionHash {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut [u8] {
        self.0.as_mut()
    }
}

/// Transaction header
#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct TransactionHeader {
    // TODO: Right now this is primarily used for data alignment, but is it useful in general?
    // TODO: Some more complex field?
    /// Transaction version
    pub version: u64,
    /// Block root at which transaction was created
    pub block_root: BlockRoot,
    /// Gas limit
    pub gas_limit: Gas,
    /// Contract implementing `TxHandler` trait to use for transaction verification and execution
    pub contract: Address,
}

impl TransactionHeader {
    /// The only supported transaction version right now
    pub const TRANSACTION_VERSION: u64 = 0;
}

/// Transaction slot
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, TrivialType)]
#[repr(C)]
pub struct TransactionSlot {
    /// Slot owner
    pub owner: Address,
    /// Contract that manages the slot
    pub contract: Address,
}

/// Lengths of various components in a serialized version of [`Transaction`]
#[derive(Debug, Default, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct SerializedTransactionLengths {
    /// Number of read-only slots
    pub read_slots: u16,
    /// Number of read-write slots
    pub write_slots: u16,
    /// Payload length in bytes, must be a multiple of `u128` size
    pub payload: u32,
    /// Seal length in bytes
    pub seal: u32,
    /// Not used and must be set to `0`
    pub padding: [u8; 4],
}

/// Similar to `Transaction`, but doesn't require `allow` or data ownership.
///
/// Can be created with `Transaction::as_ref()` call.
#[derive(Debug, Copy, Clone)]
pub struct Transaction<'a> {
    /// Transaction header
    pub header: &'a TransactionHeader,
    /// Slots in the form of [`TransactionSlot`] that may be read during transaction processing.
    ///
    /// These are the only slots that can be used in authorization code.
    ///
    /// The code slot of the contract that is being executed and balance of native token are
    /// implicitly included and doesn't need to be specified (see [`Transaction::read_slots()`].
    /// Also slots that may also be written to do not need to be repeated in the read slots.
    pub read_slots: &'a [TransactionSlot],
    /// Slots in the form of [`TransactionSlot`] that may be written during transaction processing
    pub write_slots: &'a [TransactionSlot],
    /// Transaction payload
    pub payload: &'a [u128],
    /// Transaction seal
    pub seal: &'a [u8],
}

impl<'a> Transaction<'a> {
    /// Create an instance from provided correctly aligned bytes.
    ///
    /// `bytes` should be 16-bytes aligned.
    ///
    /// The layout is following data structures concatenated as bytes (they are carefully picked to
    /// ensure alignment):
    /// * [`TransactionHeader`]
    /// * [`SerializedTransactionLengths`] (with values set to correspond to below contents)
    /// * All read [`TransactionSlot`]
    /// * All write [`TransactionSlot`]
    /// * Payload as `u128`s
    /// * Seal as `u8`s
    ///
    /// Payload and seal lengths in [`SerializedTransactionLengths`] are in bytes.
    ///
    /// Returns an instance and remaining bytes on success.
    #[inline]
    #[expect(
        clippy::cast_ptr_alignment,
        reason = "Alignment of `bytes` is checked and offsets within them are multiples of it"
    )]
    pub fn try_from_bytes(bytes: &'a [u8]) -> Option<(Self, &'a [u8])> {
        const {
            // Slots after `SerializedTransactionLengths` and payload after slots must be aligned
            assert!(
                (size_of::<TransactionHeader>() + size_of::<SerializedTransactionLengths>())
                    .is_multiple_of(usize::from(Alignment::MAX.get()))
            );
            assert!(size_of::<TransactionSlot>().is_multiple_of(usize::from(Alignment::MAX.get())));
            assert!(align_of::<TransactionSlot>() <= usize::from(Alignment::MAX.get()));
            assert!(align_of::<u128>() <= usize::from(Alignment::MAX.get()));
        }

        if !Alignment::MAX.is_slice_aligned(bytes) {
            return None;
        }

        let (header, remainder) = TransactionHeader::from_bytes(bytes)?;
        let (lengths, remainder) = SerializedTransactionLengths::from_bytes(remainder)?;
        let SerializedTransactionLengths {
            read_slots,
            write_slots,
            payload,
            seal,
            padding,
        } = *lengths;

        if padding != [0; _] {
            return None;
        }

        if !payload.is_multiple_of(u128::SIZE) {
            return None;
        }

        let read_slots = usize::from(read_slots);
        let write_slots = usize::from(write_slots);
        // Lossless, the crate only compiles for targets where `usize` is at least 32 bits (see the
        // assertion in the crate root)
        let payload = payload as usize;
        let seal = seal as usize;

        let (read_slots_bytes, remainder) =
            remainder.split_at_checked(read_slots * size_of::<TransactionSlot>())?;
        let (write_slots_bytes, remainder) =
            remainder.split_at_checked(write_slots * size_of::<TransactionSlot>())?;
        let (payload_bytes, remainder) = remainder.split_at_checked(payload)?;
        let (seal, remainder) = remainder.split_at_checked(seal)?;

        let transaction = Self {
            header,
            // SAFETY: Any bytes are valid for `TransactionSlot`, there are enough of them as
            // checked above, and they are correctly aligned since `bytes` are aligned to
            // `Alignment::MAX` and everything before them is a multiple of it (see the assertions
            // above)
            read_slots: unsafe {
                slice::from_raw_parts(
                    read_slots_bytes.as_ptr().cast::<TransactionSlot>(),
                    read_slots,
                )
            },
            // SAFETY: Any bytes are valid for `TransactionSlot`, there are enough of them as
            // checked above, and they are correctly aligned since `bytes` are aligned to
            // `Alignment::MAX` and everything before them is a multiple of it (see the assertions
            // above)
            write_slots: unsafe {
                slice::from_raw_parts(
                    write_slots_bytes.as_ptr().cast::<TransactionSlot>(),
                    write_slots,
                )
            },
            // SAFETY: Any bytes are valid for `u128`, there are enough of them as checked above
            // (the length is a multiple of `u128` size), and they are correctly aligned since
            // `bytes` are aligned to `Alignment::MAX` and everything before them is a multiple of
            // it (see the assertions above)
            payload: unsafe {
                slice::from_raw_parts(
                    payload_bytes.as_ptr().cast::<u128>(),
                    payload / size_of::<u128>(),
                )
            },
            seal,
        };

        Some((transaction, remainder))
    }

    /// Create an owned version of this transaction
    #[cfg(feature = "alloc")]
    #[inline(always)]
    pub fn to_owned(self) -> Result<OwnedTransaction, OwnedTransactionError> {
        OwnedTransaction::from_transaction(self)
    }

    /// Size of the encoded transaction in bytes
    pub const fn encoded_size(&self) -> usize {
        size_of::<TransactionHeader>()
            + size_of::<SerializedTransactionLengths>()
            + size_of_val(self.read_slots)
            + size_of_val(self.write_slots)
            + size_of_val(self.payload)
            + size_of_val(self.seal)
    }

    /// Compute transaction hash.
    ///
    /// Note: this computes transaction hash on every call, so worth caching if it is expected to be
    /// called often.
    pub fn hash(&self) -> TransactionHash {
        let mut hasher = Hasher::new();

        hasher.update(self.header.as_bytes());
        // SAFETY: `TransactionSlot` is `TrivialType` and can be treated as bytes
        hasher.update(unsafe {
            slice::from_raw_parts(
                self.read_slots.as_ptr().cast::<u8>(),
                size_of_val(self.read_slots),
            )
        });
        // SAFETY: `TransactionSlot` is `TrivialType` and can be treated as bytes
        hasher.update(unsafe {
            slice::from_raw_parts(
                self.write_slots.as_ptr().cast::<u8>(),
                size_of_val(self.write_slots),
            )
        });
        // SAFETY: `u128` and can be treated as bytes
        hasher.update(unsafe {
            slice::from_raw_parts(
                self.payload.as_ptr().cast::<u8>(),
                size_of_val(self.payload),
            )
        });
        hasher.update(self.seal);

        TransactionHash(Blake3Hash::from(hasher.finalize()))
    }

    /// Read slots touched by the transaction.
    ///
    /// In contrast to `read_slots` property, this includes implicitly used slots.
    pub fn read_slots(&self) -> impl Iterator<Item = TransactionSlot> {
        // Slots included implicitly that are always used
        let implicit_slots = [
            TransactionSlot {
                owner: self.header.contract,
                contract: Address::SYSTEM_CODE,
            },
            // TODO: Uncomment once system token contract exists
            // TransactionSlot {
            //     owner: self.header.contract,
            //     contract: Address::SYSTEM_TOKEN,
            // },
        ];

        implicit_slots
            .into_iter()
            .chain(self.read_slots.iter().copied())
    }

    /// All slots touched by the transaction.
    ///
    /// In contrast to `read_slots` and `write_slots` properties, this includes implicitly used
    /// slots.
    pub fn slots(&self) -> impl Iterator<Item = TransactionSlot> {
        self.read_slots().chain(self.write_slots.iter().copied())
    }
}
