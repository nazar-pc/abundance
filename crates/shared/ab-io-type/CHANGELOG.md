# 0.3.0

Breaking changes:

* Metadata kinds for specific numbers of fields, variants and elements (like `Struct3` and `ArrayU8x32`) are removed,
  array lengths, recommended allocations and capacities always take 4 bytes, `TupleStruct` and `EnumNoFields` are merged
  into `Struct` and `Enum`, and the remaining kinds are renumbered, so `METADATA` of most types changes
* `VariableBytes` is now an alias of `VariableElements<u8>`, and `FixedCapacityBytesU8/U16` and
  `FixedCapacityStringU8/U16` are replaced with `FixedCapacityElements<Element, CAPACITY>` (with the
  `FixedCapacityBytes<CAPACITY>` alias) and `FixedCapacityString<CAPACITY>`, which store the length as `u32`
* `IoTypeMetadataKind::type_details()` rejects enums without variants, `VariableElements` inside other types and layouts
  with padding
* `IoTypeMetadataKind::type_details()` and `IoTypeMetadataKind::compact()` take the max nesting depth as a const generic
  parameter and return `None` for deeper metadata, their stack usage doesn't depend on metadata
* `IoTypeMetadataKind::compact()` replaces names with zero-length names, so it produces valid metadata with the same
  capacity and alignment, previously it didn't for structs with more than 10 named fields and enums with fields. It also
  replaces structs with a single field with the type of the field and structs without fields with `()`, so a newtype has
  the same compact metadata as the type it wraps

Improvements:

* `TrivialType` can now be derived for enums with tuple variants

Fixes:

* `MaybeData` and `VariableElements` with zero-sized types no longer compile, previously `MaybeData` couldn't represent
  absence of data and `VariableElements` panicked
* `VariableElements::copy_from()` no longer reads and writes out of bounds for elements larger than a byte, `count()`
  returns the number of elements instead of the size in bytes, `from_uninit()` sets capacity in bytes instead of the
  number of elements, and `append()` updates the size, so appended elements are no longer lost
* `MaybeData::get_mut_or_init_with()` no longer exposes uninitialized memory if the initialization function returns a
  reference to other data
* Arrays of 4 GiB or larger or with `2^32` or more elements can no longer be used as `TrivialType`, previously their
  size and length were truncated
* `TrivialType` can now be derived for enums with 11 to 16 variants and for deeply nested types, and derived metadata of
  enums whose first variant has no fields while later variants have zero-sized fields is now valid
* `IoTypeMetadataKind::type_details()` now returns the alignment of elements for `VariableElements<T>` with the default
  recommended allocation instead of 1, and `None` instead of overflowing for enums with a variant of `u32::MAX` bytes

# 0.2.0

Breaking changes:

* Migrate from `generic_const_exprs` to `generic_const_args` family of nightly features

# 0.1.0

Initial release
