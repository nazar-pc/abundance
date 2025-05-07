use crate::hash_pair;
#[cfg(feature = "alloc")]
use alloc::boxed::Box;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;
use blake3::OUT_LEN;
use core::mem::MaybeUninit;

/// Merkle Tree variant that has pre-hashed leaves with arbitrary number of elements.
///
/// This can be considered a general case of [`BalancedHashedMerkleTree`]. The root and proofs are
/// identical for both in case the number of leaves is a power of two. [`BalancedHashedMerkleTree`]
/// is more efficient and should be preferred when possible.
///
/// [`BalancedHashedMerkleTree`]: crate::balanced_hashed::BalancedHashedMerkleTree
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
#[derive(Debug)]
pub struct UnbalancedHashedMerkleTree;

// TODO: Optimize by implementing SIMD-accelerated hashing of multiple values:
//  https://github.com/BLAKE3-team/BLAKE3/issues/478
// TODO: Experiment with replacing a single pass with splitting the whole data set with a sequence
//  of power-of-two elements that can be processed in parallel and do it recursively until a single
//  element is left. This can be done for both root creation and proof generation.
impl UnbalancedHashedMerkleTree {
    /// Compute Merkle Tree Root.
    ///
    /// `MAX_N` generic constant defines the maximum number of elements supported and controls stack
    /// usage.
    #[inline]
    pub fn compute_root_only<'a, const N: usize, Iter>(leaves: Iter) -> Option<[u8; OUT_LEN]>
    where
        [(); N.ilog2() as usize + 1]:,
        Iter: Iterator<Item = &'a [u8; OUT_LEN]> + 'a,
    {
        // Stack of intermediate nodes per tree level
        let mut stack = [[0u8; OUT_LEN]; N.ilog2() as usize + 1];
        // Bitmask: bit `i = 1` if level `i` is active
        let mut active_levels = 0_u64;

        for &hash in leaves {
            let mut current = hash;
            let mut level = 0;

            // Check if level is active by testing bit (active_levels & (1 << level))
            while (active_levels & (1 << level)) != 0 {
                current = hash_pair(&stack[level], &current);

                // Clear the current level
                active_levels &= !(1 << level);
                level += 1;
            }

            // Place the current hash at the first inactive level
            stack[level] = current;
            // Set bit for level
            active_levels |= 1 << level;
        }

        if active_levels == 0 {
            // If no leaves were provided
            return None;
        }

        {
            let lowest_active_level = active_levels.trailing_zeros() as usize;
            // Reuse `stack[0]` for resulting value
            stack[0] = stack[lowest_active_level];
            // Clear lowest active level
            active_levels &= !(1 << lowest_active_level);
        }

        // Hash remaining peaks (if any) of the potentially unbalanced tree together
        loop {
            let lowest_active_level = active_levels.trailing_zeros() as usize;

            if lowest_active_level == u64::BITS as usize {
                break;
            }

            // Clear lowest active level
            active_levels &= !(1 << lowest_active_level);

            stack[0] = hash_pair(&stack[lowest_active_level], &stack[0]);
        }

        Some(stack[0])
    }

