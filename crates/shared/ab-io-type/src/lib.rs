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
    derive_const,
    final_associated_functions,
    generic_const_args,
    generic_const_items,
    macroless_generic_const_args,
    min_generic_const_args,
    mut_restriction,
    ptr_as_uninit
)]
#![no_std]

pub mod alignment;
pub mod fixed_capacity_bytes;
pub mod fixed_capacity_elements;
pub mod fixed_capacity_string;
pub mod maybe_data;
pub mod metadata;
pub mod trivial_type;
pub mod unaligned;
pub mod variable_bytes;
pub mod variable_elements;

use crate::alignment::Alignment;
use crate::trivial_type::TrivialType;
use core::borrow::BorrowMut;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;

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

    // Only support targets with expected alignment and refuse to compile on other targets
    assert!(align_of::<()>() == 1, "Unsupported alignment of `()`");
    assert!(align_of::<u8>() == 1, "Unsupported alignment of `u8`");
    assert!(align_of::<u16>() == 2, "Unsupported alignment of `u16`");
    assert!(align_of::<u32>() == 4, "Unsupported alignment of `u32`");
    assert!(align_of::<u64>() == 8, "Unsupported alignment of `u64`");
    assert!(align_of::<i8>() == 1, "Unsupported alignment of `i8`");
    assert!(align_of::<i16>() == 2, "Unsupported alignment of `i16`");
    assert!(align_of::<i32>() == 4, "Unsupported alignment of `i32`");
    assert!(align_of::<i64>() == 8, "Unsupported alignment of `i64`");
    assert!(
        align_of::<u128>() <= usize::from(Alignment::MAX.get()),
        "Unsupported alignment of `u128`"
    );
    assert!(
        align_of::<i128>() <= usize::from(Alignment::MAX.get()),
        "Unsupported alignment of `i128`"
    );
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
/// [`IoType::WithLifetime`] is the same type as `Self`, except that the memory it views, if any, is
/// borrowed for the lifetime it is given. [`IoType::reborrow_mut()`] returns `self` for types that
/// don't view memory, and a new value that views the same memory for types that do.
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

    /// The same type, viewing memory borrowed for `'b`.
    ///
    /// Containers like [`VariableElements`] view memory they don't own and have a lifetime
    /// parameter for it, which is `'b` here. Types that don't view memory, like those implementing
    /// [`TrivialType`], are `Self`.
    ///
    /// Methods that create values from pointers and [`Self::reborrow_mut()`] return this type with
    /// the lifetime of the borrow. So a container moved out of what they return, for example with
    /// [`mem::swap()`], can't be used after the borrow ends, whatever lifetime `Self` has.
    ///
    /// [`VariableElements`]: variable_elements::VariableElements
    /// [`mem::swap()`]: core::mem::swap
    type WithLifetime<'b>: IoType<PointerType = Self::PointerType>;

    /// Exclusive access to a value for `'b`, which [`Self::reborrow_mut()`] returns.
    ///
    /// This is `&'b mut Self` for types that don't view memory, like those implementing
    /// [`TrivialType`], and [`Self::WithLifetime`] by value for containers. Generic code can get
    /// `&mut Self::WithLifetime<'b>` from both with [`BorrowMut::borrow_mut()`].
    type Reborrowed<'b>: BorrowMut<Self::WithLifetime<'b>>
    where
        Self: 'b;

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
    // `impl Deref` keeps `ptr` and `size` borrowed for `'a`, and otherwise works like a shared
    // reference. The value it points to has the lifetime of that borrow too (see
    // `Self::WithLifetime`). Size must point to properly initialized memory.
    #[track_caller]
    unsafe fn from_ptr<'a>(
        ptr: &'a NonNull<Self::PointerType>,
        size: &'a u32,
        capacity: u32,
    ) -> impl Deref<Target = Self::WithLifetime<'a>> + 'a;

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
    ///
    /// The returned value can be used for `'a`. A container like [`VariableElements`] can also be
    /// moved out of it, for example with [`mem::swap()`], and then used until `'a` ends. So the
    /// memory must stay valid and must not be accessed otherwise until `'a` ends, not only while
    /// the returned value exists.
    ///
    /// [`VariableElements`]: variable_elements::VariableElements
    /// [`mem::swap()`]: core::mem::swap
    // `impl DerefMut` keeps `ptr` and `size` borrowed for `'a`, and otherwise works like an
    // exclusive reference. The value it points to has the lifetime of that borrow too (see
    // `Self::WithLifetime`), so a container moved out of it can't be used after `'a` ends either.
    // Size must point to properly initialized and aligned memory for non-[`TrivialType`].
    #[track_caller]
    unsafe fn from_mut_ptr<'a>(
        ptr: &'a mut NonNull<Self::PointerType>,
        size: &'a mut u32,
        capacity: u32,
    ) -> impl DerefMut<Target = Self::WithLifetime<'a>> + 'a;

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
    ) -> Option<impl Deref<Target = Self::WithLifetime<'a>> + 'a>;

    /// Like [`Self::try_from_ptr()`], but returns an exclusive reference.
    ///
    /// # Safety
    /// `ptr`, `size` and `capacity` are valid as required by [`Self::try_from_ptr()`], except that
    /// the capacity of [`MaybeData`] is at least the size of the data, since data can be written
    /// into it even if it is empty. `ptr` is valid for writes of `capacity` bytes and for reads of
    /// the first `size`, which are initialized, while the bytes after them may be uninitialized.
    /// The memory is not accessed otherwise during the call and while the returned value exists.
    /// Like with [`Self::from_mut_ptr()`], this lasts until `'a` ends, since a container can be
    /// moved out of the returned value.
    ///
    /// [`MaybeData`]: maybe_data::MaybeData
    #[track_caller]
    unsafe fn try_from_mut_ptr<'a>(
        ptr: &'a mut NonNull<Self::PointerType>,
        size: &'a mut u32,
        capacity: u32,
    ) -> Option<impl DerefMut<Target = Self::WithLifetime<'a>> + 'a>;

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

    /// Exclusive access to this value for the duration of the borrow.
    ///
    /// For a container, this is a new view of the same memory with the lifetime of the borrow.
    /// Replacing it doesn't replace the container behind `self`, and it can't be used after the
    /// borrow ends. Types that don't view memory return `self`.
    fn reborrow_mut(&mut self) -> Self::Reborrowed<'_>;
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
