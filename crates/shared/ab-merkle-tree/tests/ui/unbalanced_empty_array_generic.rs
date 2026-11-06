//! Empty arrays are rejected in generic code in crates with `gca_const_items` too, which doesn't
//! have to propagate the bound, the check is evaluated when the function is instantiated instead

#![expect(incomplete_features, reason = "generic_const_*")]
#![feature(
    gca_const_items,
    gca_macroless_args,
    gca_min_const_items,
    generic_const_items
)]

use ab_merkle_tree::blake3::UnbalancedMerkleTree;

fn root<const N: usize>(leaves: &[[u8; 32]; N]) -> [u8; 32] {
    UnbalancedMerkleTree::compute_root_only_array(leaves)
}

fn main() {
    root::<0>(&[]);
}
