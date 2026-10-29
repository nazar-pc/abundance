//! Merkle Mountain Range, Sparse Merkle Tree and Unbalanced Merkle Tree methods for iterators that
//! are not `TrustedLen` don't support hashers that depend on the tree size

#![feature(const_trait_impl)]

use ab_merkle_tree::mmr::{MerkleMountainRange, MmrPeaks};
use ab_merkle_tree::sparse::SparseMerkleTree;
use ab_merkle_tree::unbalanced::UnbalancedMerkleTree;
use ab_merkle_tree::{MerkleHasher, NewMerkleHasherForTreeSize};
use std::mem::MaybeUninit;

#[derive(Debug, Copy, Clone)]
struct TreeSizeHasher(u64);

const impl NewMerkleHasherForTreeSize for TreeSizeHasher {
    fn new_for_tree_size(num_leaves: u64) -> Self {
        Self(num_leaves)
    }
}

impl MerkleHasher for TreeSizeHasher {
    const OUT_LEN: usize = 32;

    fn hash_pair(&self, pair: &[[u8; 32]; 2]) -> [u8; 32] {
        let Self(num_leaves) = self;
        let [mut left, _right] = *pair;
        left[0] ^= *num_leaves as u8;
        left
    }
}

fn main() {
    MerkleMountainRange::<4, TreeSizeHasher>::new();
    MerkleMountainRange::<4, TreeSizeHasher>::default();
    MerkleMountainRange::<4, TreeSizeHasher>::from_peaks(&MmrPeaks {
        num_leaves: 0,
        peaks: [[0; 32]; 2],
    });
    MerkleMountainRange::<4, TreeSizeHasher>::verify(&[0; 32], &[], 0, [0; 32], 1);

    SparseMerkleTree::<4, TreeSizeHasher>::compute_root_only([]);
    SparseMerkleTree::<4, TreeSizeHasher>::verify(&[0; 32], &[[0; 32]; 4], 0, [0; 32]);

    UnbalancedMerkleTree::<TreeSizeHasher>::compute_root_only_dynamic_len::<4, _, _>([[0; 32]]);
    UnbalancedMerkleTree::<TreeSizeHasher>::compute_root_and_proof_dynamic_len_in::<4, _, _>(
        [[0; 32]],
        0,
        &mut [MaybeUninit::uninit(); 2],
    );
}
