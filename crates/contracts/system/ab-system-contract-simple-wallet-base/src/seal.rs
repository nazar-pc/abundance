//! Utilities for [`Seal`] creation and verification

use crate::{SIGNING_CONTEXT, Seal};
use ab_contracts_common::ContractError;
use ab_contracts_common::env::Blake3Hash;
use ab_contracts_common::transaction::{TransactionHeader, TransactionSlot};
use ab_contracts_io_type::trivial_type::TrivialType;
use core::slice;
use schnorrkel::context::SigningContext;
use schnorrkel::{Keypair, PublicKey, Signature};

/// Create transaction hash used for signing with [`sign()`].
///
/// [`hash_and_sign()`] helper function exists that combines this method with [`sign()`].
pub fn hash_transaction(
    header: &TransactionHeader,
    read_slots: &[TransactionSlot],
    write_slots: &[TransactionSlot],
    payload: &[u128],
    nonce: u64,
) -> Blake3Hash {
    let mut hasher = blake3::Hasher::new();
    hasher.update(header.as_bytes());
    for slot in read_slots {
        hasher.update(slot.as_bytes());
    }
    for slot in write_slots {
        hasher.update(slot.as_bytes());
    }
    // SAFETY: Valid memory of correct size
    let payload_bytes =
        unsafe { slice::from_raw_parts(payload.as_ptr().cast::<u8>(), size_of_val(payload)) };
    hasher.update(payload_bytes);
    hasher.update(&nonce.to_le_bytes());
    *hasher.finalize().as_bytes()
}

/// Sign transaction hash created with [`hash_transaction()`].
///
/// [`hash_and_sign()`] helper function exists that combines this method with
/// [`hash_transaction()`].
pub fn sign(keypair: &Keypair, tx_hash: &Blake3Hash) -> Signature {
    let signing_context = SigningContext::new(SIGNING_CONTEXT);
    keypair.sign(signing_context.bytes(tx_hash))
}

/// Combines [`hash_transaction()`] and [`sign()`] and returns [`Seal`]
pub fn hash_and_sign(
    keypair: &Keypair,
    header: &TransactionHeader,
    read_slots: &[TransactionSlot],
    write_slots: &[TransactionSlot],
    payload: &[u128],
    nonce: u64,
) -> Seal {
    let tx_hash = hash_transaction(header, read_slots, write_slots, payload, nonce);
    let signature = sign(keypair, &tx_hash).to_bytes();

    Seal { signature, nonce }
}

/// Verify seal created by [`hash_and_sign()`].
///
/// [`hash_and_verify()`] helper function exists that combines this method with
/// [`hash_transaction()`].
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub fn verify(
    public_key: &PublicKey,
    expected_nonce: u64,
    tx_hash: &Blake3Hash,
    signature: &Signature,
    nonce: u64,
) -> Result<(), ContractError> {
    if nonce != expected_nonce {
        return Err(ContractError::BadInput);
    }

    let signing_context = SigningContext::new(SIGNING_CONTEXT);
    public_key
        .verify(signing_context.bytes(tx_hash.as_bytes()), signature)
        .map_err(|_error| ContractError::Forbidden)
}

// TODO: Add guarantees that this does not panic
/// Combines [`hash_transaction()`] and [`verify()`]
pub fn hash_and_verify(
    public_key: &PublicKey,
    expected_nonce: u64,
    header: &TransactionHeader,
    read_slots: &[TransactionSlot],
    write_slots: &[TransactionSlot],
    payload: &[u128],
    seal: &Seal,
) -> Result<(), ContractError> {
    if seal.nonce != expected_nonce {
        return Err(ContractError::BadInput);
    }

    let tx_hash = hash_transaction(header, read_slots, write_slots, payload, seal.nonce);
    let signature =
        Signature::from_bytes(&seal.signature).map_err(|_error| ContractError::BadInput)?;
    let signing_context = SigningContext::new(SIGNING_CONTEXT);
    public_key
        .verify(signing_context.bytes(tx_hash.as_bytes()), &signature)
        .map_err(|_error| ContractError::Forbidden)
}
