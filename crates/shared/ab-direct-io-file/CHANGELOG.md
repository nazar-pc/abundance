# 0.2.0

Breaking changes:

* Nightly Rust is required again
* `DirectIoFile::read_exact_at_raw()` takes `BorrowedCursor<'_, AlignedPage>` instead of
  `&mut [MaybeUninit<AlignedPage>]`, which works with both initialized and uninitialized memory without treating
  uninitialized memory as `&mut [u8]` internally
* Removed `AlignedPage::as_uninit_slice_mut()`, which was unsound: safe code could write uninitialized values through
  it and then read them through the original slice

Fixes:

* `DirectIoFile::write_all_at()` no longer panics on small page-aligned writes, which were incorrectly treated as
  writes of whole pages

# 0.1.1

Improvements:

* Support for stable Rust

# 0.1.0

Initial release
