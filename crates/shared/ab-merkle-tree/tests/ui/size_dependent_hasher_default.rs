//! Hashers that implement `Default` don't depend on the tree size, so they can't implement
//! `NewMerkleHasherForTreeSize` themselves

#![feature(const_trait_impl)]

use ab_merkle_tree::NewMerkleHasherForTreeSize;

#[derive(Debug, Default)]
struct TreeSizeHasher;

const impl NewMerkleHasherForTreeSize for TreeSizeHasher {
    fn new_for_tree_size(_num_leaves: u64) -> Self {
        Self
    }
}

fn main() {}
