#![no_std]

pub mod maybe_data;
pub mod metadata;
pub mod trivial_type;
pub mod variable_bytes;

use crate::trivial_type::TrivialType;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;

// Refuse to compile on lower than 32-bit platforms
static_assertions::const_assert!(size_of::<usize>() >= size_of::<u32>());

// Only support little-endian environments, in big-endian byte order will be different, and
// it'll not be possible to simply send bytes of data structures that implement `TrivialType` from host
// to guest environment
static_assertions::const_assert_eq!(u16::from_ne_bytes(1u16.to_le_bytes()), 1u16);

// Only support targets with expected alignment and refuse to compile on other targets
static_assertions::const_assert_eq!(align_of::<()>(), 1);
static_assertions::const_assert_eq!(align_of::<u8>(), 1);
static_assertions::const_assert_eq!(align_of::<u16>(), 2);
static_assertions::const_assert_eq!(align_of::<u32>(), 4);
static_assertions::const_assert_eq!(align_of::<u64>(), 8);
static_assertions::const_assert_eq!(align_of::<u128>(), 16);
static_assertions::const_assert_eq!(align_of::<i8>(), 1);
static_assertions::const_assert_eq!(align_of::<i16>(), 2);
static_assertions::const_assert_eq!(align_of::<i32>(), 4);
static_assertions::const_assert_eq!(align_of::<i64>(), 8);
static_assertions::const_assert_eq!(align_of::<i128>(), 16);

struct DerefWrapper<T>(T);

impl<T> Deref for DerefWrapper<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for DerefWrapper<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// TODO: A way to point output types to input types in order to avoid unnecessary memory copy
//  (setting a pointer)
/// Trait that is used for types that are crossing host/guest boundary in smart contracts.
///
/// Crucially, it is implemented for any type that implements [`TrivialType`] and for
/// [`VariableBytes`](crate::variable_bytes::VariableBytes).
///
/// # Safety
/// This trait is used for types with memory transmutation capabilities, it must not be relied on
/// with untrusted data. Serializing and deserializing of types that implement this trait is simply
/// casting of underlying memory. As a result, all the types implementing this trait must not use
/// implicit padding, unions or anything similar that might make it unsound to access any bits of
/// the type.
///
/// Helper functions are provided to make casting to/from bytes a bit safer than it would otherwise,
/// but extra care is still needed.
///
/// **Do not implement this trait explicitly!** Use `#[derive(TrivialType)]` instead, which will
/// ensure safety requirements are upheld, or use `VariableBytes` if more flexibility is needed.
///
/// In case of variable state size is needed, create a wrapper struct around `VariableBytes` and
/// implement traits on it by forwarding everything to inner implementation.
pub unsafe trait IoType {
    /// Data structure metadata in binary form, describing shape and types of the contents, see
    /// [`IoTypeMetadataKind`] for encoding details
    ///
    /// [`IoTypeMetadataKind`]: crate::metadata::IoTypeMetadataKind
    const METADATA: &[u8];

    /// Pointer with trivial type that this `IoType` represents
    type PointerType: TrivialType;

    /// Number of bytes are currently used to store data
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
    /// Only `size` are guaranteed to be allocated for types that can store variable amount of
    /// data due to read-only nature of read-only access here.
    ///
    /// # Safety
    /// Input bytes must be previously produced by taking underlying bytes of the same type.
    // `impl Deref` is used to tie lifetime of returned value to inputs, but still treat it as a
    // shared reference for most practical purposes. While lifetime here is somewhat superficial due
    // to `Copy` nature of the value, it must be respected.
    unsafe fn from_ptr<'a>(
        ptr: &'a NonNull<Self::PointerType>,
        size: &'a u32,
        capacity: u32,
    ) -> impl Deref<Target = Self> + 'a;

    /// Create a mutable reference to a type, which is represented by provided memory.
    ///
    /// Memory must be correctly aligned and sufficient in size or else `None` will be returned, but
    /// padding beyond the size of the type is allowed. Memory behind pointer must not be read or
    /// written to in the meantime either.
    ///
    /// `size` indicates how many bytes are used within larger allocation for types that can
    /// store variable amount of data.
    ///
    /// # Safety
    /// Input bytes must be previously produced by taking underlying bytes of the same type.
    // `impl DerefMut` is used to tie lifetime of returned value to inputs, but still treat it as an
    // exclusive reference for most practical purposes. While lifetime here is somewhat superficial
    // due to `Copy` nature of the value, it must be respected.
    unsafe fn from_ptr_mut<'a>(
        ptr: &'a mut NonNull<Self::PointerType>,
        size: &'a mut u32,
        capacity: u32,
    ) -> impl DerefMut<Target = Self> + 'a;

    /// Get raw pointer to the underlying data with no checks
    ///
    /// # Safety
    /// While calling this function is technically safe, it and allows to ignore many of its
    /// invariants, so requires extra care. In particular no modifications must be done to the value
    /// while this returned pointer might be used and no changes must be done through returned
    /// pointer. Also, lifetimes are only superficial here and can be easily (and incorrectly)
    /// ignored by using `Copy`.
    unsafe fn as_ptr(&self) -> impl Deref<Target = NonNull<Self::PointerType>>;

    /// Get exclusive raw pointer to the underlying data with no checks
    ///
    /// # Safety
    /// While calling this function is technically safe, it and allows to ignore many of its
    /// invariants, so requires extra care. In particular the value's contents must not be read or
    /// written to while returned point might be used. Also, lifetimes are only superficial here and
    /// can be easily (and incorrectly) ignored by using `Copy`.
    unsafe fn as_mut_ptr(&mut self) -> impl DerefMut<Target = NonNull<Self::PointerType>>;
}

/// Marker trait, companion to [`IoType`] that indicates ability to store optional contents
pub trait IoTypeOptional: IoType {}
