//! Same as `mmr_unsupported_max_n_generic.rs`, but for `MerkleMountainRange::from_peaks()`, which
//! needs a separate file since each of the two unsupported `MAX_N` values is only reported once

#![expect(incomplete_features, reason = "generic_const_*")]
#![feature(
    generic_const_args,
    generic_const_items,
    macroless_generic_const_args,
    min_generic_const_args
)]

use ab_merkle_tree::mmr::{MerkleMountainRange, MmrPeaks};

fn from_peaks<const MAX_N: u64>(peaks: &MmrPeaks<MAX_N>) -> Option<u64> {
    MerkleMountainRange::<MAX_N>::from_peaks(peaks).map(|mmr| mmr.num_leaves())
}

fn main() {
    from_peaks::<0>(&MmrPeaks {
        num_leaves: 0,
        peaks: [],
    });
}
