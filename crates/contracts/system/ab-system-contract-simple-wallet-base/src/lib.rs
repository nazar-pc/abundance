//! A simple wallet contract base contract to be used by other contracts
//!
//! It includes the core logic, making contracts using it much more compact. The implementation is
//! based on [`schnorrkel`] crate and its SR25519 signature scheme.
//!
//! It abstracts away its inner types in the public API to allow it to evolve over time.
//!
//! The general workflow is:
//! * [`SimpleWalletBase::initialize`] is used for wallet initialization
//! * [`SimpleWalletBase::authorize`] is used for authorization
//! * [`SimpleWalletBase::execute`] is used for executing method calls contained in the payload,
//!   followed by [`SimpleWalletBase::increase_nonce`]
//! * [`SimpleWalletBase::change_public_key`] is used to change a public key to a different one

#![feature(
    const_block_items,
    const_convert,
    const_trait_impl,
    maybe_uninit_as_bytes,
    mut_restriction,
    slice_ptr_get
)]
#![cfg_attr(feature = "no-panic", feature(const_closures))]
#![no_std]

pub mod payload;
pub mod seal;
pub mod utils;

use crate::payload::TransactionMethodContext;
use crate::payload::caller::{OutputChunk, TransactionPayloadCallError, TransactionPayloadCaller};
use crate::seal::hash_and_verify;
use ab_contracts_common::env::{Env, MethodContext};
use ab_contracts_common::{ContractError, MAX_TOTAL_METHOD_ARGS};
use ab_contracts_macros::contract;
use ab_contracts_standards::tx_handler::{TxHandlerPayload, TxHandlerSeal, TxHandlerSlots};
use ab_core_primitives::transaction::TransactionHeader;
use ab_io_type::trivial_type::TrivialType;
use core::ffi::c_void;
use core::mem::MaybeUninit;
use core::ptr;
use schnorrkel::PublicKey;

/// Context for transaction signatures, see [`SigningContext`].
///
/// [`SigningContext`]: schnorrkel::context::SigningContext
///
/// This constant is helpful for frontend/hardware wallet implementations.
pub const SIGNING_CONTEXT: &[u8] = b"system-simple-wallet";
/// Size of the buffer (in pointers) that is used for `ExternalArgs` pointers.
///
/// This constant is helpful for transaction generation to check whether a created transaction
/// doesn't exceed this limit.
///
/// `#[slot]` argument using one pointer, `#[input]` and `#[output]` use one pointer and size +
/// capacity each.
pub const EXTERNAL_ARGS_BUFFER_SIZE: usize = MAX_TOTAL_METHOD_ARGS as usize
    * (size_of::<*mut c_void>() + size_of::<u32>() * 2)
    / size_of::<*mut c_void>();
/// Size of the buffer in [`OutputChunk`] elements that holds the memory of the outputs that a
/// payload declares.
///
/// This constant is helpful for transaction generation to check whether a created transaction
/// doesn't exceed this limit.
///
/// This defines how big the outputs that a payload declares could be together. Each of them starts
/// at the next multiple of 16 bytes ([`Alignment::MAX`]) after the previous one.
///
/// Overflow will result in an error before any method is called.
///
/// [`Alignment::MAX`]: ab_io_type::alignment::Alignment::MAX
pub const OUTPUT_BUFFER_SIZE: usize = 32 * 1024 / size_of::<OutputChunk>();
/// The largest number of outputs that a payload can declare.
///
/// This constant is helpful for transaction generation to check whether a created transaction
/// doesn't exceed this limit.
///
/// Every `#[output]` argument and return value of the payload's methods writes one of the declared
/// outputs, and several methods can write the same one. Slots, inputs and outputs refer to a
/// declared output by its index, which is below this limit.
///
/// Overflow will result in an error before any method is called.
pub const MAX_OUTPUTS: usize = 16;

/// Transaction seal.
///
/// Contains signature and nonce, this is necessary to produce a correctly sealed transaction.
#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct Seal {
    pub signature: [u8; 64],
    pub nonce: u64,
}

/// State of the wallet.
///
/// Shouldn't be necessary to use directly.
#[derive(Debug, Copy, Clone, Eq, PartialEq, TrivialType)]
#[repr(C)]
pub struct WalletState {
    pub public_key: [u8; 32],
    pub nonce: u64,
}

/// A simple wallet contract base contract to be used by other contracts.
///
/// See the module description for details.
#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct SimpleWalletBase;

