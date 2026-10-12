# 0.3.0

Breaking changes:

* Metadata kinds for specific numbers of fields, variants and elements (like `Struct3` and `ArrayU8x32`) are removed,
  array lengths and recommended allocations always take 4 bytes and the remaining kinds are renumbered, so `METADATA` of
  most types changes
* `VariableBytes` is now an alias of `VariableElements<u8>`
* `IoTypeMetadataKind::compact()` produces valid metadata for structs with more than 10 named fields, previously it
  didn't

Fixes:

* `FixedCapacityBytesU8`/`FixedCapacityStringU8` with capacity above `u8::MAX` and
  `FixedCapacityBytesU16`/`FixedCapacityStringU16` with capacity above `u16::MAX` or odd capacity no longer compile,
  previously the length silently wrapped or `TrivialType` exposed the padding byte
* `MaybeData` and `VariableElements` with zero-sized types no longer compile, previously `MaybeData` couldn't represent
  absence of data and `VariableElements` panicked
* Arrays of 4 GiB or larger or with `2^32` or more elements can no longer be used as `TrivialType`, previously their
  size and length were truncated
* `FixedCapacityBytesU8::append()` and `FixedCapacityBytesU16::append()` now append bytes after existing contents,
  previously they overwrote the beginning of contents without changing the length
* `TrivialType` can now be derived for enums with 11 to 16 variants
* `IoTypeMetadataKind::type_details()` now returns the alignment of elements for `VariableElements<T>` with the default
  recommended allocation instead of 1

# 0.2.0

Breaking changes:

* Migrate from `generic_const_exprs` to `generic_const_args` family of nightly features

# 0.1.0

Initial release
