use crate::{MerkleHasher, NewMerkleHasherForTreeSize};
#[cfg(feature = "alloc")]
use alloc::boxed::Box;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;
use core::iter::TrustedLen;
use core::marker::PhantomCovariant;
use core::mem::MaybeUninit;

/// Number of elements in a proof for a tree with `MAX_N` leaves
pub const PROOF_ELEMENTS<const MAX_N: u64>: usize = MAX_N.next_power_of_two().ilog2() as usize;
const STACK_SIZE<const MAX_N: u64>: usize = MAX_N.next_power_of_two().ilog2() as usize + 1;
const SUPPORTED_LEAVES_ARRAY_SIZE<const N: usize>: usize = {
    assert!(N != 0, "Number of leaves must be greater than 0");
    0
};
const USIZE_TO_U64<const N: usize>: u64 = N as u64;

/// Merkle Tree variant that has pre-hashed leaves with arbitrary number of elements.
///
/// This can be considered a general case of [`BalancedMerkleTree`]. The root and proofs are
/// identical for both in case the number of leaves is a power of two. [`BalancedMerkleTree`] is
/// more efficient and should be preferred when possible.
///
/// [`BalancedMerkleTree`]: crate::balanced::BalancedMerkleTree
///
/// The unbalanced tree is not padded, it is created the same way Merkle Mountain Range would be:
/// ```text
///               Root
///         /--------------\
///        H3              H4
///    /-------\         /----\
///   H0       H1       H2     \
///  /  \     /  \     /  \     \
/// L0  L1   L2  L3   L4  L5    L6
/// ```
///
/// Inner nodes are hashed with `H`, which is created from the number of leaves. That is why methods
/// that take an iterator of leaves require a [`TrustedLen`] iterator. Variants with `dynamic_len`
/// in their name accept any iterator instead, but only support hashers that implement [`Default`].
#[derive(Debug)]
pub struct UnbalancedMerkleTree<H>(PhantomCovariant<H>);

