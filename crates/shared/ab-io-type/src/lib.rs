//! Infrastructure for zero-cost zero-copy serialization/deserialization.
//!
//! <div class="warning">
//! This crate only supports little-endian platforms by design.
//! </div>
//!
//! This crate primarily offers the following:
//! * [`TrivialType`] trait
//! * [`IoType`] trait
//! * metadata describing types implementing the above traits (see [`IoTypeMetadataKind`])
//!
//! [`IoTypeMetadataKind`]: metadata::IoTypeMetadataKind
//!
//! ## `TrivialType`
//! This trait is implemented for a bunch of built-in types and can be derived for custom types that
//! contain them (structs and enums). It represents trivial types, which do not contain
//! uninitialized bytes and are fully represented by their byte representation.
//!
//! What this means is that serialization to bytes can be done by simply casting a pointer to a data
//! structure to an array of bytes. Similarly, deserialization parses bytes, checking that they are
//! a valid value of the type, which for types that accept every bit pattern compiles to a length
//! check, and then casts the pointer. The trait provides a few helper methods for dealing with
//! serialization/deserialization.
//!
//! ## `IoType`
//! This trait is implemented for all types that implement [`TrivialType`] and for a few additional
//! custom types that have special properties and are useful for FFI purposes.
//!
//! `IoType` data structures can contain optional values or lists of values of a dynamic size. They
//! are not as composable as `TrivialType` and are usually used as wrappers of the highest level.
//! This is in contrast to `TrivialType` that is always fixed size and can't have optional data.
//!
//! ## Metadata
//!
//! The data structures implementing [`TrivialType`] and [`IoType`] traits have the `METADATA`
//! associated constant (see [`IoTypeMetadataKind`]). This field contains a compact binary
//! representation of the recursive layout of the type, which can then be decoded by the machine for
//! FFI purposes or converted into human-readable format for presentation to the user in a somewhat
//! readable way.
//!
//! The metadata contains both the memory layout and the names of data structures and fields.
//! Metadata can also be compressed into an equivalent layout without names and with newtypes
//! replaced by the types they wrap to shrink the size of the metadata further when human
//! readability is not necessary. Compressed metadata can also be hashed to get a "fingerprint" of
//! the data structure, which may be used to distinguish a compatible FFI interface from an
//! incompatible one based on the data layout, not just the number of bytes.
//!
//! ## Overall
//!
//! These traits are designed for zero-cost zero-copy serialization/deserialization. Correctly
//! aligned memory (both normal and memory-mapped files with `mmap`) is used in place as
//! ready-to-use data structures after parsing, which for types that accept every bit pattern is a
//! length check that doesn't read the memory.
//!
//! Does not require a standard library (`no_std`) or an allocator.

#![expect(incomplete_features, reason = "generic_const_*")]
#![feature(
    cast_maybe_uninit,
    const_block_items,
    const_cmp,
    const_convert,
    const_index,
    const_result_trait_fn,
    const_split_off_first_last,
    const_trait_impl,
    const_try,
    final_associated_functions,
    generic_const_args,
    generic_const_items,
    macroless_generic_const_args,
    min_generic_const_args,
    ptr_as_uninit
)]
#![no_std]

pub mod fixed_capacity_bytes;
pub mod fixed_capacity_elements;
pub mod fixed_capacity_string;
pub mod maybe_data;
pub mod metadata;
pub mod trivial_type;
pub mod unaligned;
pub mod variable_bytes;
pub mod variable_elements;

use crate::trivial_type::TrivialType;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;

/// The maximum alignment supported by [`IoType`] types (16 bytes, corresponds to alignment of
/// `u128`)
pub const MAX_ALIGNMENT: u8 = 16;

// Only little-endian platforms are supported. On big-endian platforms the byte order differs,
// so `TrivialType` values cannot be transferred simply by sending their raw struct bytes
// between host and guest environments
#[cfg(not(target_endian = "little"))]
compile_error!("Only little-endian platforms are supported");

const {
    assert!(
        size_of::<usize>() >= size_of::<u32>(),
        "At least 32-bit platform required"
    );

    // Max alignment is expected to match that of `u128`
    assert!(
        align_of::<u128>() == MAX_ALIGNMENT as usize,
        "Max alignment mismatch"
    );

    // Only support targets with expected alignment and refuse to compile on other targets
    assert!(align_of::<()>() == 1, "Unsupported alignment of `()`");
    assert!(align_of::<u8>() == 1, "Unsupported alignment of `u8`");
    assert!(align_of::<u16>() == 2, "Unsupported alignment of `u16`");
    assert!(align_of::<u32>() == 4, "Unsupported alignment of `u32`");
    assert!(align_of::<u64>() == 8, "Unsupported alignment of `u64`");
    assert!(align_of::<u128>() == 16, "Unsupported alignment of `u128`");
    assert!(align_of::<i8>() == 1, "Unsupported alignment of `i8`");
    assert!(align_of::<i16>() == 2, "Unsupported alignment of `i16`");
    assert!(align_of::<i32>() == 4, "Unsupported alignment of `i32`");
    assert!(align_of::<i64>() == 8, "Unsupported alignment of `i64`");
    assert!(align_of::<i128>() == 16, "Unsupported alignment of `i128`");
}

