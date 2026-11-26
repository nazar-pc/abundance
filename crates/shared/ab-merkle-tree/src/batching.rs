//! Helpers for hashing many pairs of nodes at once

use crate::MerkleHasher;
use core::mem::MaybeUninit;
use core::{hint, mem};

/// [`MerkleHasher::PARALLEL_PAIRS`] that is checked to be supported
pub(crate) const PARALLEL_PAIRS<H: MerkleHasher>: usize = {
    assert!(
        H::PARALLEL_PAIRS.is_power_of_two() && H::PARALLEL_PAIRS <= 64,
        "`MerkleHasher::PARALLEL_PAIRS` must be a power of two that is not larger than 64"
    );

    H::PARALLEL_PAIRS
};
/// Number of leaves of [`MerkleHasher::PARALLEL_PAIRS`] pairs, which are hashed as a batch
pub(crate) const BATCH_NUM_LEAVES<H: MerkleHasher>: usize = PARALLEL_PAIRS::<H> * 2;
/// Number of consecutive leaves that trees buffer to hash them together as perfect subtrees.
///
/// 8 times [`MerkleHasher::PARALLEL_PAIRS`] keeps the extra stack small (4 KiB for BLAKE3 with
/// AVX-512), while a larger buffer barely helps. Hashers without parallelism don't benefit from
/// it, so the buffer is a single leaf for them, which trees don't use.
pub(crate) const BUFFER_NUM_LEAVES<H: MerkleHasher>: usize = if PARALLEL_PAIRS::<H> == 1 {
    1
} else {
    PARALLEL_PAIRS::<H> * 8
};

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

/// Root of a perfect subtree with `nodes` at its lowest level.
///
/// Levels are hashed in place: parents are written to the front of `nodes`, such that no memory
/// is needed beyond the output of a single [`MerkleHasher::hash_pairs()`] call. Contents of `nodes`
/// are meaningless afterward.
///
/// # Safety
/// `nodes.len()` must be a power of two.
#[inline(always)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub(crate) unsafe fn perfect_subtree_root<H>(
    hasher: &H,
    mut nodes: &mut [[u8; H::OUT_LEN]],
) -> [u8; H::OUT_LEN]
where
    H: MerkleHasher,
{
    // Levels with at least a batch of nodes are hashed one batch at a time, parents of each batch
    // take the place of nodes that were hashed already
    while nodes.len() >= BATCH_NUM_LEAVES::<H> {
        let batches = nodes.as_chunks_mut::<{ BATCH_NUM_LEAVES::<H> }>().0;
        for index in 0..batches.len() {
            let parents = hash_pairs::<{ PARALLEL_PAIRS::<H> }, _, _>(hasher, &batches[index]);
            // Parents of two consecutive batches fill the first of them, which was hashed already
            batches[index / 2]
                .as_chunks_mut::<{ PARALLEL_PAIRS::<H> }>()
                .0[index % 2] = parents;
        }
        nodes = nodes.split_at_mut(nodes.len() / 2).0;
    }

    // Levels with fewer nodes than a batch are hashed at once, which is at most
    // `MerkleHasher::PARALLEL_PAIRS` nodes
    while nodes.len() > 1 {
        // Guards always hold for levels that get here, they only keep arms with more nodes than
        // `MerkleHasher::PARALLEL_PAIRS` from being compiled
        match nodes.len() {
            2 => hash_level::<1, 2, _>(hasher, nodes),
            4 if PARALLEL_PAIRS::<H> >= 4 => hash_level::<2, 4, _>(hasher, nodes),
            8 if PARALLEL_PAIRS::<H> >= 8 => hash_level::<4, 8, _>(hasher, nodes),
            16 if PARALLEL_PAIRS::<H> >= 16 => hash_level::<8, 16, _>(hasher, nodes),
            32 if PARALLEL_PAIRS::<H> >= 32 => hash_level::<16, 32, _>(hasher, nodes),
            64 if PARALLEL_PAIRS::<H> >= 64 => hash_level::<32, 64, _>(hasher, nodes),
            // SAFETY: Levels here have a power of two nodes, which is at least 2 and at most
            // `MerkleHasher::PARALLEL_PAIRS`, which in turn is at most 64
            _ => unsafe { hint::unreachable_unchecked() },
        }
        nodes = nodes.split_at_mut(nodes.len() / 2).0;
    }

    // SAFETY: A power of two is not zero, so a single node remains
    *unsafe { nodes.get_unchecked(0) }
}

/// Hash `NUM_NODES` nodes as `NUM_PAIRS` pairs, parents are written to the front of `nodes`
#[inline(always)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
fn hash_level<const NUM_PAIRS: usize, const NUM_NODES: usize, H>(
    hasher: &H,
    nodes: &mut [[u8; H::OUT_LEN]],
) where
    H: MerkleHasher,
{
    if let Some(level) = nodes.as_mut_array::<NUM_NODES>() {
        let parents = hash_pairs::<NUM_PAIRS, NUM_NODES, _>(hasher, level);
        *level
            .first_chunk_mut()
            .expect("There are fewer parents than nodes; qed") = parents;
    }
}

/// Hash leaves `buffer[start..end]` as the fewest perfect subtrees that start at multiples of
/// their sizes, calling `push(offset, height, root)` for each of them from left to right, where
/// `offset` is the index of the first leaf of the subtree in the buffer.
///
/// Contents of `buffer[start..end]` are meaningless afterward, other leaves are not changed.
///
/// # Safety
/// `end` must not be larger than `BUFFER_LEN` and leaves `buffer[start..end]` must be initialized.
#[inline(always)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub(crate) unsafe fn hash_buffered_leaves<const BUFFER_LEN: usize, H, Push>(
    hasher: &H,
    buffer: &mut [MaybeUninit<[u8; H::OUT_LEN]>; BUFFER_LEN],
    mut start: usize,
    end: usize,
    mut push: Push,
) where
    H: MerkleHasher,
    Push: FnMut(usize, u32, [u8; H::OUT_LEN]),
{
    while start < end {
        // The largest subtree that starts at a multiple of its size and ends within the range
        let height = (end - start).ilog2().min(start.trailing_zeros());
        let subtree_end = start + 2usize.pow(height);
        // SAFETY: `start < subtree_end <= end <= BUFFER_LEN` and leaves within the range are
        // initialized
        let leaves = unsafe {
            buffer
                .get_unchecked_mut(start..subtree_end)
                .assume_init_mut()
        };
        // SAFETY: Number of leaves is a power of two
        let root = unsafe { perfect_subtree_root(hasher, leaves) };
        push(start, height, root);
        start = subtree_end;
    }
}
