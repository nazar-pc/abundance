//! Utilities for [`Seal`] creation and verification

use crate::{SIGNING_CONTEXT, Seal};
use ab_contracts_common::ContractError;
use ab_core_primitives::transaction::{
    SerializedTransactionLengths, TransactionHeader, TransactionSlot,
};
use ab_io_type::trivial_type::TrivialType;
use blake3::{Hasher, OUT_LEN};
use core::mem::offset_of;
use schnorrkel::context::SigningContext;
use schnorrkel::{Keypair, PublicKey, Signature};

/// Create transaction hash used for signing with [`sign()`].
///
/// The hash input is the serialized transaction (see [`Transaction::try_from_bytes()`]) with a
/// [`Seal`] containing `nonce`, without the signature at the end of the seal.
///
/// [`hash_and_sign()`] helper function exists that combines this method with [`sign()`].
///
/// Returns `None` if there are more slots or payload bytes than [`SerializedTransactionLengths`]
/// can describe, in which case it is not a transaction.
///
/// [`Transaction::try_from_bytes()`]: ab_core_primitives::transaction::Transaction::try_from_bytes
// TODO: `no_panic` can't prove lack of panics in `blake3::Hasher` for input of arbitrary length
// #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn hash_transaction(
    header: &TransactionHeader,
    read_slots: &[TransactionSlot],
    write_slots: &[TransactionSlot],
    payload: &[u128],
    nonce: u64,
) -> Option<[u8; OUT_LEN]> {
    const {
        // Everything in the seal except the signature is signed, so the signature must be last
        assert!(offset_of!(Seal, signature) + size_of::<[u8; 64]>() == size_of::<Seal>());
    }

    let lengths = SerializedTransactionLengths {
        read_slots: u16::try_from(read_slots.len()).ok()?,
        write_slots: u16::try_from(write_slots.len()).ok()?,
        payload: u32::try_from(size_of_val(payload)).ok()?,
        seal: Seal::SIZE,
        padding: [0; _],
    };

    let mut hasher = Hasher::new();
    hasher.update(header.as_bytes());
    hasher.update(lengths.as_bytes());
    hasher.update(TransactionSlot::slice_as_bytes(read_slots));
    hasher.update(TransactionSlot::slice_as_bytes(write_slots));
    hasher.update(u128::slice_as_bytes(payload));
    let seal = Seal {
        nonce,
        signature: [0; _],
    };
    hasher.update(&seal.as_bytes()[..offset_of!(Seal, signature)]);
    Some(hasher.finalize().into())
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

/// Combines [`hash_transaction()`] and [`sign()`] and returns [`Seal`].
///
/// Returns `None` if [`hash_transaction()`] does.
// TODO: `no_panic` can't prove lack of panics in `hash_transaction()` and `sign()`
// #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn hash_and_sign(
    keypair: &Keypair,
    header: &TransactionHeader,
    read_slots: &[TransactionSlot],
    write_slots: &[TransactionSlot],
    payload: &[u128],
    nonce: u64,
) -> Option<Seal> {
    let tx_hash = hash_transaction(header, read_slots, write_slots, payload, nonce)?;
    let signature = sign(keypair, &tx_hash).to_bytes();

    Some(Seal { nonce, signature })
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

/// Combines [`hash_transaction()`] and [`verify()`].
///
/// Returns [`ContractError::BadInput`] if [`hash_transaction()`] returns `None`.
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

    let tx_hash = hash_transaction(header, read_slots, write_slots, payload, seal.nonce)
        .ok_or(ContractError::BadInput)?;
    let signature =
        Signature::from_bytes(&seal.signature).map_err(|_error| ContractError::BadInput)?;
    let signing_context = SigningContext::new(SIGNING_CONTEXT);
    public_key
        .verify(signing_context.bytes(tx_hash.as_bytes()), &signature)
        .map_err(|_error| ContractError::Forbidden)
}
