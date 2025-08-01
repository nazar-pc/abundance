use crate::{hash_pair, hash_pair_block, hash_pairs};
use ab_blake3::{BLOCK_LEN, OUT_LEN};
#[cfg(feature = "alloc")]
use alloc::boxed::Box;
use core::iter::TrustedLen;
use core::mem;
use core::mem::MaybeUninit;
use core::num::NonZero;

/// Optimal number of blocks for hashing at once to saturate BLAKE3 SIMD on any hardware
const BATCH_HASH_NUM_BLOCKS: usize = 16;
/// Number of leaves that corresponds to [`BATCH_HASH_NUM_BLOCKS`]
const BATCH_HASH_NUM_LEAVES: usize = BATCH_HASH_NUM_BLOCKS * BLOCK_LEN / OUT_LEN;

/// Inner function used in [`BalancedMerkleTree::compute_root_only()`] for stack allocation, only
/// public due to use in generic bounds
pub const fn compute_root_only_large_stack_size(n: usize) -> usize {
    // For small trees the large stack is not used, so the returned value does not matter as long as
    // it compiles
    if n < BATCH_HASH_NUM_LEAVES {
        return 1;
    }

    (n / BATCH_HASH_NUM_LEAVES).ilog2() as usize + 1
}

/// Ensuring only supported `N` can be specified for [`BalancedMerkleTree`].
///
/// This is essentially a workaround for the current Rust type system constraints that do not allow
/// a nicer way to do the same thing at compile time.
pub const fn ensure_supported_n(n: usize) -> usize {
    assert!(
        n.is_power_of_two(),
        "Balanced Merkle Tree must have a number of leaves that is a power of 2"
    );

    assert!(
        n > 1,
        "This Balanced Merkle Tree must have more than one leaf"
    );

    0
}

/// Merkle Tree variant that has hash-sized leaves and is fully balanced according to configured
/// generic parameter.
///
/// This can be considered a general case of [`UnbalancedMerkleTree`]. The root and proofs are
/// identical for both in case the number of leaves is a power of two. For the number of leaves that
/// is a power of two [`UnbalancedMerkleTree`] is useful when a single proof needs to be generated
/// and the number of leaves is very large (it can generate proofs with very little RAM usage
/// compared to this version).
///
/// [`UnbalancedMerkleTree`]: crate::unbalanced::UnbalancedMerkleTree
///
/// This Merkle Tree implementation is best suited for use cases when proofs for all (or most) of
/// the elements need to be generated and the whole tree easily fits into memory. It can also be
/// constructed and proofs can be generated efficiently without heap allocations.
///
/// With all parameters of the tree known statically, it results in the most efficient version of
/// the code being generated for a given set of parameters.
#[derive(Debug)]
pub struct BalancedMerkleTree<'a, const N: usize>
where
    [(); N - 1]:,
{
    leaves: &'a [[u8; OUT_LEN]],
    // This tree doesn't include leaves because we have them in `leaves` field
    tree: [[u8; OUT_LEN]; N - 1],
}

