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
//! structure to an array of bytes. Similarly, deserialization of correctly aligned memory is simply
//! casting of a pointer back to the data structure. The trait provides a few helper methods for
//! dealing with serialization/deserialization.
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
//! Metadata can also be compressed into an equivalent layout without field names to shrink the size
//! of the metadata further when human readability is not necessary. Compressed metadata can also
//! be hashed to get a "fingerprint" of the data structure, which may be used to distinguish a
//! compatible FFI interface from an incompatible one based on the data layout, not just the number
//! of bytes.
//!
//! ## Overall
//!
//! These traits are designed for zero-cost zero-copy serialization/deserialization. Any correctly
//! aligned memory (both normal and memory-mapped files with `mmap`) can be interpreted as
//! ready-to-use data structures without even reading them first.
//!
//! Does not require a standard library (`no_std`) or an allocator.

#![expect(incomplete_features, reason = "generic_const_*")]
#![feature(
    cast_maybe_uninit,
    const_block_items,
    const_convert,
    const_index,
    const_option_ops,
    const_result_trait_fn,
    const_split_off_first_last,
    const_trait_impl,
    const_try,
    generic_const_args,
    generic_const_items,
    min_generic_const_args,
    ptr_as_uninit
)]
#![no_std]

pub mod bool;
pub mod fixed_capacity_bytes;
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
/// This trait is used for types with memory transmutation capabilities, it must not be relied on
/// with untrusted data. Serializing and deserializing of types that implement this trait is simply
/// casting of underlying memory. As a result, all the types implementing this trait must not use
/// implicit padding, unions, or anything similar that might make it unsound to access any bits of
/// the type.
///
/// Helper functions are provided to make casting to/from bytes a bit safer than it would otherwise,
/// but extra care is still needed.
///
/// **Do not implement this trait explicitly!** Use `#[derive(TrivialType)]` instead, which will
/// ensure safety requirements are upheld, or use `VariableBytes` or other provided wrapper types if
/// more flexibility is needed.
///
/// In case of variable state size is needed, create a wrapper struct around `VariableBytes` and
/// implement traits on it by forwarding everything to the inner implementation.
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

    /// Set the number of used bytes
    ///
    /// # Safety
    /// `size` must be set to number of properly initialized bytes
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
    /// Input bytes must be previously produced by taking underlying bytes of the same type.
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
    /// Memory must be correctly aligned and sufficient in size, or else `None` will be returned,
    /// but padding beyond the size of the type is allowed. Memory behind a pointer must not be
    /// read or written to in the meantime either.
    ///
    /// `size` indicates how many bytes are used within a larger allocation for types that can
    /// store a variable amount of data.
    ///
    /// # Safety
    /// Input bytes must be previously produced by taking underlying bytes of the same type.
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

    /// Get a raw pointer to the underlying data with no checks.
    ///
    /// # Safety
    /// While calling this function is technically safe, it and allows to ignore many of its
    /// invariants, so requires extra care. In particular, no modifications must be done to the
    /// value while this returned pointer might be used and no changes must be done through the
    /// returned pointer. Also, lifetimes are only superficial here and can be easily (and
    /// incorrectly) ignored by using `Copy`.
    unsafe fn as_ptr(&self) -> impl Deref<Target = NonNull<Self::PointerType>>;

    /// Get an exclusive raw pointer to the underlying data with no checks.
    ///
    /// # Safety
    /// While calling this function is technically safe, it and allows to ignore many of its
    /// invariants, so requires extra care. In particular, the value's contents must not be read or
    /// written to while returned point might be used. Also, lifetimes are only superficial here and
    /// can be easily (and incorrectly) ignored by using `Copy`.
    unsafe fn as_mut_ptr(&mut self) -> impl DerefMut<Target = NonNull<Self::PointerType>>;
}

/// Marker trait, companion to [`IoType`] that indicates the ability to store optional contents.
///
/// This means that zero bytes size is a valid invariant. This type is never implemented for types
/// implementing [`TrivialType`] because they always have fixed size, and it is not zero.
pub trait IoTypeOptional: IoType {}
