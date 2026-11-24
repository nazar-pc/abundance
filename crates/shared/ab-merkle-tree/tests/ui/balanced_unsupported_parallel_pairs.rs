//! Balanced Merkle Tree rejects hashers whose number of pairs hashed in parallel is not a power of
//! two or is larger than 64

#![feature(const_default, const_trait_impl, derive_const)]

use ab_merkle_tree::MerkleHasher;
use ab_merkle_tree::balanced::BalancedMerkleTree;

#[derive(Debug, Copy, Clone)]
#[derive_const(Default)]
struct ZeroPairsHasher;

impl MerkleHasher for ZeroPairsHasher {
    const OUT_LEN: usize = 32;
    const PARALLEL_PAIRS: usize = 0;

    fn hash_pair(&self, pair: &[[u8; 32]; 2]) -> [u8; 32] {
        pair[0]
    }
}

#[derive(Debug, Copy, Clone)]
#[derive_const(Default)]
struct ThreePairsHasher;

impl MerkleHasher for ThreePairsHasher {
    const OUT_LEN: usize = 32;
    const PARALLEL_PAIRS: usize = 3;

    fn hash_pair(&self, pair: &[[u8; 32]; 2]) -> [u8; 32] {
        pair[0]
    }
}

#[derive(Debug, Copy, Clone)]
#[derive_const(Default)]
struct ManyPairsHasher;

impl MerkleHasher for ManyPairsHasher {
    const OUT_LEN: usize = 32;
    const PARALLEL_PAIRS: usize = 128;

    fn hash_pair(&self, pair: &[[u8; 32]; 2]) -> [u8; 32] {
        pair[0]
    }
}

fn main() {
    BalancedMerkleTree::<64, ZeroPairsHasher>::compute_root_only(&[[0; 32]; 64]);
    BalancedMerkleTree::<64, ThreePairsHasher>::new(&[[0; 32]; 64]);
    BalancedMerkleTree::<512, ManyPairsHasher>::compute_root_only(&[[0; 32]; 512]);
}