// TODO: Experiment with replacing a single pass with splitting the whole data set with a sequence
//  of power-of-two elements that can be processed in parallel and do it recursively until a single
//  element is left. This can be done for both root creation and proof generation.
impl<H> UnbalancedMerkleTree<H>
where
    H: MerkleHasher,
{
    /// Compute Merkle Tree Root.
    ///
    /// `MAX_N` generic constant defines the maximum number of elements supported and controls stack
    /// usage.
    ///
    /// Returns `None` for an empty list of leaves, or if the number of leaves is larger than
    /// `MAX_N`.
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn compute_root_only<'a, const MAX_N: u64, Item, Iter>(
        leaves: Iter,
    ) -> Option<[u8; H::OUT_LEN]>
    where
        H: NewMerkleHasherForTreeSize,
        Item: Into<[u8; H::OUT_LEN]>,
        Iter: IntoIterator<Item = Item, IntoIter: TrustedLen> + 'a,
    {
        let leaves = leaves.into_iter();
        let hasher = Self::hasher_for_leaves(&leaves)?;

        Self::compute_root_only_with::<MAX_N, _, _>(&hasher, leaves)
    }

    /// Similar to [`Self::compute_root_only()`], but for iterators that are not [`TrustedLen`]
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn compute_root_only_dynamic_len<'a, const MAX_N: u64, Item, Iter>(
        leaves: Iter,
    ) -> Option<[u8; H::OUT_LEN]>
    where
        H: Default,
        Item: Into<[u8; H::OUT_LEN]>,
        Iter: IntoIterator<Item = Item> + 'a,
    {
        let hasher = H::default();

        Self::compute_root_only_with::<MAX_N, _, _>(&hasher, leaves)
    }

    #[inline]
    fn compute_root_only_with<const MAX_N: u64, Item, Iter>(
        hasher: &H,
        leaves: Iter,
    ) -> Option<[u8; H::OUT_LEN]>
    where
        Item: Into<[u8; H::OUT_LEN]>,
        Iter: IntoIterator<Item = Item>,
    {
        // Stack of intermediate nodes per tree level
        let mut stack = [[0u8; H::OUT_LEN]; STACK_SIZE::<MAX_N>];
        let mut num_leaves = 0u64;

        for hash in leaves {
            // How many leaves were processed so far. Should have been `num_leaves == MAX_N`, but
            // `>=` helps compiler with panic safety checks.
            // TODO: Use `==` once https://github.com/rust-lang/rust/issues/162834 is resolved
            if num_leaves >= MAX_N {
                return None;
            }

            let mut current = hash.into();

            // Every bit set to `1` corresponds to an active Merkle Tree level
            let lowest_active_levels = num_leaves.trailing_ones() as usize;
            for item in stack.iter().take(lowest_active_levels) {
                current = hasher.hash_pair(&[*item, current]);
            }

            // Place the current hash at the first inactive level
            stack[lowest_active_levels] = current;
            num_leaves += 1;
        }

        if num_leaves == 0 {
            // If no leaves were provided
            return None;
        }

        // SAFETY: `num_leaves` is not zero, not larger than `MAX_N`, and peaks of all `num_leaves`
        // leaves are on the stack
        Some(unsafe { Self::root_from_peaks::<MAX_N>(hasher, &mut stack, num_leaves) })
    }

    /// Hash peaks of `num_leaves` leaves on the `stack` together into the root.
    ///
    /// # Safety
    /// `num_leaves` must not be zero or larger than `MAX_N`, and peaks of all `num_leaves` leaves
    /// must be on the `stack`.
    #[inline(always)]
    unsafe fn root_from_peaks<const MAX_N: u64>(
        hasher: &H,
        stack: &mut [[u8; H::OUT_LEN]; STACK_SIZE::<MAX_N>],
        num_leaves: u64,
    ) -> [u8; H::OUT_LEN] {
        let mut stack_bits = num_leaves;

        {
            let lowest_active_level = stack_bits.trailing_zeros() as usize;
            // Reuse `stack[0]` for resulting value
            // SAFETY: Active level must have been set successfully before, hence it exists
            stack[0] = *unsafe { stack.get_unchecked(lowest_active_level) };
            // Clear lowest active level
            stack_bits &= !(1 << lowest_active_level);
        }

        // Hash remaining peaks (if any) of the potentially unbalanced tree together
        loop {
            let lowest_active_level = stack_bits.trailing_zeros() as usize;

            if lowest_active_level == u64::BITS as usize {
                break;
            }

            // Clear lowest active level for next iteration
            stack_bits &= !(1 << lowest_active_level);

            // SAFETY: Active level must have been set successfully before, hence it exists
            let lowest_active_level_item = unsafe { stack.get_unchecked(lowest_active_level) };

            stack[0] = hasher.hash_pair(&[*lowest_active_level_item, stack[0]]);
        }

        if num_leaves == 1 {
            stack[0] = hasher.hash_single_leaf(&stack[0]);
        }

        stack[0]
    }

    /// Compute Merkle Tree Root of `num_leaves` leaves, which are all zeroes except `leaves`.
    ///
    /// `leaves` are pairs of leaf index and leaf, with strictly increasing indices. Produces the
    /// same root as [`Self::compute_root_only()`] with zeroes in all other positions, but hashes
    /// runs of zero leaves as whole subtrees, so the number of hashes is proportional to the number
    /// of `leaves` and the height of the tree rather than to `num_leaves`.
    ///
    /// `MAX_N` generic constant defines the maximum number of elements supported and controls stack
    /// usage.
    ///
    /// Returns `None` if `num_leaves` is zero or larger than `MAX_N`, or if indices aren't strictly
    /// increasing or not smaller than `num_leaves`.
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn compute_root_only_sparse<const MAX_N: u64, Item, Iter>(
        num_leaves: u64,
        leaves: Iter,
    ) -> Option<[u8; H::OUT_LEN]>
    where
        H: NewMerkleHasherForTreeSize,
        Item: Into<[u8; H::OUT_LEN]>,
        Iter: IntoIterator<Item = (u64, Item)>,
    {
        if num_leaves == 0 || num_leaves > MAX_N {
            return None;
        }

        let hasher = H::new_for_tree_size(num_leaves);
        // Stack of intermediate nodes per tree level
        let mut stack = [[0u8; H::OUT_LEN]; STACK_SIZE::<MAX_N>];
        // Roots of perfect subtrees of zero leaves per height, filled up to `zeros_height`
        let mut zeros = [[0u8; H::OUT_LEN]; STACK_SIZE::<MAX_N>];
        let mut zeros_height = 0;
        // Number of leaves processed so far, including zero leaves
        let mut processed = 0u64;

        for (index, leaf) in leaves {
            if index < processed || index >= num_leaves {
                return None;
            }

            // SAFETY: `processed <= index < num_leaves <= MAX_N`
            unsafe {
                Self::push_zero_leaves::<MAX_N>(
                    &hasher,
                    &mut stack,
                    &mut zeros,
                    &mut zeros_height,
                    &mut processed,
                    index,
                );
            }
            // SAFETY: Any `processed` is a multiple of `2^0` and `processed + 1 <= num_leaves`,
            // which is not larger than `MAX_N`
            unsafe {
                Self::push_subtree::<MAX_N>(&hasher, &mut stack, processed, 0, leaf.into());
            }
            processed += 1;
        }

        // SAFETY: `processed <= num_leaves <= MAX_N`
        unsafe {
            Self::push_zero_leaves::<MAX_N>(
                &hasher,
                &mut stack,
                &mut zeros,
                &mut zeros_height,
                &mut processed,
                num_leaves,
            );
        }

        // SAFETY: `num_leaves` is not zero, not larger than `MAX_N`, and peaks of all `num_leaves`
        // leaves are on the stack
        Some(unsafe { Self::root_from_peaks::<MAX_N>(&hasher, &mut stack, num_leaves) })
    }

    /// Push zero leaves until `processed` reaches `target`, as perfect subtrees that are as large
    /// as possible.
    ///
    /// # Safety
    /// `processed` must not be larger than `target`, which must not be larger than `MAX_N`.
    #[inline(always)]
    unsafe fn push_zero_leaves<const MAX_N: u64>(
        hasher: &H,
        stack: &mut [[u8; H::OUT_LEN]; STACK_SIZE::<MAX_N>],
        zeros: &mut [[u8; H::OUT_LEN]; STACK_SIZE::<MAX_N>],
        zeros_height: &mut usize,
        processed: &mut u64,
        target: u64,
    ) {
        while let Some(height) = (target - *processed).checked_ilog2() {
            // The largest subtree that fits into the remaining zero leaves and starts at a multiple
            // of its size
            let height = height.min(processed.trailing_zeros()) as usize;

            while *zeros_height < height {
                // SAFETY: `height` is at most `log2(MAX_N)`, which is within `STACK_SIZE`
                let zero = unsafe { zeros.get_unchecked(*zeros_height) };
                let zero = hasher.hash_pair(&[*zero, *zero]);
                *zeros_height += 1;
                // SAFETY: `height` is at most `log2(MAX_N)`, which is within `STACK_SIZE`
                *unsafe { zeros.get_unchecked_mut(*zeros_height) } = zero;
            }

            // SAFETY: `height` is at most `log2(MAX_N)`, which is within `STACK_SIZE`
            let zero = *unsafe { zeros.get_unchecked(height) };
            // SAFETY: `processed` is a multiple of `2^height` and `processed + 2^height <= target`,
            // which is not larger than `MAX_N`
            unsafe {
                Self::push_subtree::<MAX_N>(hasher, stack, *processed, height, zero);
            }
            *processed += 1 << height;
        }
    }

    /// Push the root of a perfect subtree of height `height` after `processed` leaves, merging it
    /// with the peaks it completes the same way as [`Self::compute_root_only()`] merges a leaf.
    ///
    /// # Safety
    /// `processed` must be a multiple of `2^height`, and `processed + 2^height` must not be larger
    /// than `MAX_N`.
    #[inline(always)]
    unsafe fn push_subtree<const MAX_N: u64>(
        hasher: &H,
        stack: &mut [[u8; H::OUT_LEN]; STACK_SIZE::<MAX_N>],
        processed: u64,
        height: usize,
        mut node: [u8; H::OUT_LEN],
    ) {
        // Every bit set to `1` corresponds to an active Merkle Tree level
        let active_levels = (processed >> height).trailing_ones() as usize;
        for item in stack.iter().skip(height).take(active_levels) {
            node = hasher.hash_pair(&[*item, node]);
        }

        // Place the node at the first inactive level.
        //
        // SAFETY: Bit `height + active_levels` is set in `processed + 2^height`, which is not
        // larger than `MAX_N`, so the level is at most `log2(MAX_N)`, which is within `STACK_SIZE`
        *unsafe { stack.get_unchecked_mut(height + active_levels) } = node;
    }

    /// Compute Merkle Tree Root for an array of leaves.
    ///
    /// Similar to [`Self::compute_root_only()`], but for inputs that are small arrays where `MAX_N`
    /// is the same as the array length.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn compute_root_only_array<const N: usize, Item>(leaves: &[Item; N]) -> [u8; H::OUT_LEN]
    where
        H: const NewMerkleHasherForTreeSize,
        [(); SUPPORTED_LEAVES_ARRAY_SIZE::<N>]:,
        Item: Into<[u8; H::OUT_LEN]> + Copy,
    {
        // TODO: Technically redundant with the where-clause, which is not enforced in generic code
        //  due to https://github.com/rust-lang/rust/issues/164019, remove once fixed
        const { SUPPORTED_LEAVES_ARRAY_SIZE::<N> };

        let hasher = const { H::new_for_tree_size(N as u64) };
        // Iterating over indices rather than with `leaves.iter().copied()` allows the compiler to
        // prove that the root always exists on aarch64 and riscv64 too
        let leaves = (0..N).map(|index| *leaves.get(index).expect("Index is within array; qed"));
        let maybe_root =
            Self::compute_root_only_with::<{ USIZE_TO_U64::<N> }, _, _>(&hasher, leaves);
        maybe_root.expect("Non-empty array with length matching `MAX_N` always has a root; qed")
    }

    /// Compute Merkle Tree root and generate a proof for the `leaf` at `target_index`.
    ///
    /// Returns `Some((root, proof))` on success, `None` if index is outside of list of leaves.
    ///
    /// `MAX_N` generic constant defines the maximum number of elements supported and controls stack
    /// usage.
    #[expect(clippy::type_complexity, reason = "Return type")]
    #[inline]
    #[cfg(feature = "alloc")]
    pub fn compute_root_and_proof<'a, const MAX_N: u64, Item, Iter>(
        leaves: Iter,
        target_index: usize,
    ) -> Option<([u8; H::OUT_LEN], Vec<[u8; H::OUT_LEN]>)>
    where
        H: NewMerkleHasherForTreeSize,
        Item: Into<[u8; H::OUT_LEN]>,
        Iter: IntoIterator<Item = Item, IntoIter: TrustedLen> + 'a,
    {
        let leaves = leaves.into_iter();
        let hasher = Self::hasher_for_leaves(&leaves)?;

        Self::compute_root_and_proof_with::<MAX_N, _, _>(&hasher, leaves, target_index)
    }

    /// Similar to [`Self::compute_root_and_proof()`], but for iterators that are not [`TrustedLen`]
    #[expect(clippy::type_complexity, reason = "Return type")]
    #[inline]
    #[cfg(feature = "alloc")]
    pub fn compute_root_and_proof_dynamic_len<'a, const MAX_N: u64, Item, Iter>(
        leaves: Iter,
        target_index: usize,
    ) -> Option<([u8; H::OUT_LEN], Vec<[u8; H::OUT_LEN]>)>
    where
        H: Default,
        Item: Into<[u8; H::OUT_LEN]>,
        Iter: IntoIterator<Item = Item> + 'a,
    {
        let hasher = H::default();

        Self::compute_root_and_proof_with::<MAX_N, _, _>(&hasher, leaves, target_index)
    }

    #[expect(clippy::type_complexity, reason = "Return type")]
    #[inline(always)]
    #[cfg(feature = "alloc")]
    fn compute_root_and_proof_with<const MAX_N: u64, Item, Iter>(
        hasher: &H,
        leaves: Iter,
        target_index: usize,
    ) -> Option<([u8; H::OUT_LEN], Vec<[u8; H::OUT_LEN]>)>
    where
        Item: Into<[u8; H::OUT_LEN]>,
        Iter: IntoIterator<Item = Item>,
    {
        // Stack of intermediate nodes per tree level
        let mut stack = [[0u8; H::OUT_LEN]; _];
        // SAFETY: Inner value is `MaybeUninit`
        let mut proof =
            unsafe { Box::<[MaybeUninit<[u8; H::OUT_LEN]>; _]>::new_uninit().assume_init() };

        let (root, proof_length) = Self::compute_root_and_proof_inner::<MAX_N, _, _>(
            hasher,
            leaves,
            target_index,
            &mut stack,
            &mut proof,
        )?;

        let proof_capacity = proof.len();
        let proof = Box::into_raw(proof);
        // SAFETY: Points to correctly allocated memory where `proof_length` elements were
        // initialized
        let proof = unsafe {
            Vec::from_raw_parts(
                proof.cast::<[u8; H::OUT_LEN]>(),
                proof_length,
                proof_capacity,
            )
        };

        Some((root, proof))
    }

    /// Compute Merkle Tree root and generate a proof for the `leaf` at `target_index`.
    ///
    /// Returns `Some((root, proof))` on success, `None` if index is outside of list of leaves.
    ///
    /// `MAX_N` generic constant defines the maximum number of elements supported and controls stack
    /// usage.
    #[expect(clippy::type_complexity, reason = "Return type")]
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn compute_root_and_proof_in<'a, 'proof, const MAX_N: u64, Item, Iter>(
        leaves: Iter,
        target_index: usize,
        proof: &'proof mut [MaybeUninit<[u8; H::OUT_LEN]>; PROOF_ELEMENTS::<MAX_N>],
    ) -> Option<([u8; H::OUT_LEN], &'proof mut [[u8; H::OUT_LEN]])>
    where
        H: NewMerkleHasherForTreeSize,
        Item: Into<[u8; H::OUT_LEN]>,
        Iter: IntoIterator<Item = Item, IntoIter: TrustedLen> + 'a,
    {
        let leaves = leaves.into_iter();
        let hasher = Self::hasher_for_leaves(&leaves)?;

        Self::compute_root_and_proof_in_with::<MAX_N, _, _>(&hasher, leaves, target_index, proof)
    }

    /// Similar to [`Self::compute_root_and_proof_in()`], but for iterators that are not
    /// [`TrustedLen`]
    #[expect(clippy::type_complexity, reason = "Return type")]
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn compute_root_and_proof_dynamic_len_in<'a, 'proof, const MAX_N: u64, Item, Iter>(
        leaves: Iter,
        target_index: usize,
        proof: &'proof mut [MaybeUninit<[u8; H::OUT_LEN]>; PROOF_ELEMENTS::<MAX_N>],
    ) -> Option<([u8; H::OUT_LEN], &'proof mut [[u8; H::OUT_LEN]])>
    where
        H: Default,
        Item: Into<[u8; H::OUT_LEN]>,
        Iter: IntoIterator<Item = Item> + 'a,
    {
        let hasher = H::default();

        Self::compute_root_and_proof_in_with::<MAX_N, _, _>(&hasher, leaves, target_index, proof)
    }

    #[expect(clippy::type_complexity, reason = "Return type")]
    #[inline(always)]
    fn compute_root_and_proof_in_with<'proof, const MAX_N: u64, Item, Iter>(
        hasher: &H,
        leaves: Iter,
        target_index: usize,
        proof: &'proof mut [MaybeUninit<[u8; H::OUT_LEN]>; PROOF_ELEMENTS::<MAX_N>],
    ) -> Option<([u8; H::OUT_LEN], &'proof mut [[u8; H::OUT_LEN]])>
    where
        Item: Into<[u8; H::OUT_LEN]>,
        Iter: IntoIterator<Item = Item>,
    {
        // Stack of intermediate nodes per tree level
        let mut stack = [[0u8; H::OUT_LEN]; _];

        let (root, proof_length) = Self::compute_root_and_proof_inner::<MAX_N, _, _>(
            hasher,
            leaves,
            target_index,
            &mut stack,
            proof,
        )?;
        // SAFETY: Just correctly initialized `proof_length` elements
        let proof = unsafe { proof.get_unchecked_mut(..proof_length).assume_init_mut() };

        Some((root, proof))
    }

    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn compute_root_and_proof_inner<const MAX_N: u64, Item, Iter>(
        hasher: &H,
        leaves: Iter,
        target_index: usize,
        stack: &mut [[u8; H::OUT_LEN]; STACK_SIZE::<MAX_N>],
        proof: &mut [MaybeUninit<[u8; H::OUT_LEN]>; PROOF_ELEMENTS::<MAX_N>],
    ) -> Option<([u8; H::OUT_LEN], usize)>
    where
        Item: Into<[u8; H::OUT_LEN]>,
        Iter: IntoIterator<Item = Item>,
    {
        let mut proof_length = 0;
        let mut num_leaves = 0u64;

        let mut current_target_level = None;
        let mut position = target_index;

        for (current_index, hash) in leaves.into_iter().enumerate() {
            // How many leaves were processed so far. Should have been `num_leaves == MAX_N`, but
            // `>=` helps compiler with panic safety checks.
            // TODO: Use `==` once https://github.com/rust-lang/rust/issues/162834 is resolved
            if num_leaves >= MAX_N {
                return None;
            }

            let mut current = hash.into();

            // Every bit set to `1` corresponds to an active Merkle Tree level
            let lowest_active_levels = num_leaves.trailing_ones() as usize;

            if current_index == target_index {
                for item in stack.iter().take(lowest_active_levels) {
                    // If at the target leaf index, need to collect the proof
                    // SAFETY: Method signature guarantees upper bound of the proof length
                    unsafe { proof.get_unchecked_mut(proof_length) }.write(*item);
                    proof_length += 1;

                    current = hasher.hash_pair(&[*item, current]);

                    // Move up the tree
                    position /= 2;
                }

                current_target_level = Some(lowest_active_levels);
            } else {
                for (level, item) in stack.iter().enumerate().take(lowest_active_levels) {
                    if current_target_level == Some(level) {
                        // SAFETY: Method signature guarantees upper bound of the proof length
                        unsafe { proof.get_unchecked_mut(proof_length) }.write(
                            if position.is_multiple_of(2) {
                                current
                            } else {
                                *item
                            },
                        );
                        proof_length += 1;

                        current_target_level = Some(level + 1);

                        // Move up the tree
                        position /= 2;
                    }

                    current = hasher.hash_pair(&[*item, current]);
                }
            }

            // Place the current hash at the first inactive level
            stack[lowest_active_levels] = current;
            num_leaves += 1;
        }

        // `active_levels` here contains the number of leaves after above loop
        if target_index >= num_leaves as usize {
            // If no leaves were provided
            return None;
        }

        let Some(current_target_level) = current_target_level else {
            // Index not found
            return None;
        };

        let mut stack_bits = num_leaves;

        {
            let lowest_active_level = stack_bits.trailing_zeros() as usize;
            // Reuse `stack[0]` for resulting value
            // SAFETY: Active level must have been set successfully before, hence it exists
            stack[0] = *unsafe { stack.get_unchecked(lowest_active_level) };
            // Clear lowest active level
            stack_bits &= !(1 << lowest_active_level);
        }

        // Hash remaining peaks (if any) of the potentially unbalanced tree together and collect
        // proof hashes
        let mut merged_peaks = false;
        loop {
            let lowest_active_level = stack_bits.trailing_zeros() as usize;

            if lowest_active_level == u64::BITS as usize {
                break;
            }

            // Clear lowest active level for next iteration
            stack_bits &= !(1 << lowest_active_level);

            // SAFETY: Active level must have been set successfully before, hence it exists
            let lowest_active_level_item = unsafe { stack.get_unchecked(lowest_active_level) };

            if lowest_active_level > current_target_level
                || (lowest_active_level == current_target_level
                    && !position.is_multiple_of(2)
                    && !merged_peaks)
            {
                // SAFETY: Method signature guarantees upper bound of the proof length
                unsafe { proof.get_unchecked_mut(proof_length) }.write(*lowest_active_level_item);
                proof_length += 1;
                merged_peaks = false;
            } else if lowest_active_level == current_target_level {
                // SAFETY: Method signature guarantees upper bound of the proof length
                unsafe { proof.get_unchecked_mut(proof_length) }.write(stack[0]);
                proof_length += 1;
                merged_peaks = false;
            } else {
                // Not collecting proof because of the need to merge peaks of an unbalanced tree
                merged_peaks = true;
            }

            // Collect the lowest peak into the proof
            stack[0] = hasher.hash_pair(&[*lowest_active_level_item, stack[0]]);

            position /= 2;
        }

        if num_leaves == 1 {
            stack[0] = hasher.hash_single_leaf(&stack[0]);
        }

        Some((stack[0], proof_length))
    }

    /// Verify a Merkle proof for a leaf at the given index
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn verify(
        root: &[u8; H::OUT_LEN],
        proof: &[[u8; H::OUT_LEN]],
        leaf_index: u64,
        leaf: [u8; H::OUT_LEN],
        num_leaves: u64,
    ) -> bool
    where
        H: NewMerkleHasherForTreeSize,
    {
        if leaf_index >= num_leaves {
            return false;
        }

        let hasher = H::new_for_tree_size(num_leaves);
        let mut current = leaf;
        let mut position = leaf_index;
        let mut proof_pos = 0;
        let mut level_size = num_leaves;

        // Rebuild the path to the root
        while level_size > 1 {
            let is_left = position.is_multiple_of(2);
            let is_last = position == level_size - 1;

            if is_left && !is_last {
                // Left node with a right sibling
                if proof_pos >= proof.len() {
                    // Missing sibling
                    return false;
                }
                current = hasher.hash_pair(&[current, proof[proof_pos]]);
                proof_pos += 1;
            } else if !is_left {
                // Right node with a left sibling
                if proof_pos >= proof.len() {
                    // Missing sibling
                    return false;
                }
                current = hasher.hash_pair(&[proof[proof_pos], current]);
                proof_pos += 1;
            } else {
                // Last node, no sibling, keep current
            }

            position /= 2;
            // Size of next level
            level_size = level_size.div_ceil(2);
        }

        if num_leaves == 1 {
            current = hasher.hash_single_leaf(&current);
        }

        // Check if proof is fully used and matches root
        proof_pos == proof.len() && current == *root
    }

    /// Create a hasher for a tree with `leaves`.
    ///
    /// Returns `None` if there are more than `usize::MAX` leaves.
    #[inline(always)]
    fn hasher_for_leaves<Iter>(leaves: &Iter) -> Option<H>
    where
        H: NewMerkleHasherForTreeSize,
        Iter: TrustedLen,
    {
        // `TrustedLen` guarantees that the iterator yields exactly this many leaves. An empty list
        // or more than `MAX_N` leaves are rejected while iterating, which avoids extra checks for
        // hashers that ignore the number of leaves.
        let (_lower, upper) = leaves.size_hint();

        Some(H::new_for_tree_size(upper? as u64))
    }
}