#[contract]
impl SimpleWalletBase {
    /// Returns initial state with a provided public key
    #[view]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub fn initialize(#[input] &public_key: &[u8; 32]) -> Result<WalletState, ContractError> {
        // TODO: Storing some lower-level representation of the public key might reduce the cost of
        //  verification in `Self::authorize()` method
        // Ensure public key is valid
        PublicKey::from_bytes(&public_key).map_err(|_error| ContractError::BadInput)?;

        Ok(WalletState {
            public_key,
            nonce: 0,
        })
    }

    /// Reads state of `owner` and returns `Ok(())` if authorization succeeds.
    ///
    /// `seal` must hold exactly the bytes of a [`Seal`].
    #[view]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub fn authorize(
        #[input] state: &WalletState,
        #[input] header: &TransactionHeader,
        #[input] read_slots: &TxHandlerSlots,
        #[input] write_slots: &TxHandlerSlots,
        #[input] payload: &TxHandlerPayload,
        #[input] seal: &TxHandlerSeal,
    ) -> Result<(), ContractError> {
        let Some(seal) = Seal::try_read_unaligned_exact(seal.get_initialized()) else {
            return Err(ContractError::BadInput);
        };

        let expected_nonce = state.nonce;
        // Check if max nonce value was already reached
        if expected_nonce.checked_add(1).is_none() {
            return Err(ContractError::Forbidden);
        }

        let public_key = PublicKey::from_bytes(state.public_key.as_ref())
            .expect("Guaranteed by constructor; qed");
        hash_and_verify(
            &public_key,
            expected_nonce,
            header,
            read_slots.get_initialized(),
            write_slots.get_initialized(),
            payload.get_initialized(),
            &seal,
        )
    }

    /// Executes provided transactions in the payload.
    ///
    /// IMPORTANT:
    /// * *must only be called with trusted input*, for example, successful signature verification
    ///   in [`SimpleWalletBase::authorize()`] implies transaction was seen and verified by the user
    /// * *remember to also [`SimpleWalletBase::increase_nonce()`] afterward* unless there is a very
    ///   good reason not to (like when wallet was replaced with another implementation containing a
    ///   different state)
    ///
    /// The caller must set themselves as a context or else error will be returned.
    #[update]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub fn execute(
        #[env] env: &mut Env<'_>,
        #[input] header: &TransactionHeader,
        #[input] read_slots: &TxHandlerSlots,
        #[input] write_slots: &TxHandlerSlots,
        #[input] payload: &TxHandlerPayload,
        #[input] seal: &TxHandlerSeal,
    ) -> Result<(), ContractError> {
        let _: &_ = header;
        let _: &_ = read_slots;
        let _: &_ = write_slots;
        let _: &_ = seal;

        // Only allow direct calls by context owner
        if env.caller() != env.context() {
            return Err(ContractError::Forbidden);
        }

        let mut external_args_buffer = [ptr::null_mut(); EXTERNAL_ARGS_BUFFER_SIZE];
        let mut output_buffer = [MaybeUninit::uninit(); OUTPUT_BUFFER_SIZE];
        let mut declared_outputs = [MaybeUninit::uninit(); MAX_OUTPUTS];

        // The outputs that the payload declares must fit, before any method is called
        let Ok(mut payload_caller) = TransactionPayloadCaller::new(
            payload.get_initialized(),
            &mut external_args_buffer,
            &mut output_buffer,
            &mut declared_outputs,
            |method_context| match method_context {
                TransactionMethodContext::Null => MethodContext::Reset,
                TransactionMethodContext::Wallet => MethodContext::Keep,
            },
        ) else {
            return Err(ContractError::BadInput);
        };

        loop {
            payload_caller = match payload_caller.call_next_method(env) {
                Ok(Some(payload_caller)) => payload_caller,
                Ok(None) => return Ok(()),
                Err(TransactionPayloadCallError::Decoder(_error)) => {
                    return Err(ContractError::BadInput);
                }
                Err(TransactionPayloadCallError::Method(error)) => return Err(error),
            };
        }
    }

    /// Returns state with increased nonce
    #[view]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub fn increase_nonce(#[input] state: &WalletState) -> Result<WalletState, ContractError> {
        let nonce = state.nonce.checked_add(1).ok_or(ContractError::Forbidden)?;

        Ok(WalletState {
            public_key: state.public_key,
            nonce,
        })
    }

    /// Returns a new state with a changed public key
    #[view]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub fn change_public_key(
        #[input] state: &WalletState,
        #[input] &public_key: &[u8; 32],
    ) -> Result<WalletState, ContractError> {
        // Ensure a public key is valid
        PublicKey::from_bytes(&public_key).map_err(|_error| ContractError::BadInput)?;

        Ok(WalletState {
            public_key,
            nonce: state.nonce,
        })
    }
}
