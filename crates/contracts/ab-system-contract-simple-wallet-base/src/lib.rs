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

#![feature(non_null_from_ref, try_blocks)]
#![no_std]

pub mod payload;

use crate::payload::{TransactionMethodContext, TransactionPayloadDecoder};
use ab_contracts_common::ContractError;
use ab_contracts_common::env::{Env, MethodContext, TransactionHeader};
use ab_contracts_io_type::trivial_type::TrivialType;
use ab_contracts_io_type::variable_bytes::VariableBytes;
use ab_contracts_macros::contract;
use ab_contracts_standards::tx_handler::{TxHandlerPayload, TxHandlerSeal};
use core::mem::MaybeUninit;
use core::{ptr, slice};
use schnorrkel::context::SigningContext;

/// Context for transaction signatures, see [`SigningContext`].
///
/// This constant is helpful for frontend/hardware wallet implementations.
pub const SIGNING_CONTEXT: &[u8] = b"system-simple-wallet";
/// Size of the buffer in pointers that is used for `ExternalArgs` pointers.
///
/// This constant is helpful for transaction generation to check whether a created transaction
/// doesn't exceed this limit.
///
/// `#[slot]` argument using one pointer, `#[input]` two pointers and `#[output]`/`#[result]` three
/// pointers each.
pub const EXTERNAL_ARGS_BUFFER_SIZE: usize = 32 * 1024;
/// Size of the buffer in bytes that is used as a stack for storing outputs.
///
/// This constant is helpful for transaction generation to check whether a created transaction
/// doesn't exceed this limit.
///
/// This defines how big the total sum of `#[output]` and `#[result]` could be in all methods of the
/// payload together.
///
/// Overflow will result in an error.
pub const OUTPUT_BUFFER_SIZE: usize = 32 * 1024;
/// Size of the buffer in entries that is used to store buffer offsets.
///
/// This constant is helpful for transaction generation to check whether a created transaction
/// doesn't exceed this limit.
///
/// This defines how many `#[output]` and `#[result]` arguments could exist in all methods of the
/// payload together.
///
/// Overflow will result in an error.
pub const OUTPUT_BUFFER_OFFSETS_SIZE: usize = 16;

/// Transaction seal.
///
/// Contains signature and nonce, this is necessary to produce a correctly sealed transaction.
#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
pub struct Seal {
    pub signature: [u8; 64],
    pub nonce: u64,
}

/// State of the wallet.
///
/// Shouldn't be necessary to use directly.
#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
pub struct WalletState {
    pub public_key: [u8; 32],
    pub nonce: u64,
}

/// A simple wallet contract base contract to be used by other contracts.
///
/// See the module description for details.
#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
pub struct SimpleWalletBase;

#[contract]
impl SimpleWalletBase {
    /// Returns initial state with a provided public key
    #[view]
    pub fn initialize(
        #[input] &public_key: &[u8; 32],
        #[output] state: &mut VariableBytes<0>,
    ) -> Result<(), ContractError> {
        // TODO: Storing some lower-level representation of the public key might reduce the cost of
        //  verification in `Self::authorize()` method
        // Ensure public key is valid
        schnorrkel::PublicKey::from_bytes(&public_key).map_err(|_error| ContractError::BadInput)?;

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
        #[input] state: &VariableBytes<0>,
        #[input] header: &TransactionHeader,
        #[input] payload: &TxHandlerPayload,
        #[input] seal: &TxHandlerSeal,
    ) -> Result<(), ContractError> {
        let Some(state) = state.read_trivial_type::<WalletState>() else {
            return Err(ContractError::BadInput);
        };
        let Some(seal) = seal.read_trivial_type::<Seal>() else {
            return Err(ContractError::BadInput);
        };

        let tx_hash = {
            let mut hasher = blake3::Hasher::new();
            hasher.update(header.as_bytes());
            let payload = payload.get_initialized();
            // SAFETY: Valid memory of correct size
            let payload_bytes = unsafe {
                slice::from_raw_parts(payload.as_ptr().cast::<u8>(), size_of_val(payload))
            };
            hasher.update(payload_bytes);
            hasher.update(&seal.nonce.to_le_bytes());
            hasher.finalize()
        };

        if Some(seal.nonce) == state.nonce.checked_add(1) {
            return Err(ContractError::BadInput);
        }

        let public_key = schnorrkel::PublicKey::from_bytes(state.public_key.as_ref())
            .expect("Guaranteed by constructor; qed");
        let signature = schnorrkel::Signature::from_bytes(&seal.signature)
            .map_err(|_error| ContractError::BadInput)?;
        let signing_context = SigningContext::new(SIGNING_CONTEXT);
        public_key
            .verify(signing_context.bytes(tx_hash.as_bytes()), &signature)
            .map_err(|_error| ContractError::Forbidden)
    }

    /// Executes provided transactions in the payload, *remember to also [`Self::increase_nonce()`]
    /// afterward* unless there is a very good reason not to (like when wallet was replaced with
    /// another implementation containing a different state).
    ///
    /// Caller must set themselves as a context or else error will be returned.
    #[update]
    pub fn execute(
        #[env] env: &mut Env,
        #[input] _header: &TransactionHeader,
        #[input] payload: &TxHandlerPayload,
        #[input] _seal: &TxHandlerSeal,
    ) -> Result<(), ContractError> {
        // Only allow direct calls by context owner
        if env.caller() != env.context() {
            return Err(ContractError::Forbidden);
        }

        let mut external_args_buffer = [ptr::null_mut(); EXTERNAL_ARGS_BUFFER_SIZE];
        let mut output_buffer = [MaybeUninit::uninit(); OUTPUT_BUFFER_SIZE];
        let mut output_buffer_offsets = [(0, 0); OUTPUT_BUFFER_OFFSETS_SIZE];

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
            env.call_many([prepared_method])?;
        }

        Ok(())
    }

    /// Returns state with increased nonce
    #[view]
    pub fn increase_nonce(
        #[input] state: &VariableBytes<0>,
        #[input] _seal: &TxHandlerSeal,
        #[output] new_state: &mut VariableBytes<0>,
    ) -> Result<(), ContractError> {
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
        #[input] state: &VariableBytes<0>,
        #[input] &public_key: &[u8; 32],
        #[output] new_state: &mut VariableBytes<0>,
    ) -> Result<(), ContractError> {
        let Some(mut state) = state.read_trivial_type::<WalletState>() else {
            return Err(ContractError::BadInput);
        };
        // Ensure public key is valid
        schnorrkel::PublicKey::from_bytes(&public_key).map_err(|_error| ContractError::BadInput)?;

        state.public_key = public_key;

        if !new_state.copy_from(&state) {
            return Err(ContractError::BadInput);
        }

        Ok(())
    }
}
