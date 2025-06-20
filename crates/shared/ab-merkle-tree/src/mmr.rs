use crate::hash_pair;
use crate::unbalanced_hashed::UnbalancedHashedMerkleTree;
#[cfg(feature = "alloc")]
use alloc::boxed::Box;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;
use blake3::OUT_LEN;
use core::mem::MaybeUninit;

/// MMR peaks for [`MerkleMountainRange`]
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct MmrPeaks<const MAX_N: u64>
where
    [(); MAX_N.ilog2() as usize + 1]:,
{
    /// MMR peaks, first [`Self::num_peaks()`] elements are occupied by values, the rest are ignored
    /// and do not need to be retained.
    pub peaks: [[u8; OUT_LEN]; MAX_N.ilog2() as usize + 1],
    /// Number of leaves in MMR
    pub num_leaves: u64,
}

impl<const MAX_N: u64> MmrPeaks<MAX_N>
where
    [(); MAX_N.ilog2() as usize + 1]:,
{
    /// Number of peaks stored in [`Self::peaks`] that are occupied by actual values
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn num_peaks(&self) -> u8 {
        self.num_leaves.count_ones() as u8
    }
}

/// Merkle Mountain Range variant that has pre-hashed leaves with arbitrary number of elements.
///
/// This can be considered a general case of [`UnbalancedHashedMerkleTree`]. The root and proofs are
/// identical for both. [`UnbalancedHashedMerkleTree`] is more efficient and should be preferred
/// when possible, while this data structure is designed for aggregating data incrementally over
/// long periods of time.
///
/// `MAX_N` generic constant defines the maximum number of elements supported and controls stack
/// usage.
#[derive(Debug, Copy, Clone)]
pub struct MerkleMountainRange<const MAX_N: u64>
where
    [(); MAX_N.ilog2() as usize + 1]:,
{
    // Stack of intermediate nodes per tree level
    stack: [[u8; OUT_LEN]; MAX_N.ilog2() as usize + 1],
    num_leaves: u64,
}

impl<const MAX_N: u64> Default for MerkleMountainRange<MAX_N>
where
    [(); MAX_N.ilog2() as usize + 1]:,
{
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn default() -> Self {
        Self::new()
    }
}

