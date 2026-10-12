# 0.3.0

Breaking changes:

* Metadata kinds for specific numbers of fields, variants and elements (like `Struct3` and `ArrayU8x32`) are removed,
  array lengths, recommended allocations and capacities always take 4 bytes and the remaining kinds are renumbered, so
  `METADATA` of most types changes
* `VariableBytes` is now an alias of `VariableElements<u8>`, and `FixedCapacityBytesU8/U16` and
  `FixedCapacityStringU8/U16` are replaced with `FixedCapacityElements<Element, CAPACITY>` (with the
  `FixedCapacityBytes<CAPACITY>` alias) and `FixedCapacityString<CAPACITY>`, which store the length as `u32`
* `IoTypeMetadataKind::compact()` produces valid metadata for structs with more than 10 named fields, previously it
  didn't

Fixes:

* `MaybeData` and `VariableElements` with zero-sized types no longer compile, previously `MaybeData` couldn't represent
  absence of data and `VariableElements` panicked
* Arrays of 4 GiB or larger or with `2^32` or more elements can no longer be used as `TrivialType`, previously their
  size and length were truncated
* `TrivialType` can now be derived for enums with 11 to 16 variants
* `IoTypeMetadataKind::type_details()` now returns the alignment of elements for `VariableElements<T>` with the default
  recommended allocation instead of 1

# 0.2.0

Breaking changes:

* Migrate from `generic_const_exprs` to `generic_const_args` family of nightly features

# 0.1.0

Initial release