    /// Compute Merkle Tree root and generate a proof for the `leaf` at `target_index`.
    ///
    /// Returns `Some(root, proof)` on success, `None` if index is outside of list of leaves.
    ///
    /// `MAX_N` generic constant defines the maximum number of elements supported and controls stack
    /// usage.
    #[inline]
    #[cfg(feature = "alloc")]
    pub fn compute_root_and_proof<'a, const N: usize, Iter>(
        leaves: Iter,
        target_index: usize,
    ) -> Option<([u8; OUT_LEN], Vec<[u8; OUT_LEN]>)>
    where
        [(); N.ilog2() as usize + 1]:,
        Iter: Iterator<Item = &'a [u8; OUT_LEN]> + 'a,
    {
        // Stack of intermediate nodes per tree level
        let mut stack = [[0u8; OUT_LEN]; N.ilog2() as usize + 1];
        // SAFETY: Inner value is `MaybeUninit`
        let mut proof = unsafe {
            Box::<[MaybeUninit<[u8; OUT_LEN]>; N.ilog2() as usize + 1]>::new_uninit().assume_init()
        };

        let (root, proof_length) =
            Self::compute_root_and_proof_inner(leaves, target_index, &mut stack, &mut proof)?;

        let proof_capacity = proof.len();
        let proof = Box::into_raw(proof);
        // SAFETY: Points to correctly allocated memory where `proof_length` elements were
        // initialized
        let proof = unsafe {
            Vec::from_raw_parts(proof.cast::<[u8; OUT_LEN]>(), proof_length, proof_capacity)
        };

        Some((root, proof))
    }

    /// Compute Merkle Tree root and generate a proof for the `leaf` at `target_index`.
    ///
    /// Returns `Some(root, proof)` on success, `None` if index is outside of list of leaves.
    ///
    /// `MAX_N` generic constant defines the maximum number of elements supported and controls stack
    /// usage.
    #[inline]
    pub fn compute_root_and_proof_in<'a, 'proof, const N: usize, Iter>(
        leaves: Iter,
        target_index: usize,
        proof: &'proof mut [MaybeUninit<[u8; OUT_LEN]>; N.ilog2() as usize + 1],
    ) -> Option<([u8; OUT_LEN], &'proof mut [[u8; OUT_LEN]])>
    where
        [(); N.ilog2() as usize + 1]:,
        Iter: Iterator<Item = &'a [u8; OUT_LEN]> + 'a,
    {
        // Stack of intermediate nodes per tree level
        let mut stack = [[0u8; OUT_LEN]; N.ilog2() as usize + 1];

        let (root, proof_length) =
            Self::compute_root_and_proof_inner(leaves, target_index, &mut stack, proof)?;
        // SAFETY: Just correctly initialized `proof_length` elements
        let proof = unsafe { proof[..proof_length].assume_init_mut() };

        Some((root, proof))
    }

    fn compute_root_and_proof_inner<'a, const N: usize, Iter>(
        leaves: Iter,
        target_index: usize,
        stack: &mut [[u8; OUT_LEN]; N.ilog2() as usize + 1],
        proof: &mut [MaybeUninit<[u8; OUT_LEN]>; N.ilog2() as usize + 1],
    ) -> Option<([u8; OUT_LEN], usize)>
    where
        [(); N.ilog2() as usize + 1]:,
        Iter: Iterator<Item = &'a [u8; OUT_LEN]> + 'a,
    {
        let mut proof_length = 0;
        let mut active_levels = 0_u64;

        let mut current_target_level = None;
        let mut position = target_index;

        for (current_index, &hash) in leaves.enumerate() {
            let mut current = hash;
            let mut level = 0;

            if current_index == target_index {
                // Check if level is active by testing bit (active_levels & (1 << level))
                while (active_levels & (1 << level)) != 0 {
                    // If at the target leaf index, need to collect the proof
                    // SAFETY: Method signature guarantees upper bound of the proof length
                    unsafe { proof.get_unchecked_mut(proof_length) }.write(stack[level]);
                    proof_length += 1;

                    current = hash_pair(&stack[level], &current);

                    // Clear the current level
                    active_levels &= !(1 << level);
                    level += 1;

                    // Move up the tree
                    position /= 2;
                }

                current_target_level = Some(level);

                // Place the current hash at the first inactive level
                stack[level] = current;
                // Set bit for level
                active_levels |= 1 << level;
            } else {
                // If at the target leaf index, need to collect the proof
                while (active_levels & (1 << level)) != 0 {
                    if current_target_level == Some(level) {
                        // SAFETY: Method signature guarantees upper bound of the proof length
                        unsafe { proof.get_unchecked_mut(proof_length) }.write(
                            if position % 2 == 0 {
                                current
                            } else {
                                stack[level]
                            },
                        );
                        proof_length += 1;

                        current_target_level = Some(level + 1);

                        // Move up the tree
                        position /= 2;
                    }

                    current = hash_pair(&stack[level], &current);

                    // Clear the current level
                    active_levels &= !(1 << level);
                    level += 1;
                }

                // Place the current hash at the first inactive level
                stack[level] = current;
                // Set bit for level
                active_levels |= 1 << level;
            }
        }

        // `active_levels` here contains the number of leaves after above loop
        if target_index >= active_levels as usize {
            // If no leaves were provided
            return None;
        }

        let Some(current_target_level) = current_target_level else {
            // Index not found
            return None;
        };

        {
            let lowest_active_level = active_levels.trailing_zeros() as usize;
            // Reuse `stack[0]` for resulting value
            stack[0] = stack[lowest_active_level];
            // Clear lowest active level
            active_levels &= !(1 << lowest_active_level);
        }

        // Hash remaining peaks (if any) of the potentially unbalanced tree together and collect
        // proof hashes
        let mut merged_peaks = false;
        loop {
            let lowest_active_level = active_levels.trailing_zeros() as usize;

            if lowest_active_level == u64::BITS as usize {
                break;
            }

            // Clear lowest active level
            active_levels &= !(1 << lowest_active_level);

            if lowest_active_level > current_target_level
                || (lowest_active_level == current_target_level
                    && (position % 2 != 0)
                    && !merged_peaks)
            {
                // SAFETY: Method signature guarantees upper bound of the proof length
                unsafe { proof.get_unchecked_mut(proof_length) }.write(stack[lowest_active_level]);
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
            stack[0] = hash_pair(&stack[lowest_active_level], &stack[0]);

            position /= 2;
        }

        Some((stack[0], proof_length))
    }

    /// Verify a Merkle proof for a leaf at the given index
    #[inline]
    pub fn verify(
        root: &[u8; OUT_LEN],
        proof: &[[u8; OUT_LEN]],
        leaf_index: usize,
        leaf: [u8; OUT_LEN],
        num_leaves: usize,
    ) -> bool {
        if leaf_index >= num_leaves {
            return false;
        }

        let mut current = leaf;
        let mut position = leaf_index;
        let mut proof_pos = 0;
        let mut level_size = num_leaves;

        // Rebuild the path to the root
        while level_size > 1 {
            let is_left = position % 2 == 0;
            let is_last = position == level_size - 1;

            if is_left && !is_last {
                // Left node with a right sibling
                if proof_pos >= proof.len() {
                    // Missing sibling
                    return false;
                }
                current = hash_pair(&current, &proof[proof_pos]);
                proof_pos += 1;
            } else if !is_left {
                // Right node with a left sibling
                if proof_pos >= proof.len() {
                    // Missing sibling
                    return false;
                }
                current = hash_pair(&proof[proof_pos], &current);
                proof_pos += 1;
            } else {
                // Last node, no sibling, keep current
            }

            position /= 2;
            // Size of next level
            level_size = level_size.div_ceil(2);
        }

        // Check if proof is fully used and matches root
        proof_pos == proof.len() && current == *root
    }
}
