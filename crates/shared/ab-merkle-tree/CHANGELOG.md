# 0.4.0

Breaking changes:

* Newer nightly Rust is required, `nightly-2026-11-28` is guaranteed to work, later versions will likely work too

# 0.3.0

Trees are generic over the new `MerkleHasher` trait and BLAKE3 is behind the optional `blake3` feature, trees in the
`blake3` module produce the same roots and proofs as 0.2.0. Unbalanced and sparse trees hash many pairs at once with
SIMD, which is up to 5 times faster with BLAKE3 on x86-64 with AVX-512. Several cases that produced wrong roots,
rejected valid proofs or panicked are fixed or rejected at compile time. To upgrade, enable the `blake3` feature, use
aliases from the `blake3` module and `*_dynamic_len*()` methods for iterators that are not `TrustedLen`.

Breaking changes:

* Nightly Rust is required, `nightly-2026-11-19` is guaranteed to work, later versions will likely work too
* All trees are generic over the hasher, aliases of trees with the default BLAKE3 hasher are in the new `blake3` module
* BLAKE3 is behind the `blake3` feature, which is not enabled by default
* `UnbalancedMerkleTree` methods that take an iterator of leaves require it to be `TrustedLen`, `*_dynamic_len*()`
  variants accept any iterator
* `MmrPeaks`, `MerkleMountainRangeBytes`, `sparse::Leaf` and `balanced::ProofsIterator` are generic over the hasher too
* `INNER_NODE_DOMAIN_SEPARATOR`, `hash_pair()` and `hash_pair_block()` were removed, the default key is available as
  `blake3::Blake3MerkleHasher::KEY`

New features:

* Customizable hasher with the new `MerkleHasher` trait, which defines the size of nodes (`OUT_LEN`) and hashes one or
  many pairs of nodes at once, such that hash functions with other output sizes and SIMD-accelerated hashing of many
  pairs can be used
* Hashers that don't depend on the tree size are created with `Default`, hashers that depend on it implement the new
  `const` trait `NewMerkleHasherForTreeSize`, which is implemented for all hashers that implement `Default`;
  `MerkleMountainRange`, `SparseMerkleTree` and `UnbalancedMerkleTree::*_dynamic_len*()` only support the former,
  `BalancedMerkleTree` and `UnbalancedMerkleTree::compute_root_only_array()` require a `const` implementation
* `blake3::Blake3MerkleHasher`, which produces the same roots and proofs as 0.2.0, other BLAKE3 keys require a custom
  `MerkleHasher`
* `MerkleHasher::hash_single_leaf()` customizes the root of an `UnbalancedMerkleTree` or `MerkleMountainRange` with a
  single leaf, which is the leaf itself by default, so hashers that depend on the tree size can commit to it in that
  case too
* `UnbalancedMerkleTree::compute_root_only_sparse()` computes the root of a tree whose leaves are mostly zeroes, it
  hashes runs of zero leaves as whole subtrees, so the cost depends on the number of non-zero leaves and the height of
  the tree rather than on the number of leaves
* `MerkleHasher::PARALLEL_PAIRS` is the number of pairs `MerkleHasher::hash_pairs()` hashes in parallel (1 by
  default, a power of two up to 64), which trees use to size batches of pairs, `blake3::Blake3MerkleHasher` sets it to
  the number of blocks BLAKE3 can hash in parallel on the target

Improvements:

* BLAKE3 is optional, so users of other hash functions don't need to compile it
* `BalancedMerkleTree::compute_root_only()` hashes batches of `MerkleHasher::PARALLEL_PAIRS` pairs rather than 16, so
  it uses less stack with hashers that hash fewer pairs in parallel, including BLAKE3 on targets with narrower or no
  SIMD
* `UnbalancedMerkleTree::compute_root_only*()` (except `compute_root_only_sparse()`) hash leaves as perfect subtrees
  of up to 8 times `MerkleHasher::PARALLEL_PAIRS` leaves with `MerkleHasher::hash_pairs()`, which is about 4 times
  faster for 256 leaves and more and 2-3 times faster for 16-32 leaves with BLAKE3 on x86-64 with AVX-512
* `UnbalancedMerkleTree::compute_root_and_proof*()` hash leaves the same way when `MAX_N` is at least 8, which is 3-3.8
  times faster for 256 leaves and more with BLAKE3 on x86-64 with AVX-512, about as fast as computing the root alone
* `SparseMerkleTree::compute_root_only()` hashes runs of consecutive occupied leaves the same way, which is 3.5-5 times
  faster for 1000 consecutive leaves and more with BLAKE3 on x86-64 with AVX-512
* `SparseMerkleTree::compute_root_only()` iterates over leaves with `Iterator::try_for_each()` with hashers that hash
  pairs in parallel, which is about 2 times faster with BLAKE3 on x86-64 with AVX-512 for full trees with leaves from a
  `FlatMap` of `Chain` iterators like the ones of state roots
* `SparseMerkleTree::compute_root_only()` hashes subtrees of occupied leaves with empty siblings up to the largest
  subtree that contains no other occupied leaves, `MerkleHasher::PARALLEL_PAIRS` subtrees at a time with
  `MerkleHasher::hash_pairs()`, which is 3-5 times faster for isolated leaves at random positions of a 128-bit tree with
  BLAKE3 on x86-64 with AVX-512

Fixes:

* `BalancedMerkleTree::compute_root_only()` and `BalancedMerkleTree::verify()` no longer compile with `N` that is not a
  power of two or is smaller than 2, previously they did in builds without debug info and ignored some of the leaves or
  panicked at runtime
* `MerkleMountainRange::from_peaks()` now returns `None` for more than `MAX_N` leaves, previously it accepted them as
  long as stack and peaks had space for them
* `SparseMerkleTree::compute_root_only()` and `SparseMerkleTree::verify()` with unsupported `BITS` and
  `UnbalancedMerkleTree::compute_root_only_array()` with an empty array no longer compile when called from generic code
  in crates with `generic_const_args`, previously they returned wrong results or panicked at runtime
* `SparseMerkleTree::verify()` now accepts proofs of empty leaves next to an empty sibling, previously it hashed two
  empty nodes together instead of treating their parent as empty like `SparseMerkleTree::compute_root_only()` does

# 0.2.0

Breaking changes:

* Migrate from `generic_const_exprs` to `generic_const_args` family of nightly features

Improvements:

* Faster balanced Merkle Tree proof generation
* Extended MMR/SMT benchmarks/tests

Fixes:

* Fix `MERKLE_MOUNTAIN_RANGE_BYTES_SIZE` not computed correctly for non-power-of-to sizes
* Reject `MAX_N == 1` in MMR to avoid out-of-bounds reads (it is quite useless to justify proper support for this
  special case)

# 0.1.0

Initial release