struct DerefWrapper<T>(T);

impl<T> Deref for DerefWrapper<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for DerefWrapper<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// TODO: A way to point output types to input types in order to avoid unnecessary memory copy
//  (setting a pointer)
/// Trait that is used for types that are crossing the host/guest boundary in contracts.
///
/// Crucially, it is implemented for any type that implements [`TrivialType`] and for
/// [`VariableBytes`](variable_bytes::VariableBytes).
///
/// # Safety
/// This trait is used for types with memory transmutation capabilities. Memory with untrusted
/// contents is parsed with [`IoType::try_from_ptr()`] and [`IoType::try_from_mut_ptr()`].
/// Serializing and deserializing of types that implement this trait is simply casting of underlying
/// memory. As a result, all the types implementing this trait must not use implicit padding,
/// unions, or anything similar that might make it unsound to access any bits of the type.
/// [`IoType::try_from_ptr()`] and [`IoType::try_from_mut_ptr()`] return `Some` only if
/// [`IoType::from_ptr()`] and [`IoType::from_mut_ptr()`] with the same arguments would be sound.
///
/// Helper functions are provided to make casting to/from bytes a bit safer than it would otherwise,
/// but extra care is still needed.
///
/// **Do not implement this trait explicitly!** Use `#[derive(TrivialType)]` instead, which will
/// ensure safety requirements are upheld, or use `VariableBytes` or other provided wrapper types if
/// more flexibility is needed.
///
/// In case of variable state size is needed, create a wrapper struct around `VariableBytes` and
/// implement traits on it by forwarding everything to the inner implementation, including
/// [`IoType::try_from_ptr()`] and [`IoType::try_from_mut_ptr()`].
pub unsafe trait IoType {
    /// Data structure metadata in binary form, describing shape and types of the contents, see
    /// [`IoTypeMetadataKind`] for encoding details
    ///
    /// [`IoTypeMetadataKind`]: metadata::IoTypeMetadataKind
    const METADATA: &[u8];

    /// Pointer with a trivial type that this `IoType` represents
    type PointerType: TrivialType;

    /// Number of bytes that are currently used to store data
    fn size(&self) -> u32;

    /// Number of bytes are allocated right now
    fn capacity(&self) -> u32;

    /// Set the number of used bytes.
    ///
    /// # Safety
    /// `size` must be valid for the type (see [`Self::try_from_ptr()`]) and not exceed the
    /// capacity, and that many bytes must be initialized. They don't need to be a valid value of
    /// the type, but then the contents must not be accessed through this value until they are
    /// parsed, for example with [`Self::try_from_mut_ptr()`].
    unsafe fn set_size(&mut self, size: u32);