// TODO: Optimize by implementing SIMD-accelerated hashing of multiple values:
//  https://github.com/BLAKE3-team/BLAKE3/issues/478
impl<'a, const N: usize> BalancedMerkleTree<'a, N>
where
    [(); N - 1]:,
    [(); ensure_supported_n(N)]:,
{
    /// Create a new tree from a fixed set of elements.
    ///
    /// The data structure is statically allocated and might be too large to fit on the stack!
    /// If that is the case, use `new_boxed()` method.
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn new(leaves: &'a [[u8; OUT_LEN]; N]) -> Self {
        let mut tree = [MaybeUninit::<[u8; OUT_LEN]>::uninit(); _];

        Self::init_internal(leaves, &mut tree);

        Self {
            leaves,
            // SAFETY: Statically guaranteed for all elements to be initialized
            tree: unsafe { tree.transpose().assume_init() },
        }
    }

    /// Like [`Self::new()`], but used pre-allocated memory for instantiation
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn new_in<'b>(
        instance: &'b mut MaybeUninit<Self>,
        leaves: &'a [[u8; OUT_LEN]; N],
    ) -> &'b mut Self {
        let instance_ptr = instance.as_mut_ptr();
        // SAFETY: Valid and correctly aligned non-null pointer
        unsafe {
            (&raw mut (*instance_ptr).leaves).write(leaves);
        }
        let tree = {
            // SAFETY: Valid and correctly aligned non-null pointer
            let tree_ptr = unsafe { &raw mut (*instance_ptr).tree };
            // SAFETY: Allocated and correctly aligned uninitialized data
            unsafe {
                tree_ptr
                    .cast::<[MaybeUninit<[u8; OUT_LEN]>; N - 1]>()
                    .as_mut_unchecked()
            }
        };

        Self::init_internal(leaves, tree);

        // SAFETY: Initialized field by field above
        unsafe { instance.assume_init_mut() }
    }

    /// Like [`Self::new()`], but creates heap-allocated instance, avoiding excessive stack usage
    /// for large trees
    #[cfg(feature = "alloc")]
    pub fn new_boxed(leaves: &'a [[u8; OUT_LEN]; N]) -> Box<Self> {
        let mut instance = Box::<Self>::new_uninit();

        Self::new_in(&mut instance, leaves);

        // SAFETY: Initialized by constructor above
        unsafe { instance.assume_init() }
    }

    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn init_internal(leaves: &[[u8; OUT_LEN]; N], tree: &mut [MaybeUninit<[u8; OUT_LEN]>; N - 1]) {
        let mut tree_hashes = tree.as_mut_slice();
        let mut level_hashes = leaves.as_slice();

        while level_hashes.len() > 1 {
            let num_pairs = level_hashes.len() / 2;
            let parent_hashes;
            // SAFETY: The size of the tree is statically known to match the number of leaves and
            // levels of hashes
            (parent_hashes, tree_hashes) = unsafe { tree_hashes.split_at_mut_unchecked(num_pairs) };

            if parent_hashes.len().is_multiple_of(BATCH_HASH_NUM_BLOCKS) {
                // SAFETY: Just checked to be a multiple of chunk size and not empty
                let parent_hashes_chunks =
                    unsafe { parent_hashes.as_chunks_unchecked_mut::<BATCH_HASH_NUM_BLOCKS>() };
                for (pairs, hashes) in level_hashes
                    .as_chunks::<BATCH_HASH_NUM_LEAVES>()
                    .0
                    .iter()
                    .zip(parent_hashes_chunks)
                {
                    // TODO: Would be nice to have a convenient method for this:
                    //  https://github.com/rust-lang/rust/issues/96097#issuecomment-3133515169
                    // SAFETY: Identical layout
                    let hashes = unsafe {
                        mem::transmute::<
                            &mut [MaybeUninit<[u8; OUT_LEN]>; BATCH_HASH_NUM_BLOCKS],
                            &mut MaybeUninit<[[u8; OUT_LEN]; BATCH_HASH_NUM_BLOCKS]>,
                        >(hashes)
                    };

                    // TODO: This memory copy is unfortunate, make hashing write into this memory
                    //  directly once blake3 API improves
                    hashes.write(hash_pairs(pairs));
                }
            } else {
                for (pair, parent_hash) in level_hashes
                    .as_chunks()
                    .0
                    .iter()
                    .zip(parent_hashes.iter_mut())
                {
                    // SAFETY: Same size and alignment
                    let pair = unsafe {
                        mem::transmute::<&[[u8; OUT_LEN]; BLOCK_LEN / OUT_LEN], &[u8; BLOCK_LEN]>(
                            pair,
                        )
                    };
                    parent_hash.write(hash_pair_block(pair));
                }
            }

            // SAFETY: Just initialized
            level_hashes = unsafe { parent_hashes.assume_init_ref() };
        }
    }

    // TODO: Method that generates not only root, but also proof, like Unbalanced Merkle Tree
    /// Compute Merkle Tree root.
    ///
    /// This is functionally equivalent to creating an instance first and calling [`Self::root()`]
    /// method, but is faster and avoids heap allocation when root is the only thing that is needed.
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn compute_root_only(leaves: &[[u8; OUT_LEN]; N]) -> [u8; OUT_LEN]
    where
        [(); N.ilog2() as usize + 1]:,
        [(); compute_root_only_large_stack_size(N)]:,
    {
        // Special case for small trees below optimal SIMD width
        match N {
            2 => {
                let [root] = hash_pairs(leaves);

                return root;
            }
            4 => {
                let hashes = hash_pairs::<2, _>(leaves);
                let [root] = hash_pairs(&hashes);

                return root;
            }
            8 => {
                let hashes = hash_pairs::<4, _>(leaves);
                let hashes = hash_pairs::<2, _>(&hashes);
                let [root] = hash_pairs(&hashes);

                return root;
            }
            16 => {
                let hashes = hash_pairs::<8, _>(leaves);
                let hashes = hash_pairs::<4, _>(&hashes);
                let hashes = hash_pairs::<2, _>(&hashes);
                let [root] = hash_pairs(&hashes);

                return root;
            }
            _ => {
                // We know this is the case
                assert!(N >= BATCH_HASH_NUM_LEAVES);
            }
        }

        // Stack of intermediate nodes per tree level. The logic here is the same as with a small
        // tree above, except we store `BATCH_HASH_NUM_BLOCKS` hashes per level and do a
        // post-processing step at the very end to collapse them into a single root hash.
        let mut stack =
            [[[0u8; OUT_LEN]; BATCH_HASH_NUM_BLOCKS]; compute_root_only_large_stack_size(N)];

        // This variable allows reusing and reducing stack usage instead of having a separate
        // `current` variable
        let mut parent_current = [[0u8; OUT_LEN]; BATCH_HASH_NUM_LEAVES];
        for (num_chunks, chunk_leaves) in leaves
            .as_chunks::<BATCH_HASH_NUM_LEAVES>()
            .0
            .iter()
            .enumerate()
        {
            let (_parent_half, current_half) = parent_current.split_at_mut(BATCH_HASH_NUM_BLOCKS);

            let current = hash_pairs::<BATCH_HASH_NUM_BLOCKS, _>(chunk_leaves);
            current_half.copy_from_slice(&current);

            // Every bit set to `1` corresponds to an active Merkle Tree level
            let lowest_active_levels = num_chunks.trailing_ones() as usize;
            for parent in &mut stack[..lowest_active_levels] {
                let (parent_half, _current_half) =
                    parent_current.split_at_mut(BATCH_HASH_NUM_BLOCKS);
                parent_half.copy_from_slice(parent);

                let current = hash_pairs::<BATCH_HASH_NUM_BLOCKS, _>(&parent_current);

                let (_parent_half, current_half) =
                    parent_current.split_at_mut(BATCH_HASH_NUM_BLOCKS);
                current_half.copy_from_slice(&current);
            }

            let (_parent_half, current_half) = parent_current.split_at_mut(BATCH_HASH_NUM_BLOCKS);

            // Place freshly computed 8 hashes into the first inactive level
            stack[lowest_active_levels].copy_from_slice(current_half);
        }

        let hashes = &mut stack[compute_root_only_large_stack_size(N) - 1];
        let hashes = hash_pairs::<{ BATCH_HASH_NUM_BLOCKS / 2 }, _>(hashes);
        let hashes = hash_pairs::<{ BATCH_HASH_NUM_BLOCKS / 4 }, _>(&hashes);
        let hashes = hash_pairs::<{ BATCH_HASH_NUM_BLOCKS / 8 }, _>(&hashes);
        let [root] = hash_pairs::<{ BATCH_HASH_NUM_BLOCKS / 16 }, _>(&hashes);

        root
    }

    /// Get the root of Merkle Tree
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn root(&self) -> [u8; OUT_LEN] {
        *self
            .tree
            .last()
            .or(self.leaves.last())
            .expect("There is always at least one leaf hash; qed")
    }

    /// Iterator over proofs in the same order as provided leaf hashes
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn all_proofs(
        &self,
    ) -> impl ExactSizeIterator<Item = [[u8; OUT_LEN]; N.ilog2() as usize]> + TrustedLen
    where
        [(); N.ilog2() as usize]:,
    {
        let iter = self.leaves.as_chunks().0.iter().enumerate().flat_map(
            |(pair_index, &[left_hash, right_hash])| {
                let mut left_proof = [MaybeUninit::<[u8; OUT_LEN]>::uninit(); N.ilog2() as usize];
                left_proof[0].write(right_hash);

                let left_proof = {
                    let (_, shared_proof) = left_proof.split_at_mut(1);

                    let mut tree_hashes = self.tree.as_slice();
                    let mut parent_position = pair_index;
                    let mut parent_level_size = N / 2;

                    for hash in shared_proof {
                        // The line below is a more efficient branchless version of this:
                        // let parent_other_position = if parent_position % 2 == 0 {
                        //     parent_position + 1
                        // } else {
                        //     parent_position - 1
                        // };
                        let parent_other_position = parent_position ^ 1;

                        // SAFETY: Statically guaranteed to be present by constructor
                        let other_hash =
                            unsafe { tree_hashes.get_unchecked(parent_other_position) };
                        hash.write(*other_hash);
                        (_, tree_hashes) = tree_hashes.split_at(parent_level_size);

                        parent_position /= 2;
                        parent_level_size /= 2;
                    }

                    // SAFETY: Just initialized
                    unsafe { left_proof.transpose().assume_init() }
                };

                let mut right_proof = left_proof;
                right_proof[0] = left_hash;

                [left_proof, right_proof]
            },
        );

        ProofsIterator { iter, len: N }
    }

    /// Verify previously generated proof
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn verify(
        root: &[u8; OUT_LEN],
        proof: &[[u8; OUT_LEN]; N.ilog2() as usize],
        leaf_index: usize,
        leaf: [u8; OUT_LEN],
    ) -> bool
    where
        [(); N.ilog2() as usize]:,
    {
        if leaf_index >= N {
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

struct ProofsIterator<Iter> {
    iter: Iter,
    len: usize,
}

impl<Iter> Iterator for ProofsIterator<Iter>
where
    Iter: Iterator,
{
    type Item = Iter::Item;

    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn next(&mut self) -> Option<Self::Item> {
        let item = self.iter.next();
        self.len = self.len.saturating_sub(1);
        item
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }

    #[inline(always)]
    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.len
    }

    #[inline(always)]
    fn last(self) -> Option<Self::Item>
    where
        Self: Sized,
    {
        self.iter.last()
    }

    #[inline(always)]
    fn advance_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        self.len = self.len.saturating_sub(n);
        self.iter.advance_by(n)
    }

    #[inline(always)]
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.len = self.len.saturating_sub(n.saturating_add(1));
        self.iter.nth(n)
    }
}

impl<Iter> ExactSizeIterator for ProofsIterator<Iter>
where
    Iter: Iterator,
{
    #[inline(always)]
    fn len(&self) -> usize {
        self.len
    }
}

unsafe impl<Iter> TrustedLen for ProofsIterator<Iter> where Iter: Iterator {}
