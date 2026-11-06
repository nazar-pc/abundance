//! Unsupported numbers of leaves are rejected in generic code in crates with `gca_const_items`
//! too, which doesn't have to propagate the bound, the check is evaluated by every method instead

#![expect(incomplete_features, reason = "generic_const_*")]
#![feature(
    gca_const_items,
    gca_macroless_args,
    gca_min_const_items,
    generic_const_items
)]

use ab_merkle_tree::balanced::PROOF_ELEMENTS;
use ab_merkle_tree::blake3::BalancedMerkleTree;
use std::mem::MaybeUninit;

fn root<const N: usize>(leaves: &[[u8; 32]; N]) -> [u8; 32] {
    BalancedMerkleTree::compute_root_only(leaves)
}

fn verify<const N: usize>(proof: &[[u8; 32]; PROOF_ELEMENTS::<N>]) -> bool {
    BalancedMerkleTree::<N>::verify(&[0; 32], proof, 0, [0; 32])
}

fn new<const N: usize>(leaves: &[[u8; 32]; N]) -> [u8; 32] {
    BalancedMerkleTree::new(leaves).root()
}

// Creating memory to call it with would already fail on the layout of the tree, so the function
// is only instantiated here
fn new_in<'a, const N: usize>() -> for<'b> fn(
    &'b mut MaybeUninit<BalancedMerkleTree<'a, N>>,
    &'a [[u8; 32]; N],
) -> &'b mut BalancedMerkleTree<'a, N> {
    BalancedMerkleTree::new_in
}

fn main() {
    // Not a power of two
    root::<48>(&[[0; 32]; 48]);
    verify::<3>(&[[0; 32]; 1]);
    new::<5>(&[[0; 32]; 5]);
    new_in::<6>();
    // Less than two leaves
    root::<1>(&[[0; 32]; 1]);
}
