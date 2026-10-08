//! The root is returned without `Option`, which only works for a non-empty array

use ab_merkle_tree::unbalanced::UnbalancedMerkleTree;

fn main() {
    UnbalancedMerkleTree::compute_root_only_array::<0, [u8; 32]>(&[]);
}
