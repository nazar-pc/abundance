# 0.2.0

Buffers are no longer limited to contents aligned to 16 bytes at the beginning of an allocation. Alignment of the
contents is a type parameter of buffers, allocations can be aligned to up to 4096 bytes (for direct I/O, for example),
contents can start at an offset within the allocation, and shared buffers can be sliced without copying. Buffers are
still 16 bytes in size. Empty buffers are not backed by an allocation, and owned buffers don't use atomic operations.
Existing code works with the default `Align16` alignment, but some call sites need type annotations.

Breaking changes:

* Rust 1.89 or newer is required
* `OwnedAlignedBuffer` and `SharedAlignedBuffer` have a type parameter for alignment of the contents, which is `Align16`
  by default, the same alignment as before
* `as_ptr()`, `as_mut_ptr()` and `as_u128_slice()` that return `u128`s are only available for alignment of the
  contents of at least 16 bytes, `as_byte_ptr()` and `as_mut_byte_ptr()` are available for any alignment
* Constructors, `MAX_CAPACITY`, `Default`, `EMPTY` and `empty_ref()` are generic over alignment of the contents, so
  call sites need type annotations where it can't be inferred

New features:

* `alignment` module with `Alignment` trait and `Align1` to `Align4096` types, which are used as type parameters for
  alignment of the contents of buffers and of their allocations
* `OwnedAlignedBuffer::with_capacity_aligned()` creates a buffer with allocation aligned to up to 4096 bytes, which is
  preserved when it grows, is cloned or converted into an owned buffer, `allocation_alignment()` returns it
* `into_start_align()` converts a buffer into one with a weaker alignment of the contents, `try_into_start_align()`
  converts into one with any alignment that the contents are actually aligned to
* `OwnedAlignedBuffer::EMPTY` and `SharedAlignedBuffer::EMPTY` constants, and `Default` for `OwnedAlignedBuffer`
* Contents of buffers can start at an offset within the allocation, `offset()` returns it, `capacity()` is counted
  from the beginning of the contents, and the largest capacity of a buffer is smaller by the offset:
  * `SharedAlignedBuffer::get_slice()` creates a buffer that references a part of the contents without copying them
  * `SharedAlignedBuffer::try_into_owned()` converts the last reference into an owned buffer with the same offset
  * `SharedAlignedBuffer::into_standalone()` copies the contents unless they already cover the whole allocation, so that
    a small buffer doesn't keep a large allocation alive
  * `OwnedAlignedBuffer::relocate()` moves the contents to a different offset within the allocation or into a new
    allocation with a different capacity and alignment at the same time
  * `OwnedAlignedBuffer::realign()` converts a buffer into one with a different alignment of the contents, moving them
    within the allocation if possible
* `truncate()` on both buffers
* `OwnedAlignedBuffer::zeroed()` creates a buffer filled with zeroes, allocating zeroed memory directly,
  `OwnedAlignedBuffer::zeroed_aligned()` does the same with a specified alignment of the allocation
* `OwnedAlignedBuffer::spare_capacity_mut()` returns capacity after the contents for writing into it
* `AlignedBuffer` trait implemented for both buffers, for APIs that accept either of them
* `OwnedAlignedBuffer::as_u128_slice()` returns the contents as whole `u128`s
* `OwnedAlignedBuffer::MAX_CAPACITY` is the largest capacity of a buffer, which is smaller than `u32::MAX` on 32-bit
  targets, the same for allocations with any alignment
* `SharedAlignedBuffer::capacity()` returns the capacity of the buffer, just like `OwnedAlignedBuffer::capacity()`

Improvements:

* Empty buffers are not backed by an allocation, cloning and dropping an empty `SharedAlignedBuffer` no longer modifies
  a reference counter shared by all empty buffers, which was contended between threads
* Converting an empty `OwnedAlignedBuffer` into `SharedAlignedBuffer` frees its allocation
* Dropping `OwnedAlignedBuffer` doesn't use atomic operations
* Reference counting of `SharedAlignedBuffer` uses the same memory orderings as `Arc`, which are weaker than before
* The reference counter of `SharedAlignedBuffer` is pointer-sized, and cloning panics before a 32-bit counter would wrap
  around
* 32-bit RISC-V and Arm targets are supported
* `OwnedAlignedBuffer::from_bytes()` allocates once with exactly the necessary capacity instead of reallocating an empty
  buffer, which also applies to `SharedAlignedBuffer::from_bytes()` and `SharedAlignedBuffer::into_owned()` of a buffer
  that is still shared
* Cloning `OwnedAlignedBuffer` allocates as much as its length rather than its capacity

Fixes:

* Growing a buffer to more than `isize::MAX` bytes panics, previously its size could overflow or break the contract of
  `realloc`
* Growing a buffer no longer asks for twice its capacity when that allocation would be larger than `isize::MAX` bytes,
  which panicked on 32-bit targets even though the new length fits
* `OwnedAlignedBuffer::append()` returns `false` when the new length is larger than `OwnedAlignedBuffer::MAX_CAPACITY`,
  previously it panicked on 32-bit targets

# 0.1.1

Improvements:

* Support for stable Rust

# 0.1.0

Initial release
