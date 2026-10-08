//! Methods that don't construct the tree must reject unsupported numbers of leaves too

use ab_merkle_tree::balanced::BalancedMerkleTree;

fn main() {
    // Not a power of two
    BalancedMerkleTree::compute_root_only(&[[0; 32]; 48]);
    // Less than two leaves
    BalancedMerkleTree::compute_root_only(&[[0; 32]; 1]);
    BalancedMerkleTree::<3>::verify(&[0; 32], &[[0; 32]; 1], 0, [0; 32]);
}
