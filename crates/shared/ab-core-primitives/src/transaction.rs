//! Transaction-related primitives

#[cfg(feature = "alloc")]
pub mod owned;

use crate::address::Address;
use crate::block::BlockHash;
use crate::hashes::Blake3Hash;
use ab_io_type::trivial_type::TrivialType;
use blake3::Hasher;
use core::slice;
use derive_more::{Deref, DerefMut, From, Into};

/// A measure of compute resources, 1 Gas == 1 ns of compute on reference hardware
#[derive(Debug, Default, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct Gas(u64);

/// Transaction hash
#[derive(
    Debug,
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
    // TODO: Some more complex field?
    /// Transaction version
    pub version: u64,
    /// Block hash at which transaction was created
    pub block_hash: BlockHash,
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
    /// Payload length
    pub payload: u32,
    /// Seal length
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
    /// See [`Self::from_bytes_unchecked()`] for layout details.
    ///
    /// Returns an instance and remaining bytes on success.
    #[inline]
    pub fn try_from_bytes(mut bytes: &'a [u8]) -> Option<(Self, &'a [u8])> {
        if !bytes.as_ptr().cast::<u128>().is_aligned()
            || bytes.len()
                < size_of::<TransactionHeader>() + size_of::<SerializedTransactionLengths>()
        {
            return None;
        }

        // SAFETY: Checked above that there are enough bytes and they are correctly aligned
        let lengths = unsafe {
            bytes
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
            return None;
        }

        if payload % u128::SIZE != 0 {
            return None;
        }

        let size = (size_of::<TransactionHeader>() + size_of::<SerializedTransactionLengths>())
            .checked_add(usize::from(read_slots) * size_of::<TransactionSlot>())?
            .checked_add(usize::from(write_slots) * size_of::<TransactionSlot>())?
            .checked_add(payload as usize * size_of::<u128>())?
            .checked_add(seal as usize)?;

        if bytes.len() < size {
            return None;
        }

        // SAFETY: Size and alignment checked above
        let transaction = unsafe { Self::from_bytes_unchecked(bytes) };
        let remainder = bytes.split_off(transaction.encoded_size()..)?;

        Some((transaction, remainder))
    }

    /// Create an instance from provided bytes without performing any checks for size or alignment.
    ///
    /// The internal layout of the owned transaction is following data structures concatenated as
    /// bytes (they are carefully picked to ensure alignment):
    /// * [`TransactionHeader`]
    /// * [`SerializedTransactionLengths`] (with values set to correspond to below contents)
    /// * All read [`TransactionSlot`]
    /// * All write [`TransactionSlot`]
    /// * Payload as `u128`s
    /// * Seal as `u8`s
    ///
    /// # Safety
    /// Caller must ensure provided bytes are 16-bytes aligned and of sufficient length. Extra bytes
    /// beyond necessary are silently ignored if provided.
    #[inline]
    pub unsafe fn from_bytes_unchecked(bytes: &'a [u8]) -> Transaction<'a> {
        // SAFETY: Method contract guarantees size and alignment
        let lengths = unsafe {
            bytes
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
            padding: _,
        } = lengths;

        Self {
            // SAFETY: Any bytes are valid for `TransactionHeader` and all method contract
            // guarantees there are enough bytes for header in the buffer
            header: unsafe {
                bytes
                    .as_ptr()
                    .cast::<TransactionHeader>()
                    .as_ref_unchecked()
            },
            // SAFETY: Any bytes are valid for `TransactionSlot` and all method contract guarantees
            // there are enough bytes for read slots in the buffer
            read_slots: unsafe {
                slice::from_raw_parts(
                    bytes
                        .as_ptr()
                        .add(size_of::<TransactionHeader>())
                        .add(size_of::<SerializedTransactionLengths>())
                        .cast::<TransactionSlot>(),
                    usize::from(read_slots),
                )
            },
            // SAFETY: Any bytes are valid for `TransactionSlot` and all method contract guarantees
            // there are enough bytes for write slots in the buffer
            write_slots: unsafe {
                slice::from_raw_parts(
                    bytes
                        .as_ptr()
                        .add(size_of::<TransactionHeader>())
                        .add(size_of::<SerializedTransactionLengths>())
                        .cast::<TransactionSlot>()
                        .add(usize::from(read_slots)),
                    usize::from(write_slots),
                )
            },
            // SAFETY: Any bytes are valid for `payload` and all method contract guarantees there
            // are enough bytes for payload in the buffer
            payload: unsafe {
                slice::from_raw_parts(
                    bytes
                        .as_ptr()
                        .add(size_of::<TransactionHeader>())
                        .add(size_of::<SerializedTransactionLengths>())
                        .add(
                            size_of::<TransactionSlot>()
                                * (usize::from(read_slots) + usize::from(write_slots)),
                        )
                        .cast::<u128>(),
                    payload as usize,
                )
            },
            // SAFETY: Any bytes are valid for `seal` and all method contract guarantees there are
            // enough bytes for seal in the buffer
            seal: unsafe {
                slice::from_raw_parts(
                    bytes
                        .as_ptr()
                        .add(size_of::<TransactionHeader>())
                        .add(size_of::<SerializedTransactionLengths>())
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
        // TODO: Keyed hash
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
