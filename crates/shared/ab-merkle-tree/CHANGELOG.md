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
