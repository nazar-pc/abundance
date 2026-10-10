//! Unsupported numbers of bits are rejected in generic code in crates with `generic_const_args`
//! too, which doesn't have to propagate the bound, the check is evaluated by every method instead

#![expect(incomplete_features, reason = "generic_const_*")]
#![feature(
    generic_const_args,
    generic_const_items,
    macroless_generic_const_args,
    min_generic_const_args
)]

use ab_merkle_tree::sparse::{PROOF_ELEMENTS, SparseMerkleTree};

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
