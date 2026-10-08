//! `MAX_N` below 2 is rejected in generic code in crates with `generic_const_args` too, which
//! doesn't have to propagate the bound, the check is evaluated by constructors and associated
//! functions instead

#![expect(incomplete_features, reason = "generic_const_*")]
#![feature(
    generic_const_args,
    generic_const_items,
    macroless_generic_const_args,
    min_generic_const_args
)]

use ab_merkle_tree::mmr::MerkleMountainRange;

fn num_leaves<const MAX_N: u64>() -> u64 {
    MerkleMountainRange::<MAX_N>::new().num_leaves()
}

fn verify<const MAX_N: u64>(proof: &[[u8; 32]]) -> bool {
    MerkleMountainRange::<MAX_N>::verify(&[0; 32], proof, 0, [0; 32], 1)
}

fn main() {
    num_leaves::<0>();
    verify::<1>(&[]);
}
