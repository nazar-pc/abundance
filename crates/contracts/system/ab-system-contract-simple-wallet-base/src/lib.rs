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
//! * [`SimpleWalletBase::change_public_key`] is used for change public key to a different one

#![feature(non_null_from_ref, ptr_as_ref_unchecked, try_blocks, unchecked_shifts)]
#![no_std]

pub mod payload;
pub mod seal;

use crate::payload::{TransactionMethodContext, TransactionPayloadDecoder};
use crate::seal::hash_and_verify;
use ab_contracts_common::env::{Env, MethodContext, TransactionHeader};
use ab_contracts_common::{ContractError, MAX_TOTAL_METHOD_ARGS};
use ab_contracts_io_type::trivial_type::TrivialType;
use ab_contracts_io_type::variable_bytes::VariableBytes;
use ab_contracts_macros::contract;
use ab_contracts_standards::tx_handler::{TxHandlerPayload, TxHandlerSeal, TxHandlerSlots};
use core::mem::MaybeUninit;
use core::ptr;
use schnorrkel::PublicKey;

/// Context for transaction signatures, see [`SigningContext`].
///
/// [`SigningContext`]: schnorrkel::context::SigningContext
///
/// This constant is helpful for frontend/hardware wallet implementations.
pub const SIGNING_CONTEXT: &[u8] = b"system-simple-wallet";
/// Size of the buffer in pointers that is used for `ExternalArgs` pointers.
///
/// This constant is helpful for transaction generation to check whether a created transaction
/// doesn't exceed this limit.
///
/// `#[slot]` argument using one pointer, `#[input]` two pointers and `#[output]` three pointers
/// each.
pub const EXTERNAL_ARGS_BUFFER_SIZE: usize = 3 * MAX_TOTAL_METHOD_ARGS as usize;
/// Size of the buffer in bytes that is used as a stack for storing outputs.
///
/// This constant is helpful for transaction generation to check whether a created transaction
/// doesn't exceed this limit.
///
/// This defines how big the total size of `#[output]` arguments and return values could be in all
/// methods of the payload together.
///
/// Overflow will result in an error.
pub const OUTPUT_BUFFER_SIZE: usize = 32 * 1024;
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
#[derive(Debug, Copy, Clone, TrivialType)]
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
    pub fn initialize(
        #[input] &public_key: &[u8; 32],
        #[output] state: &mut VariableBytes,
    ) -> Result<(), ContractError> {
        // TODO: Storing some lower-level representation of the public key might reduce the cost of
        //  verification in `Self::authorize()` method
        // Ensure public key is valid
        PublicKey::from_bytes(&public_key).map_err(|_error| ContractError::BadInput)?;

        if !state.copy_from(&WalletState {
            public_key,
            nonce: 0,
        }) {
            return Err(ContractError::BadInput);
        }

        Ok(())
    }

    /// Reads state of `owner` and returns `Ok(())` if authorization succeeds
    #[view]
    pub fn authorize(
        #[input] state: &VariableBytes,
        #[input] header: &TransactionHeader,
        #[input] read_slots: &TxHandlerSlots,
        #[input] write_slots: &TxHandlerSlots,
        #[input] payload: &TxHandlerPayload,
        #[input] seal: &TxHandlerSeal,
    ) -> Result<(), ContractError> {
        let Some(state) = state.read_trivial_type::<WalletState>() else {
            return Err(ContractError::BadInput);
        };
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
        let mut output_buffer_offsets = [MaybeUninit::uninit(); OUTPUT_BUFFER_OFFSETS_SIZE];

        let mut payload_decoder = TransactionPayloadDecoder::new(
            payload.get_initialized(),
            &mut external_args_buffer,
            &mut output_buffer,
            &mut output_buffer_offsets,
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
    pub fn increase_nonce(
        #[input] state: &VariableBytes,
        #[input] seal: &TxHandlerSeal,
        #[output] new_state: &mut VariableBytes,
    ) -> Result<(), ContractError> {
        let _ = seal;

        let Some(mut state) = state.read_trivial_type::<WalletState>() else {
            return Err(ContractError::BadInput);
        };

        state.nonce = state.nonce.checked_add(1).ok_or(ContractError::Forbidden)?;

        if !new_state.copy_from(&state) {
            return Err(ContractError::BadInput);
        }

        Ok(())
    }

    /// Returns state with a changed public key
    #[view]
    pub fn change_public_key(
        #[input] state: &VariableBytes,
        #[input] &public_key: &[u8; 32],
        #[output] new_state: &mut VariableBytes,
    ) -> Result<(), ContractError> {
        let Some(mut state) = state.read_trivial_type::<WalletState>() else {
            return Err(ContractError::BadInput);
        };
        // Ensure public key is valid
        PublicKey::from_bytes(&public_key).map_err(|_error| ContractError::BadInput)?;

        state.public_key = public_key;

        if !new_state.copy_from(&state) {
            return Err(ContractError::BadInput);
        }

        Ok(())
    }
}
