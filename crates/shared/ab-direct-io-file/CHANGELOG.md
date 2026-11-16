# 0.2.0

Breaking changes:

* Nightly Rust is required again
* Removed `AlignedPage` and `DISK_PAGE_SIZE`, use `OwnedPages` and `SharedPages` instead, whose alignment
  `OwnedPages::ALIGN` is the size of a page
* Removed `DirectIoFile::read_exact_at()`, `DirectIoFile::write_all_at()`, `DirectIoFile::read_exact_at_raw()` and
  `DirectIoFile::write_all_at_raw()`, use `DirectIoFile::read_at()`, `DirectIoFile::read_pages_at()` and
  `DirectIoFile::write_at()` instead
* Removed `DirectIoFile::file()`

New features:

* `DirectIoFile::read_at()`, `DirectIoFile::read_pages_at()` and `DirectIoFile::write_at()` take aligned buffers of
  `ab-aligned-buffer` by value and return them back, also on error with `IoBufError`, such that reads and writes of
  whole pages happen directly from and into the buffers without copying or locking, and allocations can be reused.
  Allocation failures are returned as `DirectIoError::AllocationFailed` instead of panicking
* `OwnedPages` and `SharedPages` type aliases for page-aligned buffers, whose alignment is the size of a page

Improvements:

* `DirectIoFile` no longer allocates a 1 MiB scratch buffer per instance
* Reads and writes no longer copy through a scratch buffer or lock a per-instance mutex, only partially written edge
  pages are read-modify-written through a page on the stack, serialized with each other, so writes never allocate

# 0.1.1

Improvements:

* Support for stable Rust

# 0.1.0

Initial release
