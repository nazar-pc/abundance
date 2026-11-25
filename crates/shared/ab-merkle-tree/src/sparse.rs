//! Sparse Merkle Tree and related data structures.
//!
//! Sparse Merkle Tree is essentially a huge Balanced Merkle Tree, where most of the leaves are
//! empty. By "empty" here we mean all zeroes. To optimize proofs and their verification, the
//! hashing function is customized and returns all zeroes when both left and right branch are all
//! zeroes, otherwise inner nodes are hashed like in a Balanced Merkle Tree.

use crate::MerkleHasher;
use crate::batching::{BUFFER_NUM_LEAVES, PARALLEL_PAIRS, perfect_subtree_root};
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

/// Subtrees of buffered leaves that are lifted together before they are pushed into the stack of
/// [`SparseMerkleTree::compute_root_only()`].
///
/// Each subtree is lifted to its target: the largest subtree that contains it and neither the
/// previous nor the next subtree, below the whole tree. Subtrees with roots of all zeroes and the
/// whole tree are not lifted. Paths of different subtrees are hashed together with
/// [`MerkleHasher::hash_pairs()`].
///
/// Invariants:
/// * subtrees are in order and start at or after `stack_leaves`, leaves between them are empty
/// * `roots[i]` is the root of the subtree of `2^heights[i]` leaves that contains `first_leaves[i]`
/// * `heights[i] <= targets[i]`, a target is capped by the previous subtree when its subtree is
///   added and by the next subtree when that one is added, lanes are only pushed after that or at
///   the end
/// * `len <= MerkleHasher::PARALLEL_PAIRS`
///
/// Two consecutive subtrees are lifted to at most `2^limit` leaves, where `limit` is the highest
/// bit in which their first leaves differ, so they end up in different halves of the subtree of
/// `2^(limit + 1)` leaves that contains both. Lifted subtrees don't overlap, are in order and don't
/// contain other occupied leaves. The same holds for the last subtree pushed into the stack, so
/// they don't start before `stack_leaves`.
struct Lanes<H>
where
    H: MerkleHasher,
{
    /// Root of the subtree of `2^height` leaves that contains the first leaf
    roots: [[u8; H::OUT_LEN]; PARALLEL_PAIRS::<H>],
    first_leaves: [u128; PARALLEL_PAIRS::<H>],
    heights: [u8; PARALLEL_PAIRS::<H>],
    /// Final once the next subtree is added
    targets: [u8; PARALLEL_PAIRS::<H>],
    len: usize,
    /// First leaf of the last subtree added
    previous_leaf: Option<u128>,
    /// Leaves before this one are in the stack
    stack_leaves: u128,
}

