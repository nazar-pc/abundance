//! Sparse Merkle Tree and related data structures.
//!
//! Sparse Merkle Tree is essentially a huge Balanced Merkle Tree, where most of the leaves are
//! empty. By "empty" here we mean `[0u8; 32]`. To optimize proofs and their verification, the
//! hashing function is customized and returns `[0u8; 32]` when both left and right branch are
//! `[0u8; 32]`, otherwise BLAKE3 hash is used like in a Balanced Merkle Tree.

use crate::{OUT_LEN, hash_pair};

/// Ensuring only supported `NUM_BITS` can be specified for [`SparseMerkleTree`].
///
/// This is essentially a workaround for the current Rust type system constraints that do not allow
/// a nicer way to do the same thing at compile time.
pub const fn ensure_supported_bits(bits: u8) -> usize {
    assert!(
        bits <= 128,
        "This Sparse Merkle Tree doesn't support more than 2^128 leaves"
    );

    assert!(
        bits != 0,
        "This Sparse Merkle Tree must have more than one leaf"
    );

    0
}

/// Sparse Merkle Tree Leaf
#[derive(Debug)]
pub enum Leaf<'a> {
    // TODO: Batch of leaves for efficiently, especially with SIMD?
    /// Leaf is occupied by a value
    Occupied {
        /// Leaf value
        leaf: &'a [u8; OUT_LEN],
    },
    /// Leaf is empty
    Empty {
        /// Number of consecutive empty leaves
        skip_count: u128,
    },
}

impl<'a> From<&'a [u8; OUT_LEN]> for Leaf<'a> {
    #[inline(always)]
    fn from(leaf: &'a [u8; OUT_LEN]) -> Self {
        Self::Occupied { leaf }
    }
}

// TODO: A version that can hold intermediate nodes in memory, efficiently update leaves, etc.
/// Sparse Merkle Tree variant that has hash-sized leaves, with most leaves being empty
/// (have value `[0u8; 32]`).
///
/// In contrast to a proper Balanced Merkle Tree, constant `BITS` here specifies the max number of
/// leaves hypothetically possible in a tree (2^BITS, often untractable), rather than the number of
/// non-empty leaves actually present.
#[derive(Debug)]
pub struct SparseMerkleTree<const BITS: u8>;

// TODO: Optimize by implementing SIMD-accelerated hashing of multiple values:
//  https://github.com/BLAKE3-team/BLAKE3/issues/478
impl<const BITS: u8> SparseMerkleTree<BITS>
where
    [(); ensure_supported_bits(BITS)]:,
{
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
    pub fn compute_root_only<'a, Iter>(leaves: Iter) -> Option<[u8; OUT_LEN]>
    where
        [(); BITS as usize + 1]:,
        Iter: IntoIterator<Item = Leaf<'a>> + 'a,
    {
        // Stack of intermediate nodes per tree level
        let mut stack = [[0u8; OUT_LEN]; BITS as usize + 1];
        let mut processed_some = false;
        let mut num_leaves = 0_u128;

        for leaf in leaves {
            if u32::from(BITS) < u128::BITS {
                // How many leaves were processed so far
                if num_leaves == 2u128.pow(u32::from(BITS)) {
                    return None;
                }
            } else {
                // For `BITS == u128::BITS` `num_leaves` will wrap around back to zero right at the
                // very end
                if processed_some && num_leaves == 0 {
                    return None;
                }
                processed_some = true;
            }

            match leaf {
                Leaf::Occupied { leaf } => {
                    let mut current = *leaf;

                    // Every bit set to `1` corresponds to an active Merkle Tree level
                    let lowest_active_levels = num_leaves.trailing_ones() as usize;
                    for item in stack.iter().take(lowest_active_levels) {
                        current = hash_pair(item, &current);
                    }

                    // Place the current hash at the first inactive level
                    // SAFETY: Number of lowest active levels corresponds to the number of inserted
                    // elements, which in turn is checked above to fit into 2^BITS, while `BITS`
                    // generic in turn ensured sufficient stack size
                    *unsafe { stack.get_unchecked_mut(lowest_active_levels) } = current;
                    // Wrapping is needed for `BITS == u128::BITS`, where number of leaves narrowly
                    // doesn't fit into `u128` itself
                    num_leaves = num_leaves.wrapping_add(1);
                }
                Leaf::Empty { skip_count } => {
                    num_leaves =
                        Self::skip_leaves(&mut stack, &mut processed_some, num_leaves, skip_count)?;
                }
            }
        }

        if u32::from(BITS) < u128::BITS {
            Self::skip_leaves(
                &mut stack,
                &mut processed_some,
                num_leaves,
                2u128.pow(u32::from(BITS)) - num_leaves,
            )?;
        } else if processed_some && num_leaves != 0 {
            // For `BITS == u128::BITS` `num_leaves` will wrap around back to zero right at the
            // very end, so we reverse the mechanism here
            Self::skip_leaves(
                &mut stack,
                &mut processed_some,
                num_leaves,
                0u128.wrapping_sub(num_leaves),
            )?;
        }

        Some(stack[BITS as usize])
    }

    /// Returns updated number of leaves
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn skip_leaves(
        stack: &mut [[u8; OUT_LEN]; BITS as usize + 1],
        processed_some: &mut bool,
        mut num_leaves: u128,
        mut skip_count: u128,
    ) -> Option<u128>
    where
        [(); BITS as usize + 1]:,
    {
        const ZERO: [u8; OUT_LEN] = [0; OUT_LEN];

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
            let mut current = ZERO;
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
                if !(item == &ZERO && current == ZERO) {
                    current = hash_pair(item, &current);
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
    /// Leaf can either be leaf value for a leaf that is occupied or `[0; 32]` for a leaf that is
    /// supposed to be empty.
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn verify(
        root: &[u8; OUT_LEN],
        proof: &[[u8; OUT_LEN]; BITS as usize],
        leaf_index: u128,
        leaf: [u8; OUT_LEN],
    ) -> bool
    where
        [(); BITS as usize]:,
    {
        // For `BITS == u128::BITS` any index is valid by definition
        if u32::from(BITS) < u128::BITS && leaf_index >= 2u128.pow(u32::from(BITS)) {
            return false;
        }

        let mut computed_root = leaf;

        let mut position = leaf_index;
        for hash in proof {
            computed_root = if position.is_multiple_of(2) {
                hash_pair(&computed_root, hash)
            } else {
                hash_pair(hash, &computed_root)
            };

            position /= 2;
        }

        root == &computed_root
    }
}
