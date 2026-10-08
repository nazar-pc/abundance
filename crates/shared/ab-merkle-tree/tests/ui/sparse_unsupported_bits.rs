//! Only 1 to 128 bits are supported, the upper bound also limits the size of the stack, which is
//! accessed without bounds checks

use ab_merkle_tree::sparse::SparseMerkleTree;

fn main() {
    SparseMerkleTree::<0>::compute_root_only([]);
    SparseMerkleTree::<129>::compute_root_only([]);
}