impl<H> Lanes<H>
where
    H: MerkleHasher,
{
    const EMPTY: Self = Self {
        roots: [[0; H::OUT_LEN]; _],
        first_leaves: [0; _],
        heights: [0; _],
        targets: [0; _],
        len: 0,
        previous_leaf: None,
        stack_leaves: 0,
    };

    /// Root, first leaf, height and target of each subtree, in order
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn iter(&self) -> impl Iterator<Item = (&[u8; H::OUT_LEN], u128, u8, u8)> {
        self.roots
            .iter()
            .zip(&self.first_leaves)
            .zip(self.heights.iter().zip(&self.targets))
            .take(self.len)
            .map(|((root, &first_leaf), (&height, &target))| (root, first_leaf, height, target))
    }

    /// Root, first leaf, height and target of each subtree, in order
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn iter_mut(&mut self) -> impl Iterator<Item = (&mut [u8; H::OUT_LEN], u128, &mut u8, u8)> {
        self.roots
            .iter_mut()
            .zip(&self.first_leaves)
            .zip(self.heights.iter_mut().zip(&self.targets))
            .take(self.len)
            .map(|((root, &first_leaf), (height, &target))| (root, first_leaf, height, target))
    }

    /// Add a subtree after the subtrees in lanes, which must not be full
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn push(&mut self, root: [u8; H::OUT_LEN], first_leaf: u128, height: u8, target: u8) {
        if let (Some(lane_root), Some(lane_first_leaf), Some(lane_height), Some(lane_target)) = (
            self.roots.get_mut(self.len),
            self.first_leaves.get_mut(self.len),
            self.heights.get_mut(self.len),
            self.targets.get_mut(self.len),
        ) {
            *lane_root = root;
            *lane_first_leaf = first_leaf;
            *lane_height = height;
            *lane_target = target;
            self.len += 1;
        }
    }

    /// Lift subtrees to their targets one level at a time
    // Inlining is only prevented in normal builds, `no_panic` relies on it
    #[cfg_attr(not(feature = "no-panic"), inline(never))]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn lift(&mut self, hasher: &H) {
        // Roots of subtrees below their targets next to their empty siblings, in order
        let mut pairs = [[[0; H::OUT_LEN]; 2]; PARALLEL_PAIRS::<H>];
        loop {
            let mut num_pairs = 0;
            for (root, first_leaf, height, target) in self.iter() {
                if height < target
                    && let Some(pair) = pairs.get_mut(num_pairs)
                {
                    *pair = if (first_leaf >> height).is_multiple_of(2) {
                        [*root, [0; H::OUT_LEN]]
                    } else {
                        [[0; H::OUT_LEN], *root]
                    };
                    num_pairs += 1;
                }
            }

            // The narrowest SIMD that hashes all pairs. There are at most
            // `MerkleHasher::PARALLEL_PAIRS` of them, guards only keep arms with more pairs from
            // being compiled.
            match num_pairs {
                1 => self.lift_level::<1>(hasher, &pairs),
                2 if PARALLEL_PAIRS::<H> >= 2 => self.lift_level::<2>(hasher, &pairs),
                3..=4 if PARALLEL_PAIRS::<H> >= 4 => self.lift_level::<4>(hasher, &pairs),
                5..=8 if PARALLEL_PAIRS::<H> >= 8 => self.lift_level::<8>(hasher, &pairs),
                9..=16 if PARALLEL_PAIRS::<H> >= 16 => self.lift_level::<16>(hasher, &pairs),
                17..=32 if PARALLEL_PAIRS::<H> >= 32 => self.lift_level::<32>(hasher, &pairs),
                33..=64 if PARALLEL_PAIRS::<H> >= 64 => self.lift_level::<64>(hasher, &pairs),
                // All subtrees reached their targets
                _ => break,
            }
        }
    }

    /// Hash the first `NUM_PAIRS` pairs, parents become roots of subtrees below their targets
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn lift_level<const NUM_PAIRS: usize>(
        &mut self,
        hasher: &H,
        pairs: &[[[u8; H::OUT_LEN]; 2]; PARALLEL_PAIRS::<H>],
    ) {
        if let Some(pairs) = pairs.first_chunk::<NUM_PAIRS>() {
            let parents = hasher.hash_pairs(pairs);
            // Pairs are in the order of subtrees below their targets, parents of the rest of the
            // pairs are not used
            let mut parents = parents.iter();
            for (root, _, height, target) in self.iter_mut() {
                if *height < target
                    && let Some(parent) = parents.next()
                {
                    *root = *parent;
                    *height += 1;
                }
            }
        }
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
/// [`Default`] since the number of leaves is always `2^BITS`. Nodes of all zeroes are empty, so `H`
/// must not hash nodes into all zeroes (cryptographic hashes practically never do), otherwise the
/// root is ambiguous and may depend on [`MerkleHasher::PARALLEL_PAIRS`].
#[derive(Debug)]
pub struct SparseMerkleTree<const BITS: u8, H>(PhantomCovariant<H>);

// TODO: Nodes in the stack are still hashed with empty siblings one pair at a time, paths of
//  different nodes could be hashed in parallel with `MerkleHasher::hash_pairs()`
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
        } else {
            // Consecutive occupied leaves within a multiple of the buffer size, hashed together
            // once the buffer is full, on the next empty leaf or at the end
            let mut buffer = [MaybeUninit::<[u8; H::OUT_LEN]>::uninit(); BUFFER_NUM_LEAVES::<H>];
            // Index of the first leaf in the buffer, which is empty if it is the index of the next
            // leaf
            let mut buffer_start = 0;
            // Subtrees of buffered leaves are lifted in lanes before they are pushed into the stack
            let mut lanes = Lanes::<H>::EMPTY;

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
                            // the index of the current leaf
                            unsafe {
                                Self::push_buffered_leaves(
                                    &hasher,
                                    &mut stack,
                                    &mut lanes,
                                    &mut buffer,
                                    num_leaves,
                                    buffer_start,
                                    buffer_end,
                                );
                            }
                        }
                        // Empty leaves are pushed into the stack later, together with subtrees
                        // after them
                        num_leaves =
                            Self::checked_skip(&mut processed_some, num_leaves, skip_count.get())?;
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
                    // SAFETY: Leaves were written to the buffer from `buffer_start` up to its end
                    unsafe {
                        Self::push_buffered_leaves(
                            &hasher,
                            &mut stack,
                            &mut lanes,
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
                // the last leaf
                unsafe {
                    Self::push_buffered_leaves(
                        &hasher,
                        &mut stack,
                        &mut lanes,
                        &mut buffer,
                        num_leaves,
                        buffer_start,
                        buffer_end,
                    );
                }
            }

            Self::push_lanes(&hasher, &mut stack, &mut lanes);
            // For `BITS == u128::BITS` the number of leaves in the tree wraps around to zero just
            // like `stack_leaves` does after the last leaf
            let trailing_leaves = if u32::from(BITS) < u128::BITS {
                2u128.pow(u32::from(BITS)) - lanes.stack_leaves
            } else {
                0u128.wrapping_sub(lanes.stack_leaves)
            };
            // Leaves in lanes and in the stack were checked to be within the tree already
            Self::skip_leaves(
                &hasher,
                &mut stack,
                &mut false,
                lanes.stack_leaves,
                trailing_leaves,
            );
        }

        Some(stack[BITS as usize])
    }

    /// Push the root of a subtree of `2^height` leaves that starts at leaf `first_leaf`, merging it
    /// with the nodes on the stack it completes
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn push_subtree(
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

        // Place the node at the first inactive level, which is within the stack for subtrees
        // within the tree
        if let Some(slot) = stack.get_mut(height as usize + active_levels) {
            *slot = node;
        }
    }

    /// Add leaves `buffer[start..end]` to lanes as the fewest perfect subtrees that start at
    /// multiples of their sizes, `leaf_index` is the index of any leaf within the same multiple of
    /// the buffer size.
    ///
    /// Contents of the buffer are meaningless afterward.
    ///
    /// # Safety
    /// `end` must not be larger than the buffer size and leaves `buffer[start..end]` must be
    /// initialized.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    unsafe fn push_buffered_leaves(
        hasher: &H,
        stack: &mut [[u8; H::OUT_LEN]; STACK_SIZE::<BITS>],
        lanes: &mut Lanes<H>,
        buffer: &mut [MaybeUninit<[u8; H::OUT_LEN]>; BUFFER_NUM_LEAVES::<H>],
        leaf_index: u128,
        mut start: usize,
        end: usize,
    ) {
        // Truncation keeps the lowest bits, which are the index in the buffer
        let buffer_first_leaf = leaf_index - (leaf_index as usize % BUFFER_NUM_LEAVES::<H>) as u128;

        while start < end {
            // The largest subtree that starts at a multiple of its size and ends within the range
            let height = (end - start).ilog2().min(start.trailing_zeros());
            // SAFETY: The subtree is within `buffer[start..end]`, which is within the buffer and
            // initialized according to the function contract
            let root = unsafe { Self::buffered_subtree_root(hasher, buffer, start, height) };
            Self::add_subtree(
                hasher,
                stack,
                lanes,
                buffer_first_leaf + start as u128,
                u8::try_from(height)
                    .expect("Subtrees are within the buffer of 512 leaves at most; qed"),
                root,
            );
            start += 2usize.pow(height);
        }
    }

    /// Root of a perfect subtree of `2^height` leaves that starts at `buffer[start]`.
    ///
    /// Hashing needs more stack than anything else that processes leaves, a separate frame keeps
    /// it off the stack while [`Self::push_lanes()`] runs. Leaves of the subtree in the buffer are
    /// meaningless afterward.
    ///
    /// # Safety
    /// `start + 2^height` must not be larger than the buffer size and leaves of the subtree must
    /// be initialized.
    // Inlining is only prevented in normal builds, `no_panic` relies on it
    #[cfg_attr(not(feature = "no-panic"), inline(never))]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    unsafe fn buffered_subtree_root(
        hasher: &H,
        buffer: &mut [MaybeUninit<[u8; H::OUT_LEN]>; BUFFER_NUM_LEAVES::<H>],
        start: usize,
        height: u32,
    ) -> [u8; H::OUT_LEN] {
        // SAFETY: Guaranteed by the function contract
        let leaves = unsafe {
            buffer
                .get_unchecked_mut(start..start + 2usize.pow(height))
                .assume_init_mut()
        };
        // SAFETY: Number of leaves is a power of two
        unsafe { perfect_subtree_root(hasher, leaves) }
    }

    /// Add a subtree of `2^height` leaves that starts at leaf `first_leaf` after the subtrees that
    /// were added before, pushing lanes into the stack first when they are full or when they hold a
    /// single subtree that isn't lifted
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn add_subtree(
        hasher: &H,
        stack: &mut [[u8; H::OUT_LEN]; STACK_SIZE::<BITS>],
        lanes: &mut Lanes<H>,
        first_leaf: u128,
        height: u8,
        root: [u8; H::OUT_LEN],
    ) {
        // Subtrees of up to `2^limit` leaves, where `limit` is the highest bit in which the first
        // leaves of this and the previous subtree differ, contain only one of them. The whole tree
        // is never needed either.
        let limit = lanes
            .previous_leaf
            .replace(first_leaf)
            .and_then(|previous_leaf| (previous_leaf ^ first_leaf).checked_ilog2())
            .unwrap_or(u32::from(BITS) - 1);
        let limit = u8::try_from(limit).expect("Smaller than `BITS`; qed");
        if let Some(last) = lanes.len.checked_sub(1)
            && let Some(target) = lanes.targets.get_mut(last)
        {
            *target = (*target).min(limit);
        }

        // Targets in lanes are final now. A single subtree that isn't lifted, like in runs of
        // consecutive leaves and dense trees, is pushed right away instead of waiting for lanes to
        // fill up.
        if lanes.len == PARALLEL_PAIRS::<H>
            || (lanes.len == 1 && lanes.heights.first() == lanes.targets.first())
        {
            Self::push_lanes(hasher, stack, lanes);
        }

        // The root of all zeroes isn't lifted, such that the stack hashes it with empty siblings
        // like hashers that hash pairs one by one do. The target is lower than the subtree only
        // for the whole tree.
        let target = if root == Self::ZERO {
            height
        } else {
            limit.max(height)
        };
        lanes.push(root, first_leaf, height, target);
    }

    /// Lift subtrees in lanes to their targets, then push them into the stack along with empty
    /// leaves before them
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn push_lanes(
        hasher: &H,
        stack: &mut [[u8; H::OUT_LEN]; STACK_SIZE::<BITS>],
        lanes: &mut Lanes<H>,
    ) {
        // Lifting and pushing use separate frames, which don't need to be on the stack at the
        // same time. Dense trees push subtrees that aren't lifted one at a time and skip lifting.
        if lanes.iter().any(|(_, _, height, target)| height < target) {
            lanes.lift(hasher);
        }
        Self::push_lifted_lanes(hasher, stack, lanes);
    }

    /// Push subtrees in lanes that reached their targets into the stack along with empty leaves
    /// before them
    // Inlining is only prevented in normal builds, `no_panic` relies on it
    #[cfg_attr(not(feature = "no-panic"), inline(never))]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn push_lifted_lanes(
        hasher: &H,
        stack: &mut [[u8; H::OUT_LEN]; STACK_SIZE::<BITS>],
        lanes: &mut Lanes<H>,
    ) {
        let mut stack_leaves = lanes.stack_leaves;
        for (root, first_leaf, height, _) in lanes.iter() {
            let first_leaf = first_leaf >> height << height;
            // Leaves in lanes and in the stack were checked to be within the tree already
            Self::skip_leaves(
                hasher,
                stack,
                &mut false,
                stack_leaves,
                first_leaf - stack_leaves,
            );
            Self::push_subtree(hasher, stack, first_leaf, u32::from(height), *root);
            // Wrapping is needed for `BITS == u128::BITS`, where the last subtree may end with the
            // last leaf of the tree
            stack_leaves = first_leaf.wrapping_add(2u128.pow(u32::from(height)));
        }
        lanes.stack_leaves = stack_leaves;
        lanes.len = 0;
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
        Self::checked_skip(processed_some, num_leaves, skip_count)?;

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
            // fit into 2^BITS, while `BITS` generic in turn ensured sufficient stack size. For
            // `BITS == u128::BITS` the level is limited by the loop above instead.
            *unsafe { stack.get_unchecked_mut(level as usize) } = current;

            // Wrapping is needed for `BITS == u128::BITS`, where number of leaves narrowly
            // doesn't fit into `u128` itself
            num_leaves = num_leaves.wrapping_add(chunk_size);
            skip_count -= chunk_size;
        }

        Some(num_leaves)
    }

    /// Returns updated number of leaves after `skip_count` empty leaves, `None` if there are too
    /// many of them
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn checked_skip(processed_some: &mut bool, num_leaves: u128, skip_count: u128) -> Option<u128> {
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

        // Wrapping is needed for `BITS == u128::BITS`, where number of leaves narrowly doesn't fit
        // into `u128` itself
        Some(num_leaves.wrapping_add(skip_count))
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