// TODO: Think harder about proof generation and verification API here
impl<const MAX_N: u64> MerkleMountainRange<MAX_N>
where
    [(); MAX_N.ilog2() as usize + 1]:,
{
    /// Create an empty instance
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn new() -> Self {
        Self {
            stack: [[0u8; OUT_LEN]; MAX_N.ilog2() as usize + 1],
            num_leaves: 0,
        }
    }

    /// Create a new instance from previously collected peaks.
    ///
    /// Returns `None` if input is invalid.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn from_peaks(peaks: MmrPeaks<MAX_N>) -> Option<Self> {
        let mut result = Self {
            stack: [[0u8; OUT_LEN]; MAX_N.ilog2() as usize + 1],
            num_leaves: peaks.num_leaves,
        };

        // Convert peaks (where all occupied entries are all at the beginning of the list instead)
        // to stack (where occupied entries are at corresponding offsets)
        let mut stack_bits = peaks.num_leaves;
        let mut peaks_offset = 0;

        while stack_bits != 0 {
            let stack_offset = stack_bits.trailing_zeros();

            *result.stack.get_mut(stack_offset as usize)? = *peaks.peaks.get(peaks_offset)?;

            peaks_offset += 1;
            // Clear the lowest set bit
            stack_bits &= !(1 << stack_offset);
        }

        Some(result)
    }

    /// Get number of leaves aggregated in Merkle Mountain Range so far
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn num_leaves(&self) -> u64 {
        self.num_leaves
    }

    /// Calculate the root of Merkle Mountain Range.
    ///
    /// In case MMR contains a single leaf hash, that leaf hash is returned, `None` is returned if
    /// there were no leafs added yet.
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn root(&self) -> Option<[u8; OUT_LEN]> {
        if self.num_leaves == 0 {
            // If no leaves were provided
            return None;
        }

        let mut root;
        let mut stack_bits = self.num_leaves;
        {
            let lowest_active_level = stack_bits.trailing_zeros() as usize;
            // SAFETY: Active level must have been set successfully before, hence it exists
            root = *unsafe { self.stack.get_unchecked(lowest_active_level) };
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
            let lowest_active_level_item = unsafe { self.stack.get_unchecked(lowest_active_level) };

            root = hash_pair(lowest_active_level_item, &root);
        }

        Some(root)
    }

    /// Get peaks of Merkle Mountain Range
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn peaks(&self) -> MmrPeaks<MAX_N> {
        let mut result = MmrPeaks {
            peaks: [[0u8; OUT_LEN]; MAX_N.ilog2() as usize + 1],
            num_leaves: self.num_leaves,
        };

        // Convert stack (where occupied entries are at corresponding offsets) to peaks (where all
        // occupied entries are all at the beginning of the list instead)
        let mut stack_bits = self.num_leaves;
        let mut peaks_offset = 0;
        while stack_bits != 0 {
            let stack_offset = stack_bits.trailing_zeros();

            // SAFETY: Stack offset is always within the range of stack and peaks, this is
            // guaranteed by internal invariants of the MMR
            *unsafe { result.peaks.get_unchecked_mut(peaks_offset) } =
                *unsafe { self.stack.get_unchecked(stack_offset as usize) };

            peaks_offset += 1;
            // Clear the lowest set bit
            stack_bits &= !(1 << stack_offset);
        }

        result
    }

    /// Add leaf to Merkle Mountain Range.
    ///
    /// There is a more efficient version [`Self::add_leaves()`] in case multiple leaves are
    /// available.
    ///
    /// Returns `true` on success, `false` if too many leafs were added.
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn add_leaf(&mut self, leaf: &[u8; OUT_LEN]) -> bool {
        // How many leaves were processed so far
        if self.num_leaves >= MAX_N {
            return false;
        }

        let mut current = *leaf;

        // Every bit set to `1` corresponds to an active Merkle Tree level
        let lowest_active_levels = self.num_leaves.trailing_ones() as usize;
        for item in self.stack.iter().take(lowest_active_levels) {
            current = hash_pair(item, &current);
        }

        // Place the current hash at the first inactive level
        self.stack[lowest_active_levels] = current;
        self.num_leaves += 1;

        true
    }

    /// Add many leaves to Merkle Mountain Range.
    ///
    /// This is a more efficient version of [`Self::add_leaf()`] in case multiple leaves are
    /// available.
    ///
    /// Returns `true` on success, `false` if too many leaves were added.
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn add_leaves<'a, Item, Iter>(&mut self, leaves: Iter) -> bool
    where
        Item: Into<[u8; OUT_LEN]>,
        Iter: IntoIterator<Item = Item> + 'a,
    {
        // TODO: This can be optimized further
        for leaf in leaves {
            // How many leaves were processed so far
            if self.num_leaves >= MAX_N {
                return false;
            }

            let mut current = leaf.into();

            // Every bit set to `1` corresponds to an active Merkle Tree level
            let lowest_active_levels = self.num_leaves.trailing_ones() as usize;
            for item in self.stack.iter().take(lowest_active_levels) {
                current = hash_pair(item, &current);
            }

            // Place the current hash at the first inactive level
            self.stack[lowest_active_levels] = current;
            self.num_leaves += 1;
        }

        true
    }

    /// Add leaf to Merkle Mountain Range and generate inclusion proof.
    ///
    /// Returns `Some((root, proof))` on success, `None` if too many leafs were added.
    #[inline]
    #[cfg(feature = "alloc")]
    pub fn add_leaf_and_compute_proof(
        &mut self,
        leaf: &[u8; OUT_LEN],
    ) -> Option<([u8; OUT_LEN], Vec<[u8; OUT_LEN]>)> {
        // SAFETY: Inner value is `MaybeUninit`
        let mut proof = unsafe {
            Box::<[MaybeUninit<[u8; OUT_LEN]>; MAX_N.ilog2() as usize + 1]>::new_uninit()
                .assume_init()
        };

        let (root, proof_length) = self.add_leaf_and_compute_proof_inner(leaf, &mut proof)?;

        let proof_capacity = proof.len();
        let proof = Box::into_raw(proof);
        // SAFETY: Points to correctly allocated memory where `proof_length` elements were
        // initialized
        let proof = unsafe {
            Vec::from_raw_parts(proof.cast::<[u8; OUT_LEN]>(), proof_length, proof_capacity)
        };

        Some((root, proof))
    }

    /// Add leaf to Merkle Mountain Range and generate inclusion proof.
    ///
    /// Returns `Some((root, proof))` on success, `None` if too many leafs were added.
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn add_leaf_and_compute_proof_in<'proof>(
        &mut self,
        leaf: &[u8; OUT_LEN],
        proof: &'proof mut [MaybeUninit<[u8; OUT_LEN]>; MAX_N.ilog2() as usize + 1],
    ) -> Option<([u8; OUT_LEN], &'proof mut [[u8; OUT_LEN]])> {
        let (root, proof_length) = self.add_leaf_and_compute_proof_inner(leaf, proof)?;

        // SAFETY: Just correctly initialized `proof_length` elements
        let proof = unsafe {
            proof
                .split_at_mut_unchecked(proof_length)
                .0
                .assume_init_mut()
        };

        Some((root, proof))
    }

    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn add_leaf_and_compute_proof_inner(
        &mut self,
        leaf: &[u8; OUT_LEN],
        proof: &mut [MaybeUninit<[u8; OUT_LEN]>; MAX_N.ilog2() as usize + 1],
    ) -> Option<([u8; OUT_LEN], usize)> {
        let mut proof_length = 0;

        let current_target_level;
        let mut position = self.num_leaves;

        {
            // How many leaves were processed so far
            if self.num_leaves >= MAX_N {
                return None;
            }

            let mut current = *leaf;

            // Every bit set to `1` corresponds to an active Merkle Tree level
            let lowest_active_levels = self.num_leaves.trailing_ones() as usize;

            for item in self.stack.iter().take(lowest_active_levels) {
                // If at the target leaf index, need to collect the proof
                // SAFETY: Method signature guarantees upper bound of the proof length
                unsafe { proof.get_unchecked_mut(proof_length) }.write(*item);
                proof_length += 1;

                current = hash_pair(item, &current);

                // Move up the tree
                position /= 2;
            }

            current_target_level = lowest_active_levels;

            // Place the current hash at the first inactive level
            self.stack[lowest_active_levels] = current;
            self.num_leaves += 1;
        }

        let mut root;
        let mut stack_bits = self.num_leaves;

        {
            let lowest_active_level = stack_bits.trailing_zeros() as usize;
            // SAFETY: Active level must have been set successfully before, hence it exists
            root = *unsafe { self.stack.get_unchecked(lowest_active_level) };
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
            let lowest_active_level_item = unsafe { self.stack.get_unchecked(lowest_active_level) };

            if lowest_active_level > current_target_level
                || (lowest_active_level == current_target_level
                    && (position % 2 != 0)
                    && !merged_peaks)
            {
                // SAFETY: Method signature guarantees upper bound of the proof length
                unsafe { proof.get_unchecked_mut(proof_length) }.write(*lowest_active_level_item);
                proof_length += 1;
                merged_peaks = false;
            } else if lowest_active_level == current_target_level {
                // SAFETY: Method signature guarantees upper bound of the proof length
                unsafe { proof.get_unchecked_mut(proof_length) }.write(root);
                proof_length += 1;
                merged_peaks = false;
            } else {
                // Not collecting proof because of the need to merge peaks of an unbalanced tree
                merged_peaks = true;
            }

            // Collect the lowest peak into the proof
            root = hash_pair(lowest_active_level_item, &root);

            position /= 2;
        }

        Some((root, proof_length))
    }

    /// Verify a Merkle proof for a leaf at the given index.
    ///
    /// NOTE: `MAX_N` constant doesn't matter here and can be anything that is `>= 1`.
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn verify(
        root: &[u8; OUT_LEN],
        proof: &[[u8; OUT_LEN]],
        leaf_index: u64,
        leaf: [u8; OUT_LEN],
        num_leaves: u64,
    ) -> bool {
        UnbalancedHashedMerkleTree::verify(root, proof, leaf_index, leaf, num_leaves)
    }
}
