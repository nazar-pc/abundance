# 0.2.1

Fixes:

* `FixedCapacityBytesU8`/`FixedCapacityStringU8` with capacity above `u8::MAX` and
  `FixedCapacityBytesU16`/`FixedCapacityStringU16` with capacity above `u16::MAX` or odd capacity no longer compile,
  previously the length silently wrapped or `TrivialType` exposed the padding byte

# 0.2.0

Breaking changes:

* Migrate from `generic_const_exprs` to `generic_const_args` family of nightly features

# 0.1.0

Initial release