    /// Create a reference to a type, which is represented by provided memory.
    ///
    /// Memory must be correctly aligned and sufficient in size, but padding beyond the size of the
    /// type is allowed. Memory behind a pointer must not be written to in the meantime either.
    ///
    /// Only `size` bytes are guaranteed to be allocated for types that can store a variable amount
    /// of data due to the read-only nature of read-only access here.
    ///
    /// # Safety
    /// The memory holds a valid value of the type with a valid size, for example one that
    /// [`Self::try_from_ptr()`] accepts.
    // `impl Deref` is used to tie lifetime of returned value to inputs but still treat it as a
    // shared reference for most practical purposes. While lifetime here is somewhat superficial due
    // to the `Copy` nature of the value, it must be respected. Size must point to properly
    // initialized memory.
    #[track_caller]
    unsafe fn from_ptr<'a>(
        ptr: &'a NonNull<Self::PointerType>,
        size: &'a u32,
        capacity: u32,
    ) -> impl Deref<Target = Self> + 'a;

    /// Create a mutable reference to a type, which is represented by provided memory.
    ///
    /// Memory must be correctly aligned and sufficient in size, but padding beyond the size of the
    /// type is allowed. Memory behind a pointer must not be read or written to in the meantime
    /// either.
    ///
    /// `size` indicates how many bytes are used within a larger allocation for types that can
    /// store a variable amount of data.
    ///
    /// # Safety
    /// The memory holds a valid value of the type with a valid size, for example one that
    /// [`Self::try_from_mut_ptr()`] accepts.
    // `impl DerefMut` is used to tie lifetime of returned value to inputs, but still treat it as an
    // exclusive reference for most practical purposes. While lifetime here is somewhat superficial
    // due to the `Copy` nature of the value, it must be respected. Size must point to properly
    // initialized and aligned memory for non-[`TrivialType`].
    #[track_caller]
    unsafe fn from_mut_ptr<'a>(
        ptr: &'a mut NonNull<Self::PointerType>,
        size: &'a mut u32,
        capacity: u32,
    ) -> impl DerefMut<Target = Self> + 'a;

    /// Create a reference to a type, which is represented by provided memory, if the memory holds a
    /// valid value.
    ///
    /// In contrast to [`Self::from_ptr()`], the contents are checked, `None` is returned if the
    /// first `size` bytes are not a valid value. Alignment, `size` and `capacity` are trusted like
    /// in [`Self::from_ptr()`] and only checked with debug assertions.
    ///
    /// # Safety
    /// `ptr` is aligned for `Self::PointerType`, `size` doesn't exceed `capacity`, and both are
    /// valid for the type: `size` is the size of a [`TrivialType`], `0` or the size of the data for
    /// [`MaybeData`] and a multiple of the element size for [`VariableElements`], and the capacity
    /// of [`MaybeData`] is `0` or at least the size of the data. `ptr` is valid for reads of `size`
    /// bytes, which are initialized and not written to during the call and while the returned value
    /// exists.
    ///
    /// [`MaybeData`]: maybe_data::MaybeData
    /// [`VariableElements`]: variable_elements::VariableElements
    #[track_caller]
    unsafe fn try_from_ptr<'a>(
        ptr: &'a NonNull<Self::PointerType>,
        size: &'a u32,
        capacity: u32,
    ) -> Option<impl Deref<Target = Self> + 'a>;

    /// Like [`Self::try_from_ptr()`], but returns an exclusive reference.
    ///
    /// # Safety
    /// `ptr`, `size` and `capacity` are valid as required by [`Self::try_from_ptr()`], except that
    /// the capacity of [`MaybeData`] is at least the size of the data, since data can be written
    /// into it even if it is empty. `ptr` is valid for writes of `capacity` bytes and for reads of
    /// the first `size`, which are initialized, while the bytes after them may be uninitialized.
    /// The memory is not accessed otherwise during the call and while the returned value exists.
    ///
    /// [`MaybeData`]: maybe_data::MaybeData
    #[track_caller]
    unsafe fn try_from_mut_ptr<'a>(
        ptr: &'a mut NonNull<Self::PointerType>,
        size: &'a mut u32,
        capacity: u32,
    ) -> Option<impl DerefMut<Target = Self> + 'a>;

    /// Get a raw pointer to the underlying data.
    ///
    /// The pointer can be used for reading [`Self::size()`] bytes as long as the value is not
    /// modified, it must not be used for writing.
    fn as_ptr(&self) -> NonNull<Self::PointerType>;

    /// Get an exclusive raw pointer to the underlying data.
    ///
    /// The pointer can be used for reading [`Self::size()`] bytes and writing up to
    /// [`Self::capacity()`] bytes as long as the value is not accessed otherwise in the meantime.
    /// [`Self::set_size()`] needs to be called afterward if the number of initialized bytes
    /// changes.
    fn as_mut_ptr(&mut self) -> NonNull<Self::PointerType>;
}

/// Marker trait, companion to [`IoType`] that indicates the ability to store optional contents.
///
/// This means that zero bytes size is a valid invariant. This trait is never implemented for types
/// implementing [`TrivialType`] because they always have fixed size, and it is not zero.
///
/// # Safety
/// Size `0` must be a valid state that doesn't require any initialized memory:
/// [`IoType::from_ptr()`], [`IoType::from_mut_ptr()`], [`IoType::try_from_ptr()`] and
/// [`IoType::try_from_mut_ptr()`] must be sound to call with `size` of `0` and uninitialized memory
/// behind the pointer, and must not expose that memory as initialized.
pub unsafe trait IoTypeOptional: IoType {}

/// Exclusive access to an [`IoType`] value.
///
/// It is implemented for:
/// * every [`IoType`]
/// * views that constructors of containers return over exclusive memory, like
///   [`MaybeData::from_mut()`] and [`VariableElements::from_uninit()`]
///
/// This allows generic code to accept both.
///
/// # Safety
/// Generic code relies on [`Self::io_type_mut()`] giving access to a single value owned by `Self`:
/// every call returns a reference to the same value without changing it or anything else, and the
/// value is not accessible other than through `Self`. Generic code may also move the value out of
/// `Self` through that reference and forget `Self` right after, so the value must stay valid
/// without `Self`.
///
/// [`MaybeData::from_mut()`]: maybe_data::MaybeData::from_mut
/// [`VariableElements::from_uninit()`]: variable_elements::VariableElements::from_uninit
#[diagnostic::on_unimplemented(
    message = "`{Self}` doesn't give exclusive access to an `IoType` value"
)]
pub unsafe trait IoTypeMut {
    /// The type of the value
    type Target: IoType;

    /// Exclusive access to the value
    fn io_type_mut(&mut self) -> &mut Self::Target;
}

// SAFETY: The value is `self` itself
unsafe impl<T> IoTypeMut for T
where
    T: IoType,
{
    type Target = T;

    #[inline(always)]
    fn io_type_mut(&mut self) -> &mut Self::Target {
        self
    }
}

// SAFETY: The value is the only field of the wrapper, which doesn't implement `Drop`
unsafe impl<T> IoTypeMut for DerefWrapper<T>
where
    T: IoType,
{
    type Target = T;

    #[inline(always)]
    fn io_type_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
