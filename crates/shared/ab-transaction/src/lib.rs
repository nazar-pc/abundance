#![feature(ptr_as_ref_unchecked)]
#![no_std]

#[cfg(feature = "alloc")]
pub mod owned;

use ab_contracts_common::Address;
use ab_contracts_common::env::Blake3Hash;
use ab_contracts_io_type::trivial_type::TrivialType;

/// A measure of compute resources, 1 Gas == 1 ns of compute on reference hardware
#[derive(Debug, Default, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct Gas(u64);

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct TransactionHeader {
    pub block_hash: Blake3Hash,
    pub gas_limit: Gas,
    /// Contract implementing `TxHandler` trait to use for transaction verification and execution
    pub contract: Address,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, TrivialType)]
#[repr(C)]
pub struct TransactionSlot {
    pub owner: Address,
    pub contract: Address,
}

/// Similar to `Transaction`, but doesn't require `allow` or data ownership.
///
/// Can be created with `Transaction::as_ref()` call.
#[derive(Debug, Copy, Clone)]
pub struct Transaction<'a> {
    pub header: &'a TransactionHeader,
    /// Slots in the form of [`TransactionSlot`] that may be read during transaction processing.
    ///
    /// The code slot of the contract that is being executed is implicitly included and doesn't need
    /// to be repeated. Also slots that may also be written to do not need to be repeated in the
    /// read slots.
    pub read_slots: &'a [TransactionSlot],
    /// Slots in the form of [`TransactionSlot`] that may be written during transaction processing
    pub write_slots: &'a [TransactionSlot],
    pub payload: &'a [u128],
    pub seal: &'a [u8],
}
