use ab_contracts_common::ContractError;
use ab_core_primitives::address::Address;
use ab_core_primitives::block::BlockRoot;
use ab_core_primitives::transaction::owned::OwnedTransaction;
use ab_core_primitives::transaction::{Gas, TransactionHeader, TransactionSlot};
use ab_io_type::trivial_type::TrivialType;
use ab_system_contract_simple_wallet_base::Seal;
use ab_system_contract_simple_wallet_base::seal::{hash_and_verify, hash_transaction};
use schnorrkel::{ExpansionMode, MiniSecretKey};

const NONCE: u64 = 5;
const SEAL: Seal = Seal {
    nonce: NONCE,
    signature: [1; 64],
};

#[test]
fn hash_transaction_is_hash_of_serialized_transaction_without_signature() {
    let header = TransactionHeader {
        version: TransactionHeader::TRANSACTION_VERSION,
        block_root: BlockRoot::default(),
        gas_limit: Gas::default(),
        contract: Address::SYSTEM_SIMPLE_WALLET_BASE,
    };
    let slot = TransactionSlot {
        owner: Address::from(100),
        contract: Address::SYSTEM_STATE,
    };
    let slot_as_payload = [u128::from(slot.owner), u128::from(slot.contract)];
    let transactions = [
        OwnedTransaction::from_parts(&header, &[], &[], &[], SEAL.as_bytes()).unwrap(),
        OwnedTransaction::from_parts(&header, &[slot], &[], &[], SEAL.as_bytes()).unwrap(),
        OwnedTransaction::from_parts(&header, &[], &[slot], &[], SEAL.as_bytes()).unwrap(),
        OwnedTransaction::from_parts(&header, &[], &[], &slot_as_payload, SEAL.as_bytes()).unwrap(),
    ];

    let mut hashes = Vec::new();
    for owned_transaction in &transactions {
        let bytes = owned_transaction.buffer().as_slice();
        let (signed_bytes, signature) = bytes.split_at(bytes.len() - size_of_val(&SEAL.signature));
        assert_eq!(signature, SEAL.signature);

        let transaction = owned_transaction.transaction();
        let hash = hash_transaction(
            transaction.header,
            transaction.read_slots,
            transaction.write_slots,
            transaction.payload,
            NONCE,
        )
        .unwrap();
        assert_eq!(hash, *blake3::hash(signed_bytes).as_bytes());
        hashes.push(hash);
    }

    // The same slot as a read slot, as a write slot and as the payload gives different hashes
    assert_ne!(hashes[1], hashes[2]);
    assert_ne!(hashes[1], hashes[3]);
    assert_ne!(hashes[2], hashes[3]);
}

#[test]
fn transactions_with_too_many_slots_are_rejected() {
    let header = TransactionHeader {
        version: TransactionHeader::TRANSACTION_VERSION,
        block_root: BlockRoot::default(),
        gas_limit: Gas::default(),
        contract: Address::SYSTEM_SIMPLE_WALLET_BASE,
    };
    let slot = TransactionSlot {
        owner: Address::from(100),
        contract: Address::SYSTEM_STATE,
    };
    let slots = vec![slot; usize::from(u16::MAX) + 1];
    let keypair = MiniSecretKey::from_bytes(&[1; 32])
        .unwrap()
        .expand_to_keypair(ExpansionMode::Ed25519);

    assert_eq!(hash_transaction(&header, &slots, &[], &[], NONCE), None);
    assert_eq!(hash_transaction(&header, &[], &slots, &[], NONCE), None);
    assert_eq!(
        hash_and_verify(&keypair.public, NONCE, &header, &slots, &[], &[], &SEAL),
        Err(ContractError::BadInput)
    );
    assert_eq!(
        hash_and_verify(&keypair.public, NONCE, &header, &[], &slots, &[], &SEAL),
        Err(ContractError::BadInput)
    );
}
