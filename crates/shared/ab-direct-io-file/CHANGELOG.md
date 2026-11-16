# 0.2.0

Breaking changes:

* Nightly Rust is required again
* Removed `DISK_PAGE_SIZE`, `AlignedPage::SIZE` is the same constant
* `DirectIoFile::read_exact_at_raw()` takes `BorrowedCursor<'_, AlignedPage>` instead of
  `&mut [MaybeUninit<AlignedPage>]`, which works with both initialized and uninitialized memory without treating
  uninitialized memory as `&mut [u8]` internally
* Removed `AlignedPage::as_uninit_slice_mut()`, which was unsound: safe code could write uninitialized values through
  it and then read them through the original slice

New features:

* `DirectIoFile::read_at()`, `DirectIoFile::read_pages_at()` and `DirectIoFile::write_at()` take aligned buffers of
  `ab-aligned-buffer` by value and return them back, also on error with `IoBufError`, such that reads and writes of
  whole pages happen directly from and into the buffers without copying or locking, and allocations can be reused.
  Allocation failures are returned as `DirectIoError::AllocationFailed` instead of panicking
* `OwnedPages` and `SharedPages` type aliases for page-aligned buffers, whose alignment is the size of a page

Fixes:

* `DirectIoFile::write_all_at()` no longer panics on small page-aligned writes, which were incorrectly treated as
  writes of whole pages

# 0.1.1

Improvements:

* Support for stable Rust

# 0.1.0

Initial release
