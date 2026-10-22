# 0.1.2

New features:

* `OwnedAlignedBuffer::as_u128_slice()` returns the contents as whole `u128`s

Improvements:

* The reference counter of `SharedAlignedBuffer` is pointer-sized, and cloning panics before a 32-bit counter would wrap
  around
* 32-bit RISC-V and Arm targets are supported

Fixes:

* Growing a buffer to more than `isize::MAX` bytes panics, previously its size could overflow or break the contract of
  `realloc`

# 0.1.1

Improvements:

* Support for stable Rust

# 0.1.0

Initial release
