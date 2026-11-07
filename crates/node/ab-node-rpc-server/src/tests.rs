use crate::{BlockSignatureSenders, FarmerRpcApiError};
use ab_core_primitives::block::header::{BlockHeaderEd25519Seal, OwnedBlockHeaderSeal};
use ab_core_primitives::ed25519::{Ed25519PublicKey, Ed25519Signature};
use ab_core_primitives::hashes::Blake3Hash;
use ab_farmer_rpc_primitives::BlockSealResponse;
use ed25519_dalek::{Signer, SigningKey};
use futures::channel::oneshot;
use std::assert_matches;

fn seal(signing_key: &SigningKey, pre_seal_hash: &Blake3Hash) -> OwnedBlockHeaderSeal {
    OwnedBlockHeaderSeal::Ed25519(BlockHeaderEd25519Seal {
        public_key: Ed25519PublicKey::from(signing_key.verifying_key()),
        signature: Ed25519Signature::from(signing_key.sign(pre_seal_hash.as_bytes())),
    })
}

/// Senders waiting for a seal of `signing_key` for `pre_seal_hash`
fn waiting_senders(
    signing_key: &SigningKey,
    pre_seal_hash: Blake3Hash,
) -> (
    BlockSignatureSenders,
    oneshot::Receiver<OwnedBlockHeaderSeal>,
) {
    let (seal_sender, seal_receiver) = oneshot::channel();
    let senders = BlockSignatureSenders {
        current_pre_seal_hash: pre_seal_hash,
        current_public_key_hash: Ed25519PublicKey::from(signing_key.verifying_key()).hash(),
        senders: vec![seal_sender],
    };

    (senders, seal_receiver)
}

#[test]
fn valid_seal_is_delivered() {
    let signing_key = SigningKey::from([1; _]);
    let pre_seal_hash = Blake3Hash::new([2; _]);
    let (mut senders, mut seal_receiver) = waiting_senders(&signing_key, pre_seal_hash);
    let seal = seal(&signing_key, &pre_seal_hash);

    senders
        .submit_seal(BlockSealResponse {
            pre_seal_hash,
            seal,
        })
        .unwrap();

    let received_seal = seal_receiver.try_recv().unwrap().unwrap();
    assert_eq!(
        received_seal.as_ref().public_key_hash(),
        seal.as_ref().public_key_hash()
    );
    assert!(received_seal.as_ref().is_seal_valid(&pre_seal_hash));
    assert!(senders.senders.is_empty());
}

#[test]
fn seal_of_another_public_key_is_rejected() {
    let signing_key = SigningKey::from([1; _]);
    let pre_seal_hash = Blake3Hash::new([2; _]);
    let (mut senders, mut seal_receiver) = waiting_senders(&signing_key, pre_seal_hash);
    // Valid signature, but not of the farmer that created the solution
    let other_signing_key = SigningKey::from([3; _]);
    let other_public_key_hash = Ed25519PublicKey::from(other_signing_key.verifying_key()).hash();

    assert_matches!(
        senders.submit_seal(BlockSealResponse {
            pre_seal_hash,
            seal: seal(&other_signing_key, &pre_seal_hash),
        }),
        Err(FarmerRpcApiError::InvalidBlockSeal {
            pre_seal_hash: error_pre_seal_hash,
            public_key_hash,
        }) if error_pre_seal_hash == pre_seal_hash && public_key_hash == other_public_key_hash
    );
    assert_eq!(senders.senders.len(), 1);

    // A valid seal still completes the block
    senders
        .submit_seal(BlockSealResponse {
            pre_seal_hash,
            seal: seal(&signing_key, &pre_seal_hash),
        })
        .unwrap();
    assert_matches!(seal_receiver.try_recv(), Ok(Some(_)));
}

#[test]
fn seal_with_invalid_signature_is_rejected() {
    let signing_key = SigningKey::from([1; _]);
    let pre_seal_hash = Blake3Hash::new([2; _]);
    let (mut senders, mut seal_receiver) = waiting_senders(&signing_key, pre_seal_hash);

    // The expected public key, but signature of another pre-seal hash
    assert_matches!(
        senders.submit_seal(BlockSealResponse {
            pre_seal_hash,
            seal: seal(&signing_key, &Blake3Hash::new([3; _])),
        }),
        Err(FarmerRpcApiError::InvalidBlockSeal {
            pre_seal_hash: error_pre_seal_hash,
            public_key_hash,
        }) if error_pre_seal_hash == pre_seal_hash
            && public_key_hash == senders.current_public_key_hash
    );
    assert_eq!(senders.senders.len(), 1);

    // A valid seal still completes the block
    senders
        .submit_seal(BlockSealResponse {
            pre_seal_hash,
            seal: seal(&signing_key, &pre_seal_hash),
        })
        .unwrap();
    assert_matches!(seal_receiver.try_recv(), Ok(Some(_)));
}

#[test]
fn seal_for_another_pre_seal_hash_is_ignored() {
    let signing_key = SigningKey::from([1; _]);
    let pre_seal_hash = Blake3Hash::new([2; _]);
    let (mut senders, mut seal_receiver) = waiting_senders(&signing_key, pre_seal_hash);
    let other_pre_seal_hash = Blake3Hash::new([3; _]);

    senders
        .submit_seal(BlockSealResponse {
            pre_seal_hash: other_pre_seal_hash,
            seal: seal(&signing_key, &other_pre_seal_hash),
        })
        .unwrap();
    assert_eq!(senders.senders.len(), 1);
    assert_matches!(seal_receiver.try_recv(), Ok(None));
}
