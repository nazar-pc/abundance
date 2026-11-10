# 0.1.2

New features:

* `OwnedAlignedBuffer::as_u128_slice()` returns the contents as whole `u128`s
* `OwnedAlignedBuffer::MAX_CAPACITY` is the largest capacity of a buffer, which is smaller than `u32::MAX` on 32-bit
  targets

Improvements:

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
