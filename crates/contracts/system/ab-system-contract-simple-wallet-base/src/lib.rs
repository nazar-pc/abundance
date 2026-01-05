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

#![feature(maybe_uninit_as_bytes, ptr_as_ref_unchecked, slice_ptr_get, try_blocks)]
#![no_std]

pub mod payload;
pub mod seal;
pub mod utils;

use crate::payload::{TransactionMethodContext, TransactionPayloadDecoder};
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
/// Size of the buffer in `u128` elements that is used as a stack for storing outputs.
///
/// This constant is helpful for transaction generation to check whether a created transaction
/// doesn't exceed this limit.
///
/// This defines how big the total size of `#[output]` arguments and return values could be in all
/// methods of the payload together.
///
/// Overflow will result in an error.
pub const OUTPUT_BUFFER_SIZE: usize = 32 * 1024 / size_of::<u128>();
/// Size of the buffer in entries that is used to store buffer offsets.
///
/// This constant is helpful for transaction generation to check whether a created transaction
/// doesn't exceed this limit.
///
/// This defines how many `#[output]` arguments and return values could exist in all methods of the
/// payload together.
///
/// Overflow will result in an error.
pub const OUTPUT_BUFFER_OFFSETS_SIZE: usize = 16;

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
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
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

    /// Reads state of `owner` and returns `Ok(())` if authorization succeeds
    #[view]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn authorize(
        #[input] state: &WalletState,
        #[input] header: &TransactionHeader,
        #[input] read_slots: &TxHandlerSlots,
        #[input] write_slots: &TxHandlerSlots,
        #[input] payload: &TxHandlerPayload,
        #[input] seal: &TxHandlerSeal,
    ) -> Result<(), ContractError> {
        let Some(seal) = seal.read_trivial_type::<Seal>() else {
            return Err(ContractError::BadInput);
        };

        let expected_nonce = state.nonce;
        // Check if max nonce value was already reached
        if expected_nonce.checked_add(1).is_none() {
            return Err(ContractError::Forbidden);
        };

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
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn execute(
        #[env] env: &mut Env<'_>,
        #[input] header: &TransactionHeader,
        #[input] read_slots: &TxHandlerSlots,
        #[input] write_slots: &TxHandlerSlots,
        #[input] payload: &TxHandlerPayload,
        #[input] seal: &TxHandlerSeal,
    ) -> Result<(), ContractError> {
        let _ = header;
        let _ = read_slots;
        let _ = write_slots;
        let _ = seal;

        // Only allow direct calls by context owner
        if env.caller() != env.context() {
            return Err(ContractError::Forbidden);
        }

        let mut external_args_buffer = [ptr::null_mut(); EXTERNAL_ARGS_BUFFER_SIZE];
        let mut output_buffer = [MaybeUninit::uninit(); OUTPUT_BUFFER_SIZE];
        let mut output_buffer_details = [MaybeUninit::uninit(); OUTPUT_BUFFER_OFFSETS_SIZE];

        let mut payload_decoder = TransactionPayloadDecoder::new(
            payload.get_initialized(),
            &mut external_args_buffer,
            &mut output_buffer,
            &mut output_buffer_details,
            |method_context| match method_context {
                TransactionMethodContext::Null => MethodContext::Reset,
                TransactionMethodContext::Wallet => MethodContext::Keep,
            },
        );

        while let Some(prepared_method) = payload_decoder
            .decode_next_method()
            .map_err(|_error| ContractError::BadInput)?
        {
            env.call_prepared(prepared_method)?;
        }

        Ok(())
    }

    /// Returns state with increased nonce
    #[view]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn increase_nonce(#[input] state: &WalletState) -> Result<WalletState, ContractError> {
        let nonce = state.nonce.checked_add(1).ok_or(ContractError::Forbidden)?;

        Ok(WalletState {
            public_key: state.public_key,
            nonce,
        })
    }

    /// Returns a new state with a changed public key
    #[view]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
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
