//! Sparse Merkle Tree and related data structures.
//!
//! Sparse Merkle Tree is essentially a huge Balanced Merkle Tree, where most of the leaves are
//! empty. By "empty" here we mean all zeroes. To optimize proofs and their verification, the
//! hashing function is customized and returns all zeroes when both left and right branch are all
//! zeroes, otherwise inner nodes are hashed like in a Balanced Merkle Tree.

use crate::MerkleHasher;
use crate::batching::{BUFFER_NUM_LEAVES, PARALLEL_PAIRS, hash_buffered_leaves};
use core::marker::PhantomCovariant;
use core::mem::MaybeUninit;
use core::num::NonZeroU128;

/// Number of elements in a proof for a tree with `BITS`
pub const PROOF_ELEMENTS<const BITS: u8>: usize = usize::from(BITS);
const STACK_SIZE<const BITS: u8>: usize = usize::from(BITS) + 1;

/// Ensuring only supported `NUM_BITS` can be specified for [`SparseMerkleTree`].
///
/// This is essentially a workaround for the current Rust type system constraints that do not allow
/// a nicer way to do the same thing at compile time.
const ENSURE_SUPPORTED_BITS<const BITS: u8>: usize = {
    assert!(
        BITS <= 128,
        "This Sparse Merkle Tree doesn't support more than 2^128 leaves"
    );

    assert!(
        BITS != 0,
        "This Sparse Merkle Tree must have more than one leaf"
    );

    0
};

/// Sparse Merkle Tree Leaf
#[derive(Debug)]
pub enum Leaf<'a, H>
where
    H: MerkleHasher,
{
    /// Leaf contains a value
    Occupied {
        /// Leaf value
        leaf: &'a [u8; H::OUT_LEN],
    },
    /// Leaf contains a value (owned)
    OccupiedOwned {
        /// Leaf value
        leaf: [u8; H::OUT_LEN],
    },
    /// Leaf is empty
    Empty {
        /// Number of consecutive empty leaves
        skip_count: NonZeroU128,
    },
}

impl<'a, H> From<&'a [u8; H::OUT_LEN]> for Leaf<'a, H>
where
    H: MerkleHasher,
{
    #[inline(always)]
    fn from(leaf: &'a [u8; H::OUT_LEN]) -> Self {
        Self::Occupied { leaf }
    }
}

// TODO: A version that can hold intermediate nodes in memory, efficiently update leaves, etc.
/// Sparse Merkle Tree variant that has hash-sized leaves, with most leaves being empty
/// (all zeroes).
///
/// In contrast to a proper Balanced Merkle Tree, constant `BITS` here specifies the max number of
/// leaves hypothetically possible in a tree (2^BITS, often intractable), rather than the number of
/// non-empty leaves actually present.
///
/// Inner nodes are hashed with `H` (except when both children are empty), which is created with
/// [`Default`] since the number of leaves is always `2^BITS`.
#[derive(Debug)]
pub struct SparseMerkleTree<const BITS: u8, H>(PhantomCovariant<H>);

