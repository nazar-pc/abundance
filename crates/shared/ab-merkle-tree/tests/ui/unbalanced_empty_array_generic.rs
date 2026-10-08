//! Empty arrays are rejected in generic code in crates with `generic_const_args` too, which doesn't
//! have to propagate the bound, the check is evaluated when the function is instantiated instead

#![expect(incomplete_features, reason = "generic_const_*")]
#![feature(
    generic_const_args,
    generic_const_items,
    macroless_generic_const_args,
    min_generic_const_args
)]

use ab_merkle_tree::unbalanced::UnbalancedMerkleTree;

fn root<const N: usize>(leaves: &[[u8; 32]; N]) -> [u8; 32] {
    UnbalancedMerkleTree::compute_root_only_array(leaves)
}

fn main() {
    root::<0>(&[]);
}
