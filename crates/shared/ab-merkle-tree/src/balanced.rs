use crate::hash_pair;
#[cfg(feature = "alloc")]
use alloc::boxed::Box;
use blake3::OUT_LEN;
use core::iter;
use core::iter::TrustedLen;
use core::mem::MaybeUninit;

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
    // This tree doesn't include leaves because we know the size
    tree: [[u8; OUT_LEN]; N - 1],
}

// TODO: Optimize by implementing SIMD-accelerated hashing of multiple values:
//  https://github.com/BLAKE3-team/BLAKE3/issues/478
impl<'a, const N: usize> BalancedMerkleTree<'a, N>
where
    [(); N - 1]:,
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

        let mut pair = [0u8; OUT_LEN * 2];
        while level_hashes.len() > 1 {
            let num_pairs = level_hashes.len() / 2;
            let parent_hashes;
            // SAFETY: The size of the tree is statically known to match the number of leaves and
            // levels of hashes
            (parent_hashes, tree_hashes) = unsafe { tree_hashes.split_at_mut_unchecked(num_pairs) };

            for pair_index in 0..num_pairs {
                // SAFETY: Entry is statically known to be present
                let left_hash = unsafe { level_hashes.get_unchecked(pair_index * 2) };
                // SAFETY: Entry is statically known to be present
                let right_hash = unsafe { level_hashes.get_unchecked(pair_index * 2 + 1) };
                // SAFETY: Entry is statically known to be present
                let parent_hash = unsafe { parent_hashes.get_unchecked_mut(pair_index) };

                pair[..OUT_LEN].copy_from_slice(left_hash);
                pair[OUT_LEN..].copy_from_slice(right_hash);

                parent_hash.write(hash_pair(left_hash, right_hash));
            }

            // SAFETY: Just initialized
            level_hashes = unsafe { parent_hashes.assume_init_ref() };
        }
    }

    /// Compute Merkle Tree root.
    ///
    /// This is functionally equivalent to creating an instance first and calling [`Self::root()`]
    /// method, but is faster and avoids heap allocation when root is the only thing that is needed.
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn compute_root_only(leaves: &[[u8; OUT_LEN]; N]) -> [u8; OUT_LEN]
    where
        [(); N.ilog2() as usize + 1]:,
    {
        if leaves.len() == 1 {
            return leaves[0];
        }

        // Stack of intermediate nodes per tree level
        let mut stack = [[0u8; OUT_LEN]; N.ilog2() as usize + 1];

        for (num_leaves, &hash) in leaves.iter().enumerate() {
            let mut current = hash;

            // Every bit set to `1` corresponds to an active Merkle Tree level
            let lowest_active_levels = num_leaves.trailing_ones() as usize;
            for item in stack.iter().take(lowest_active_levels) {
                current = hash_pair(item, &current);
            }

            // Place the current hash at the first inactive level
            stack[lowest_active_levels] = current;
        }

        stack[N.ilog2() as usize]
    }

    /// Get the root of Merkle Tree.
    ///
    /// In case a tree contains a single leaf hash, that leaf hash is returned.
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
        let iter = self
            .leaves
            .array_chunks()
            .enumerate()
            .flat_map(|(pair_index, &[left_hash, right_hash])| {
                let mut left_proof = [MaybeUninit::<[u8; OUT_LEN]>::uninit(); N.ilog2() as usize];
                left_proof[0].write(right_hash);

                let left_proof = {
                    let (_, shared_proof) = left_proof.split_at_mut(1);

                    let mut tree_hashes = self.tree.as_slice();
                    let mut parent_position = pair_index;
                    let mut parent_level_size = N / 2;

                    for hash in shared_proof {
                        let parent_other_position = if parent_position % 2 == 0 {
                            parent_position + 1
                        } else {
                            parent_position - 1
                        };
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
            })
            // Special case for a single leaf tree to make sure proof is returned, even if it is
            // empty
            .chain({
                let mut returned = false;

                iter::from_fn(move || {
                    if N == 1 && !returned {
                        returned = true;
                        Some([[0; OUT_LEN]; N.ilog2() as usize])
                    } else {
                        None
                    }
                })
            });

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
            computed_root = if position % 2 == 0 {
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
