# 0.3.0

Breaking changes:

* Metadata kinds for specific numbers of fields, variants and elements (like `Struct3` and `ArrayU8x32`) are removed,
  array lengths, recommended allocations and capacities always take 4 bytes, `TupleStruct` and `EnumNoFields` are merged
  into `Struct` and `Enum`, `MaybeData` has its own kind instead of the metadata of its data, and the remaining kinds
  are renumbered, so `METADATA` of most types changes
* `VariableBytes` is now an alias of `VariableElements<u8>`, and `FixedCapacityBytesU8/U16` and
  `FixedCapacityStringU8/U16` are replaced with `FixedCapacityElements<Element, CAPACITY>` (with the
  `FixedCapacityBytes<CAPACITY>` alias) and `FixedCapacityString<CAPACITY>`, which store the length as `u32`
* `IoTypeMetadataKind::type_details()` rejects enums without variants, `MaybeData` and `VariableElements` inside other
  types and layouts with padding
* `IoTypeMetadataKind::type_details()` and `IoTypeMetadataKind::compact()` take the max nesting depth as a const generic
  parameter and return `None` for deeper metadata, their stack usage doesn't depend on metadata
* `IoTypeDetails::alignment` is `Alignment` instead of `NonZeroU8`. `Alignment` is a new type that holds the alignment
  of a type or value as a power of two up to `Alignment::MAX`, so decoded alignments don't need to be checked.
  `MAX_ALIGNMENT` is removed, `Alignment::MAX` replaces it
* `IoTypeMetadataKind::compact()` replaces names with zero-length names, so it produces valid metadata with the same
  capacity and alignment, previously it didn't for structs with more than 10 named fields and enums with fields. It also
  replaces structs with a single field with the type of the field and structs without fields with `()`, so a newtype has
  the same compact metadata as the type it wraps
* `IoType::as_ptr()` and `IoType::as_mut_ptr()` are safe and return `NonNull` by value, the inherent
  `VariableElements::as_mut_ptr()` is removed, constructors of `VariableElements` and `MaybeData` that take a size
  return `None` for an invalid size instead of checking it only in debug builds, if at all, and `IoTypeOptional` is an
  `unsafe` trait, since it must only be implemented for types with a valid empty state
* `TrivialType` derive rejects enums with explicit discriminants that differ from variant indices, which metadata
  doesn't record. It also checks all `#[repr(..)]` attributes instead of only the first one and rejects enums with
  `#[repr(C, u8)]`
* `TrivialType` requires `'static` and the hidden `unsafe` parsing step `parse_step::<ALIGNED>()`, which the derive
  implements. It has an aligned flavour, which may rely on the bytes being aligned for the type, and an unaligned one,
  manual implementations must uphold the safety contract for both
* `TrivialType` requires an alignment of at most `Alignment::MAX` (16 bytes), which derived implementations always have
* `TrivialType` requires the constant `ALIGNMENT`, the alignment of the type in metadata, which the derive implements.
  It is 16 bytes (`Alignment::MAX`) for `u128` and `i128`. `Alignment::of()` and the length prefix of
  `FixedCapacityElements` use it
* Provided methods of `TrivialType` can't be overridden
* `IoType` requires `try_from_ptr()` and `try_from_mut_ptr()`
* Methods of `MaybeData` and `VariableElements`, except for `size()`, `capacity()`, `cast_ref()` and `cast_mut()` of
  `VariableElements`, require `MaybeData<'_, Data>: IoType` and
  `VariableElements<'_, Element, RECOMMENDED_ALLOCATION>: IoType`, which don't hold for zero-sized types, generic code
  calling them needs these bounds, like `for<'a> MaybeData<'a, Data>: IoType`
* `VariableBytes::read_trivial_type()`, which created invalid enum values from bytes in safe code, is removed,
  `Unaligned::<T>::try_from_bytes()` replaces it
* `Bool` is removed, `bool` implements `TrivialType` and parsing accepts only `0` and `1` for it
* `VariableElements`, `VariableBytes` and `MaybeData` have a lifetime parameter for the memory they view, previously two
  of them could be swapped in safe code and one of them used after the memory of the other was freed. Constructors
  over exclusive memory return `Self`, constructors over shared memory still return `impl Deref`
* `IoType` requires the associated type `WithLifetime<'b>`, the same type viewing memory borrowed for `'b`, and
  `reborrow_mut()`, which returns the associated type `Reborrowed<'b>`: `&'b mut Self` for types that don't view memory
  and a new view of the same memory for containers. Methods of `IoType` that create values from pointers return
  `WithLifetime<'a>`, and the memory must stay valid until `'a` ends, since a container can be moved out of what they
  return
* `TrivialType::read_unaligned()` and `read_unaligned_unchecked()` are removed, values are read from unaligned memory
  with the `TrivialType` methods of `Unaligned<T>` instead, like `Unaligned::<T>::try_from_bytes()`, and copied out
  with `Unaligned::as_inner()`
* `TrivialType::from_bytes()` and `from_bytes_mut()` are safe, require `AnyBitPattern` and return the remaining bytes
  like the `try_` variants, which replace them for other types. References to zero-sized types are now created from any
  aligned bytes, previously these methods returned `None` for them
* `TrivialType::as_bytes_mut()` is safe and requires `AnyBitPattern`, the unsafe `as_bytes_mut_unchecked()` replaces it
  for other types

Improvements:

* `TrivialType` can now be derived for enums with tuple variants
* 32-bit RISC-V and Arm targets are supported
* `Alignment::is_slice_aligned()` checks where a slice starts
* `AnyBitPattern` is implemented for `TrivialType`s whose every bit pattern is a valid value, which implement the
  unsafe `AnyBitPatternMarker`, the derive implements it for structs whose fields all implement it
* `TrivialType::try_from_bytes()`, `try_from_bytes_exact()`, `try_from_bytes_mut()` and `try_from_bytes_mut_exact()`
  parse bytes safely, checking enum discriminants and the length of `FixedCapacityElements`
* `TrivialType::from_bytes_exact()` and `from_bytes_mut_exact()` return `None` if there are more bytes than the value
* `IoType::try_from_ptr()` and `IoType::try_from_mut_ptr()` parse memory, checking its contents, while size, capacity
  and alignment must be valid for the type, like for `from_ptr()` and `from_mut_ptr()`
* `#[trivial_type(validate = path)]` adds a check to derived parsing, `path(&value)` returns whether the value is valid,
  types that use it don't implement `AnyBitPattern`
* `IoType::set_size()` requires a size that is valid for the type, while the contents don't need to be a valid value
  when it is called, as long as they are parsed before they are accessed

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
* Code generated by `TrivialType` derive refers to items by absolute paths, previously macros and trait methods in scope
  could disable its assertions or replace metadata, and it didn't compile with `#[no_implicit_prelude]`
* `TrivialType` can now be derived for enums with 11 to 16 variants and for deeply nested types, and derived metadata of
  enums whose first variant has no fields while later variants have zero-sized fields is now valid
* `IoTypeMetadataKind::type_details()` now returns the alignment of elements for `VariableElements<T>` with the default
  recommended allocation instead of 1, and `None` instead of overflowing for enums with a variant of `u32::MAX` bytes

# 0.2.0

Breaking changes:

* Migrate from `generic_const_exprs` to `generic_const_args` family of nightly features

# 0.1.0

Initial release
