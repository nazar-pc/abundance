use crate::state::GlobalState;
use ab_aligned_buffer::SharedAlignedBuffer;
use ab_client_api::ContractSlotState;
use ab_core_primitives::address::Address;
use ab_core_primitives::trees::{StateOwnerContractsSparseMerkleTree, StateOwnersSparseMerkleTree};
use ab_merkle_tree::MerkleHasher;
use ab_merkle_tree::sparse::Leaf;
use std::num::NonZeroU128;

fn slot(owner: u128, contract: u128, contents: &[u8]) -> ContractSlotState {
    ContractSlotState {
        owner: Address::from(owner),
        contract: Address::from(contract),
        contents: SharedAlignedBuffer::from_bytes(contents),
    }
}

fn contents_hash(contents: &[u8]) -> [u8; 32] {
    *blake3::hash(contents).as_bytes()
}

fn empty<'a, H>(skip_count: u128) -> Leaf<'a, H>
where
    H: MerkleHasher,
{
    Leaf::Empty {
        skip_count: NonZeroU128::new(skip_count).unwrap(),
    }
}

#[test]
#[cfg_attr(miri, ignore)]
fn single_slot() {
    let state = GlobalState::new(&[slot(5, 7, b"a")]);

    let owner_root = StateOwnerContractsSparseMerkleTree::compute_root_only([
        empty(7),
        Leaf::OccupiedOwned {
            leaf: contents_hash(b"a"),
        },
    ])
    .unwrap();
    // A lone leaf has only empty siblings
    assert!(StateOwnerContractsSparseMerkleTree::verify(
        &owner_root,
        &[[0; 32]; _],
        7,
        contents_hash(b"a"),
    ));

    let state_root = state.root();
    assert_eq!(
        *state_root,
        StateOwnersSparseMerkleTree::compute_root_only([
            empty(5),
            Leaf::OccupiedOwned { leaf: owner_root },
        ])
        .unwrap()
    );
    assert!(StateOwnersSparseMerkleTree::verify(
        &state_root,
        &[[0; 32]; _],
        5,
        owner_root,
    ));
}

#[test]
#[cfg_attr(miri, ignore)]
fn adjacent_and_sparse_slots() {
    let state = GlobalState::new(&[
        slot(0, 0, b"a"),
        slot(0, 2, b"b"),
        slot(1, 1, b"c"),
        slot(3, 5, b"d"),
        slot(3, 6, b"e"),
    ]);

    let owner_0_root = StateOwnerContractsSparseMerkleTree::compute_root_only([
        Leaf::OccupiedOwned {
            leaf: contents_hash(b"a"),
        },
        empty(1),
        Leaf::OccupiedOwned {
            leaf: contents_hash(b"b"),
        },
    ])
    .unwrap();
    let owner_1_root = StateOwnerContractsSparseMerkleTree::compute_root_only([
        empty(1),
        Leaf::OccupiedOwned {
            leaf: contents_hash(b"c"),
        },
    ])
    .unwrap();
    let owner_3_root = StateOwnerContractsSparseMerkleTree::compute_root_only([
        empty(5),
        Leaf::OccupiedOwned {
            leaf: contents_hash(b"d"),
        },
        Leaf::OccupiedOwned {
            leaf: contents_hash(b"e"),
        },
    ])
    .unwrap();

    assert_eq!(
        *state.root(),
        StateOwnersSparseMerkleTree::compute_root_only([
            Leaf::OccupiedOwned { leaf: owner_0_root },
            Leaf::OccupiedOwned { leaf: owner_1_root },
            empty(1),
            Leaf::OccupiedOwned { leaf: owner_3_root },
        ])
        .unwrap()
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn address_space_edges() {
    let state = GlobalState::new(&[
        slot(0, 0, b"a"),
        slot(0, u128::MAX, b"b"),
        slot(u128::MAX, u128::MAX, b"c"),
    ]);

    let owner_0_root = StateOwnerContractsSparseMerkleTree::compute_root_only([
        Leaf::OccupiedOwned {
            leaf: contents_hash(b"a"),
        },
        empty(u128::MAX - 1),
        Leaf::OccupiedOwned {
            leaf: contents_hash(b"b"),
        },
    ])
    .unwrap();
    let owner_max_root = StateOwnerContractsSparseMerkleTree::compute_root_only([
        empty(u128::MAX),
        Leaf::OccupiedOwned {
            leaf: contents_hash(b"c"),
        },
    ])
    .unwrap();

    assert_eq!(
        *state.root(),
        StateOwnersSparseMerkleTree::compute_root_only([
            Leaf::OccupiedOwned { leaf: owner_0_root },
            empty(u128::MAX - 1),
            Leaf::OccupiedOwned {
                leaf: owner_max_root
            },
        ])
        .unwrap()
    );
}
