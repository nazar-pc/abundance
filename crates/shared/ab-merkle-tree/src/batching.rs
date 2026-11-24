//! Helpers for hashing many pairs of nodes at once

use crate::MerkleHasher;
use core::mem;

// TODO: Combine `NUM_PAIRS` and `NUM_LEAVES` into a single generic parameter once compiler is
//  smart enough to deal with it
/// Similar to [`MerkleHasher::hash_pairs()`], but takes pairs as a flat list of nodes.
///
/// # Panics
/// Panics when `NUM_LEAVES != NUM_PAIRS * 2`.
#[inline(always)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub(crate) fn hash_pairs<const NUM_PAIRS: usize, const NUM_LEAVES: usize, H>(
    hasher: &H,
    leaves: &[[u8; H::OUT_LEN]; NUM_LEAVES],
) -> [[u8; H::OUT_LEN]; NUM_PAIRS]
where
    H: MerkleHasher,
{
    // This protects the invariant of the function
    assert_eq!(NUM_LEAVES, NUM_PAIRS * 2);

    // SAFETY: Same size (checked above) and alignment
    let pairs = unsafe {
        mem::transmute::<&[[u8; H::OUT_LEN]; NUM_LEAVES], &[[[u8; H::OUT_LEN]; 2]; NUM_PAIRS]>(
            leaves,
        )
    };
    hasher.hash_pairs(pairs)
}