// TODO: Isolated occupied leaves and merges with empty subtrees in `skip_leaves()` are still hashed
//  one pair at a time, paths of different leaves could be hashed in parallel with
//  `MerkleHasher::hash_pairs()`
impl<const BITS: u8, H> SparseMerkleTree<BITS, H>
where
    [(); ENSURE_SUPPORTED_BITS::<BITS>]:,
    H: MerkleHasher + Default,
{
    const ZERO: [u8; H::OUT_LEN] = [0; H::OUT_LEN];

    // TODO: Method that generates not only root, but also proof, like Unbalanced Merkle Tree
    /// Compute Merkle Tree root.
    ///
    /// If provided iterator ends early, it means the rest of the leaves are empty.
    ///
    /// There must be no [`Leaf::Occupied`] for empty/unoccupied leaves or else they may result in
    /// invalid root, [`Leaf::Empty`] must be used instead.
    ///
    /// Returns `None` if too many leaves were provided.
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn compute_root_only<'a, Iter>(leaves: Iter) -> Option<[u8; H::OUT_LEN]>
    where
        H: 'a,
        Iter: IntoIterator<Item = Leaf<'a, H>> + 'a,
    {
        // TODO: Technically redundant with the where-clause, which is not enforced in generic code
        //  due to https://github.com/rust-lang/rust/issues/164019, remove once fixed
        const { ENSURE_SUPPORTED_BITS::<BITS> };

        let hasher = H::default();

        // Stack of intermediate nodes per tree level
        let mut stack = [[0u8; H::OUT_LEN]; _];
        let mut processed_some = false;
        let mut num_leaves = 0u128;

        if PARALLEL_PAIRS::<H> == 1 {
            // Hashers that hash pairs one by one push each leaf into the stack right away, a loop
            // doesn't need a frame for a closure like internal iteration below
            for leaf in leaves {
                if u32::from(BITS) < u128::BITS {
                    // How many leaves were processed so far. Should have been
                    // `num_leaves == 2^BITS`, but `>=` helps compiler with panic safety checks.
                    // TODO: Use `==` once
                    //  https://github.com/rust-lang/rust/issues/162834 is resolved
                    if num_leaves >= 2u128.pow(u32::from(BITS)) {
                        return None;
                    }
                } else {
                    // For `BITS == u128::BITS` `num_leaves` will wrap around back to zero right at
                    // the very end
                    if processed_some && num_leaves == 0 {
                        return None;
                    }
                    processed_some = true;
                }

                let mut current = match leaf {
                    Leaf::Occupied { leaf } => *leaf,
                    Leaf::OccupiedOwned { leaf } => leaf,
                    Leaf::Empty { skip_count } => {
                        num_leaves = Self::skip_leaves(
                            &hasher,
                            &mut stack,
                            &mut processed_some,
                            num_leaves,
                            skip_count.get(),
                        )?;
                        continue;
                    }
                };

                // Every bit set to `1` corresponds to an active Merkle Tree level
                let lowest_active_levels = num_leaves.trailing_ones() as usize;
                for item in stack.iter().take(lowest_active_levels) {
                    current = hasher.hash_pair(&[*item, current]);
                }

                // Place the current hash at the first inactive level
                *stack
                    .get_mut(lowest_active_levels)
                    .expect("Stack has `BITS + 1` levels, enough for `num_leaves < 2^BITS`; qed") =
                    current;

                // Wrapping is needed for `BITS == u128::BITS`, where number of leaves narrowly
                // doesn't fit into `u128` itself
                num_leaves = num_leaves.wrapping_add(1);
            }
        } else {
            // Consecutive occupied leaves within a multiple of the buffer size, hashed together
            // once the buffer is full, on the next empty leaf or at the end
            let mut buffer = [MaybeUninit::<[u8; H::OUT_LEN]>::uninit(); BUFFER_NUM_LEAVES::<H>];
            // Index of the first leaf in the buffer, which is empty if it is the index of the next
            // leaf
            let mut buffer_start = 0;

            // Adapters like `FlatMap` implement internal iteration as a loop over each inner
            // iterator, which is much faster than pulling leaves with `Iterator::next()`
            leaves.into_iter().try_for_each(|leaf| {
                if u32::from(BITS) < u128::BITS {
                    // How many leaves were processed so far. Should have been
                    // `num_leaves == 2^BITS`, but `>=` helps compiler with panic safety checks.
                    // TODO: Use `==` once
                    //  https://github.com/rust-lang/rust/issues/162834 is resolved
                    if num_leaves >= 2u128.pow(u32::from(BITS)) {
                        return None;
                    }
                } else {
                    // For `BITS == u128::BITS` `num_leaves` will wrap around back to zero right at
                    // the very end
                    if processed_some && num_leaves == 0 {
                        return None;
                    }
                    processed_some = true;
                }

                let leaf = match leaf {
                    Leaf::Occupied { leaf } => *leaf,
                    Leaf::OccupiedOwned { leaf } => leaf,
                    Leaf::Empty { skip_count } => {
                        let buffer_end = num_leaves as usize % BUFFER_NUM_LEAVES::<H>;
                        if buffer_start != buffer_end {
                            // SAFETY: Leaves were written to the buffer from `buffer_start` up to
                            // the index of the current leaf, which is smaller than `2^BITS` as
                            // checked above
                            unsafe {
                                Self::push_buffered_leaves(
                                    &hasher,
                                    &mut stack,
                                    &mut buffer,
                                    num_leaves,
                                    buffer_start,
                                    buffer_end,
                                );
                            }
                        }
                        num_leaves = Self::skip_leaves(
                            &hasher,
                            &mut stack,
                            &mut processed_some,
                            num_leaves,
                            skip_count.get(),
                        )?;
                        // Truncation keeps the lowest bits, which are the index in the buffer
                        buffer_start = num_leaves as usize % BUFFER_NUM_LEAVES::<H>;
                        return Some(());
                    }
                };

                // Truncation keeps the lowest bits, which are the index in the buffer
                let buffer_index = num_leaves as usize % BUFFER_NUM_LEAVES::<H>;
                buffer
                    .get_mut(buffer_index)
                    .expect("Index is a remainder of division by the buffer size; qed")
                    .write(leaf);

                if buffer_index == BUFFER_NUM_LEAVES::<H> - 1 {
                    // SAFETY: Leaves were written to the buffer from `buffer_start` up to its end,
                    // the current leaf is smaller than `2^BITS` as checked above
                    unsafe {
                        Self::push_buffered_leaves(
                            &hasher,
                            &mut stack,
                            &mut buffer,
                            num_leaves,
                            buffer_start,
                            BUFFER_NUM_LEAVES::<H>,
                        );
                    }
                    buffer_start = 0;
                }

                // Wrapping is needed for `BITS == u128::BITS`, where number of leaves narrowly
                // doesn't fit into `u128` itself
                num_leaves = num_leaves.wrapping_add(1);
                Some(())
            })?;

            let buffer_end = num_leaves as usize % BUFFER_NUM_LEAVES::<H>;
            if buffer_start != buffer_end {
                // SAFETY: Leaves were written to the buffer from `buffer_start` up to the index of
                // the last leaf, which was smaller than `2^BITS`. The buffer is not full, so
                // `num_leaves` didn't wrap around to zero.
                unsafe {
                    Self::push_buffered_leaves(
                        &hasher,
                        &mut stack,
                        &mut buffer,
                        num_leaves,
                        buffer_start,
                        buffer_end,
                    );
                }
            }
        }

        if u32::from(BITS) < u128::BITS {
            Self::skip_leaves(
                &hasher,
                &mut stack,
                &mut processed_some,
                num_leaves,
                2u128.pow(u32::from(BITS)) - num_leaves,
            )?;
        } else if processed_some && num_leaves != 0 {
            // For `BITS == u128::BITS` `num_leaves` will wrap around back to zero right at the
            // very end, so we reverse the mechanism here
            Self::skip_leaves(
                &hasher,
                &mut stack,
                &mut processed_some,
                num_leaves,
                0u128.wrapping_sub(num_leaves),
            )?;
        }

        Some(stack[BITS as usize])
    }

    /// Push the root of a subtree of `2^height` leaves that starts at leaf `first_leaf`, merging it
    /// with the nodes on the stack it completes.
    ///
    /// # Safety
    /// `height` must not be larger than `BITS` and `first_leaf` must be smaller than `2^BITS`.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    unsafe fn push_subtree(
        hasher: &H,
        stack: &mut [[u8; H::OUT_LEN]; STACK_SIZE::<BITS>],
        first_leaf: u128,
        height: u32,
        mut node: [u8; H::OUT_LEN],
    ) {
        // Every bit set to `1` corresponds to an active Merkle Tree level
        let active_levels = (first_leaf >> height).trailing_ones() as usize;
        for item in stack.iter().skip(height as usize).take(active_levels) {
            node = hasher.hash_pair(&[*item, node]);
        }

        // Place the node at the first inactive level.
        //
        // SAFETY: `height <= BITS` and `first_leaf < 2^BITS`, so the level is at most `BITS`,
        // while the stack has `BITS + 1` levels
        *unsafe { stack.get_unchecked_mut(height as usize + active_levels) } = node;
    }

    /// Push leaves `buffer[start..end]` as perfect subtrees, `leaf_index` is the index of any
    /// leaf within the same multiple of the buffer size.
    ///
    /// # Safety
    /// `end` must not be larger than the buffer size, leaves `buffer[start..end]` must be
    /// initialized and the leaf corresponding to `buffer[end - 1]` must be smaller than `2^BITS`.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    unsafe fn push_buffered_leaves(
        hasher: &H,
        stack: &mut [[u8; H::OUT_LEN]; STACK_SIZE::<BITS>],
        buffer: &mut [MaybeUninit<[u8; H::OUT_LEN]>; BUFFER_NUM_LEAVES::<H>],
        leaf_index: u128,
        start: usize,
        end: usize,
    ) {
        // Truncation keeps the lowest bits, which are the index in the buffer
        let first_leaf = leaf_index - (leaf_index as usize % BUFFER_NUM_LEAVES::<H>) as u128;
        // SAFETY: Guaranteed by the function contract. Subtrees are within the buffer, which is
        // not larger than the tree, so `height <= BITS`, and they start at leaves
        // `first_leaf + offset < 2^BITS`, as `Self::push_subtree()` requires.
        unsafe {
            hash_buffered_leaves(hasher, buffer, start, end, |offset, height, root| {
                Self::push_subtree(hasher, stack, first_leaf + offset as u128, height, root);
            });
        }
    }

    /// Returns updated number of leaves
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn skip_leaves(
        hasher: &H,
        stack: &mut [[u8; H::OUT_LEN]; STACK_SIZE::<BITS>],
        processed_some: &mut bool,
        mut num_leaves: u128,
        mut skip_count: u128,
    ) -> Option<u128> {
        if u32::from(BITS) < u128::BITS {
            // How many leaves were processed so far
            if num_leaves.checked_add(skip_count)? > 2u128.pow(u32::from(BITS)) {
                return None;
            }
        } else {
            // For `BITS == u128::BITS` `num_leaves` will wrap around back to zero right at the
            // very end
            let (overflow_amount, overflowed) = num_leaves.overflowing_add(skip_count);
            if *processed_some && overflowed && overflow_amount > 0 {
                return None;
            }
            *processed_some = true;
        }

        while skip_count > 0 {
            // Find the largest aligned chunk to skip for the current state of the tree
            let max_levels_to_skip = skip_count.ilog2().min(num_leaves.trailing_zeros());
            let chunk_size = 1u128 << max_levels_to_skip;

            let mut level = max_levels_to_skip;
            let mut current = Self::ZERO;
            for item in stack.iter().skip(max_levels_to_skip as usize) {
                // Check the active level for merging up the stack.
                //
                // `BITS == u128::BITS` condition is only added for better dead code elimination
                // since that check is only relevant for 2^128 leaves case and nothing else.
                if (u32::from(BITS) == u128::BITS && level == u128::BITS)
                    || num_leaves & (1 << level) == 0
                {
                    // Level wasn't active before, stop here
                    break;
                }

                // Hash together unless both are zero
                if !(item == &Self::ZERO && current == Self::ZERO) {
                    current = hasher.hash_pair(&[*item, current]);
                }

                level += 1;
            }
            // SAFETY: Level is limited by the number of leaves, which in turn is checked above to
            // fit into 2^BITS, while `BITS` generic in turn ensured sufficient stack size
            *unsafe { stack.get_unchecked_mut(level as usize) } = current;

            // Wrapping is needed for `BITS == u128::BITS`, where number of leaves narrowly
            // doesn't fit into `u128` itself
            num_leaves = num_leaves.wrapping_add(chunk_size);
            skip_count -= chunk_size;
        }

        Some(num_leaves)
    }

    /// Verify previously generated proof.
    ///
    /// Leaf can either be leaf value for a leaf that is occupied or all zeroes for a leaf that is
    /// supposed to be empty.
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn verify(
        root: &[u8; H::OUT_LEN],
        proof: &[[u8; H::OUT_LEN]; PROOF_ELEMENTS::<BITS>],
        leaf_index: u128,
        leaf: [u8; H::OUT_LEN],
    ) -> bool {
        // TODO: Technically redundant with the where-clause, which is not enforced in generic code
        //  due to https://github.com/rust-lang/rust/issues/164019, remove once fixed
        const { ENSURE_SUPPORTED_BITS::<BITS> };

        // For `BITS == u128::BITS` any index is valid by definition
        if u32::from(BITS) < u128::BITS && leaf_index >= 2u128.pow(u32::from(BITS)) {
            return false;
        }

        let hasher = H::default();
        let mut computed_root = leaf;

        let mut position = leaf_index;
        for hash in proof {
            // Hash together unless both are zero
            if !(computed_root == Self::ZERO && hash == &Self::ZERO) {
                computed_root = if position.is_multiple_of(2) {
                    hasher.hash_pair(&[computed_root, *hash])
                } else {
                    hasher.hash_pair(&[*hash, computed_root])
                };
            }

            position /= 2;
        }

        root == &computed_root
    }
}
