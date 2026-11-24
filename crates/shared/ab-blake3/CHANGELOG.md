# 0.2.1

New features:

* `MAX_SIMD_DEGREE` is the maximum number of inputs hashed in parallel with SIMD on the target, such that callers
  can size batches of inputs accordingly

Improvements:

* `single_block_hash_many_exact()` and `single_block_keyed_hash_many_exact()` hash inputs beyond a multiple of 16 with
  narrower SIMD, previously they were hashed one by one; with the `no-panic` feature they are padded to 16 instead, so
  the compiler can prove that hashing doesn't panic
* Rust 1.95 or newer is required

# 0.2.0

Breaking changes:

* Remove workarounds no longer needed for the latest version or rust-gpu

# 0.1.0

Initial release
