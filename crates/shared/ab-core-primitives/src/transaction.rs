//! Transaction-related primitives

#[cfg(feature = "alloc")]
pub mod owned;

use ab_contracts_common::Address;
use ab_contracts_common::block::BlockHash;
use ab_contracts_common::env::Blake3Hash;
use ab_io_type::trivial_type::TrivialType;
use blake3::Hasher;
use core::slice;

/// A measure of compute resources, 1 Gas == 1 ns of compute on reference hardware
#[derive(Debug, Default, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct Gas(u64);

/// Transaction hash
#[derive(Debug, Default, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, TrivialType)]
#[repr(C)]
pub struct TransactionHash(Blake3Hash);

/// Transaction header
#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct TransactionHeader {
    /// Block hash at which transaction was created
    pub block_hash: BlockHash,
    /// Gas limit
    pub gas_limit: Gas,
    /// Contract implementing `TxHandler` trait to use for transaction verification and execution
    pub contract: Address,
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

impl Transaction<'_> {
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

        TransactionHash(*hasher.finalize().as_bytes())
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
