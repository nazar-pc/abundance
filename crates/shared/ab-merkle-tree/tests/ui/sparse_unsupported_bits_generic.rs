//! Unsupported numbers of bits are rejected in generic code in crates with `gca_const_items`
//! too, which doesn't have to propagate the bound, the check is evaluated by every method instead

#![expect(incomplete_features, reason = "generic_const_*")]
#![feature(
    gca_const_items,
    gca_macroless_args,
    gca_min_const_items,
    generic_const_items
)]

use ab_merkle_tree::blake3::SparseMerkleTree;
use ab_merkle_tree::sparse::PROOF_ELEMENTS;

fn root<const BITS: u8>() -> Option<[u8; 32]> {
    SparseMerkleTree::<BITS>::compute_root_only([])
}

fn verify<const BITS: u8>(proof: &[[u8; 32]; PROOF_ELEMENTS::<BITS>]) -> bool {
    SparseMerkleTree::<BITS>::verify(&[0; 32], proof, 0, [0; 32])
}

fn main() {
    root::<0>();
    root::<129>();
    verify::<130>(&[[0; 32]; 130]);
}
