//! `MAX_N` below 2 results in no space for peaks, which `MerkleMountainRange::peaks()` writes
//! without bounds checks

use ab_merkle_tree::mmr::MerkleMountainRange;

fn main() {
    MerkleMountainRange::<0>::new();
    MerkleMountainRange::<1>::new();
}
