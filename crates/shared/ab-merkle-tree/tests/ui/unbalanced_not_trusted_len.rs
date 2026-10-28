//! Unbalanced Merkle Tree methods that don't have `dynamic_len` in their name require `TrustedLen`
//! iterators even with hashers that don't depend on the tree size

use ab_merkle_tree::blake3::UnbalancedMerkleTree;
use std::mem::MaybeUninit;

fn main() {
    let leaves = [[0u8; 32]; 4];

    UnbalancedMerkleTree::compute_root_only::<4, _, _>(leaves.iter().copied().filter_map(Some));
    UnbalancedMerkleTree::compute_root_and_proof_in::<4, _, _>(
        leaves.iter().copied().filter_map(Some),
        0,
        &mut [MaybeUninit::uninit(); 2],
    );
}
