//! Utilities for [`Seal`] creation and verification

use crate::{SIGNING_CONTEXT, Seal};
use ab_contracts_common::ContractError;
use ab_core_primitives::transaction::{TransactionHeader, TransactionSlot};
use ab_io_type::trivial_type::TrivialType;
use blake3::{Hasher, OUT_LEN};
use schnorrkel::context::SigningContext;
use schnorrkel::{Keypair, PublicKey, Signature};

/// Create transaction hash used for signing with [`sign()`].
///
/// [`hash_and_sign()`] helper function exists that combines this method with [`sign()`].
// TODO: `no_panic` can't prove lack of panics in `blake3::Hasher` for input of arbitrary length
// #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn hash_transaction(
    header: &TransactionHeader,
    read_slots: &[TransactionSlot],
    write_slots: &[TransactionSlot],
    payload: &[u128],
    nonce: u64,
) -> [u8; OUT_LEN] {
    let mut hasher = Hasher::new();
    hasher.update(header.as_bytes());
    hasher.update(TransactionSlot::slice_as_bytes(read_slots));
    hasher.update(TransactionSlot::slice_as_bytes(write_slots));
    hasher.update(u128::slice_as_bytes(payload));
    hasher.update(&nonce.to_le_bytes());
    hasher.finalize().into()
}

/// Sign transaction hash created with [`hash_transaction()`].
///
/// [`hash_and_sign()`] helper function exists that combines this method with
/// [`hash_transaction()`].
// TODO: `no_panic` can't prove lack of panics in `schnorrkel` signing
// #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn sign(keypair: &Keypair, tx_hash: &[u8; OUT_LEN]) -> Signature {
    let signing_context = SigningContext::new(SIGNING_CONTEXT);
    keypair.sign(signing_context.bytes(tx_hash))
}

/// Combines [`hash_transaction()`] and [`sign()`] and returns [`Seal`]
// TODO: `no_panic` can't prove lack of panics in `hash_transaction()` and `sign()`
// #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
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
// TODO: `no_panic` can't prove lack of panics in `schnorrkel` verification
// #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn verify(
    public_key: &PublicKey,
    expected_nonce: u64,
    tx_hash: &[u8; OUT_LEN],
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

/// Combines [`hash_transaction()`] and [`verify()`]
// TODO: `no_panic` can't prove lack of panics in `hash_transaction()` and `schnorrkel` verification
// #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
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
