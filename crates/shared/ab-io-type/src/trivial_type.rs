use crate::metadata::{IoTypeMetadataKind, MAX_METADATA_CAPACITY, concat_metadata_sources};
use crate::unaligned::Unaligned;
use crate::{DerefWrapper, IoType};
pub use ab_trivial_type_derive::TrivialType;
use core::ops::{Deref, DerefMut};
use core::ptr;
use core::ptr::NonNull;

/// Simple wrapper data type that is designed in such a way that its serialization/deserialization
/// is the same as the type itself.
///
/// # Safety
/// This trait is used for types with memory layout that can be treated as bytes. It must not be
/// relied on with untrusted data (it can be constructed from bytes, and internal invariants might
/// not be invalid). Serializing and deserializing of types that implement this trait is simply
/// casting of underlying memory. As a result, all the types implementing this trait must not use
/// implicit padding, unions or anything similar that might make it unsound to access any bits of
/// the type.
///
/// Helper functions are provided to make casting to/from bytes a bit safer than it would otherwise,
/// but extra care is still needed.
///
/// **Do not implement this trait explicitly!** Use `#[derive(TrivialType)]` instead, which will
/// ensure safety requirements are upheld.
pub unsafe trait TrivialType
where
    Self: Copy + 'static,
{
    const SIZE: u32 = size_of::<Self>() as u32;
    /// Data structure metadata in binary form, describing shape and types of the contents, see
    /// [`IoTypeMetadataKind`] for encoding details.
    const METADATA: &[u8];

    /// Read unaligned value from memory.
    ///
    /// Returns `None` if the number of input bytes is not sufficient to represent the type.
    ///
    /// # Safety
    /// Input bytes must be previously produced by taking underlying bytes of the same type, or else
    /// the data structure might have unexpected contents. While technically not unsafe, this API
    /// should be used very carefully and data structure invariants need to be checked manually.
    #[inline(always)]
    unsafe fn read_unaligned(bytes: &[u8]) -> Option<Self> {
        // SAFETY: Guaranteed by function contract
        Some(unsafe { Unaligned::<Self>::from_bytes(bytes)?.as_inner() })
    }

    /// Similar to [`Self::read_unaligned()`], but doesn't do any checks at all.
    ///
    /// # Safety
    /// The number of bytes must be at least as big as the type itself.
    #[inline(always)]
    unsafe fn read_unaligned_unchecked(bytes: &[u8]) -> Self {
        // SAFETY: Guaranteed by function contract
        unsafe { Unaligned::<Self>::from_bytes_unchecked(bytes).as_inner() }
    }

    /// Create a reference to a type, which is represented by provided memory.
    ///
    /// Memory must be correctly aligned, or else `None` will be returned. Size must be sufficient,
    /// padding beyond the size of the type is allowed.
    ///
    /// # Safety
    /// Input bytes must be previously produced by taking underlying bytes of the same type, or else
    /// the data structure might have unexpected contents. While technically not unsafe, this API
    /// should be used very carefully and data structure invariants need to be checked manually.
    #[inline(always)]
    unsafe fn from_bytes(bytes: &[u8]) -> Option<&Self> {
        // SAFETY: For trivial types all bit patterns are valid
        let (before, slice, _) = unsafe { bytes.align_to::<Self>() };

        before.is_empty().then(|| slice.first()).flatten()
    }

    /// Similar to [`Self::from_bytes()`], but doesn't do any checks at all.
    ///
    /// # Safety
    /// Size is at least as big as the type itself, and alignment is correct.
    #[inline(always)]
    unsafe fn from_bytes_unchecked(bytes: &[u8]) -> &Self {
        // SAFETY: Guaranteed by function contract
        unsafe { bytes.as_ptr().cast::<Self>().as_ref_unchecked() }
    }

    /// Create a mutable reference to a type, which is represented by provided memory.
    ///
    /// Memory must be correctly aligned, or else `None` will be returned. Size must be sufficient,
    /// padding beyond the size of the type is allowed.
    ///
    /// # Safety
    /// Input bytes must be previously produced by taking underlying bytes of the same type, or else
    /// the data structure might have unexpected contents. While technically not unsafe, this API
    /// should be used very carefully and data structure invariants need to be checked manually.
    #[inline(always)]
    unsafe fn from_bytes_mut(bytes: &mut [u8]) -> Option<&mut Self> {
        // SAFETY: For trivial types all bit patterns are valid
        let (before, slice, _) = unsafe { bytes.align_to_mut::<Self>() };

        before.is_empty().then(|| slice.first_mut()).flatten()
    }

    /// Similar to [`Self::from_bytes_mut()`], but doesn't do any checks at all.
    ///
    /// # Safety
    /// Size is at least as big as the type itself, and alignment is correct.
    #[inline(always)]
    unsafe fn from_bytes_mut_unchecked(bytes: &mut [u8]) -> &mut Self {
        // SAFETY: Guaranteed by function contract
        unsafe { bytes.as_mut_ptr().cast::<Self>().as_mut_unchecked() }
    }

    /// Access the underlying byte representation of a data structure
    #[inline(always)]
    fn as_bytes(&self) -> &[u8; size_of::<Self>()] {
        // SAFETY: All bits are valid for reading as bytes, see `TrivialType` description
        unsafe { ptr::from_ref(self).cast::<[u8; _]>().as_ref_unchecked() }
    }

    /// Access the underlying mutable byte representation of a data structure.
    ///
    /// # Safety
    /// While calling this function is technically safe, modifying returned memory buffer may result
    /// in broken invariants of underlying data structure and should be done with extra care.
    #[inline(always)]
    unsafe fn as_bytes_mut(&mut self) -> &mut [u8; size_of::<Self>()] {
        // SAFETY: All bits are valid for reading as bytes, see `TrivialType` description
        unsafe { ptr::from_mut(self).cast::<[u8; _]>().as_mut_unchecked() }
    }
}

// SAFETY: Any bit pattern is valid, so it is safe to implement `TrivialType` for this type
unsafe impl TrivialType for () {
    const METADATA: &[u8] = &[IoTypeMetadataKind::Unit as u8];
}
// SAFETY: Any bit pattern is valid, so it is safe to implement `TrivialType` for this type
unsafe impl TrivialType for u8 {
    const METADATA: &[u8] = &[IoTypeMetadataKind::U8 as u8];
}
// SAFETY: Any bit pattern is valid, so it is safe to implement `TrivialType` for this type
unsafe impl TrivialType for u16 {
    const METADATA: &[u8] = &[IoTypeMetadataKind::U16 as u8];
}
// SAFETY: Any bit pattern is valid, so it is safe to implement `TrivialType` for this type
unsafe impl TrivialType for u32 {
    const METADATA: &[u8] = &[IoTypeMetadataKind::U32 as u8];
}
// SAFETY: Any bit pattern is valid, so it is safe to implement `TrivialType` for this type
unsafe impl TrivialType for u64 {
    const METADATA: &[u8] = &[IoTypeMetadataKind::U64 as u8];
}
// SAFETY: Any bit pattern is valid, so it is safe to implement `TrivialType` for this type
unsafe impl TrivialType for u128 {
    const METADATA: &[u8] = &[IoTypeMetadataKind::U128 as u8];
}
// SAFETY: Any bit pattern is valid, so it is safe to implement `TrivialType` for this type
unsafe impl TrivialType for i8 {
    const METADATA: &[u8] = &[IoTypeMetadataKind::I8 as u8];
}
// SAFETY: Any bit pattern is valid, so it is safe to implement `TrivialType` for this type
unsafe impl TrivialType for i16 {
    const METADATA: &[u8] = &[IoTypeMetadataKind::I16 as u8];
}
// SAFETY: Any bit pattern is valid, so it is safe to implement `TrivialType` for this type
unsafe impl TrivialType for i32 {
    const METADATA: &[u8] = &[IoTypeMetadataKind::I32 as u8];
}
// SAFETY: Any bit pattern is valid, so it is safe to implement `TrivialType` for this type
unsafe impl TrivialType for i64 {
    const METADATA: &[u8] = &[IoTypeMetadataKind::I64 as u8];
}
// SAFETY: Any bit pattern is valid, so it is safe to implement `TrivialType` for this type
unsafe impl TrivialType for i128 {
    const METADATA: &[u8] = &[IoTypeMetadataKind::I128 as u8];
}

const fn array_metadata(size: u32, inner_metadata: &[u8]) -> ([u8; MAX_METADATA_CAPACITY], usize) {
    if inner_metadata.len() == 1 && inner_metadata[0] == IoTypeMetadataKind::U8 as u8 {
        if size == 8 {
            return concat_metadata_sources(&[&[IoTypeMetadataKind::ArrayU8x8 as u8]]);
        } else if size == 16 {
            return concat_metadata_sources(&[&[IoTypeMetadataKind::ArrayU8x16 as u8]]);
        } else if size == 32 {
            return concat_metadata_sources(&[&[IoTypeMetadataKind::ArrayU8x32 as u8]]);
        } else if size == 64 {
            return concat_metadata_sources(&[&[IoTypeMetadataKind::ArrayU8x64 as u8]]);
        } else if size == 128 {
            return concat_metadata_sources(&[&[IoTypeMetadataKind::ArrayU8x128 as u8]]);
        } else if size == 256 {
            return concat_metadata_sources(&[&[IoTypeMetadataKind::ArrayU8x256 as u8]]);
        } else if size == 512 {
            return concat_metadata_sources(&[&[IoTypeMetadataKind::ArrayU8x512 as u8]]);
        } else if size == 1024 {
            return concat_metadata_sources(&[&[IoTypeMetadataKind::ArrayU8x1024 as u8]]);
        } else if size == 2028 {
            return concat_metadata_sources(&[&[IoTypeMetadataKind::ArrayU8x2028 as u8]]);
        } else if size == 4096 {
            return concat_metadata_sources(&[&[IoTypeMetadataKind::ArrayU8x4096 as u8]]);
        }
    }

    let (io_type, size_bytes) = if size < 2u32.pow(8) {
        (IoTypeMetadataKind::Array8b, 1)
    } else if size < 2u32.pow(16) {
        (IoTypeMetadataKind::Array16b, 2)
    } else {
        (IoTypeMetadataKind::Array32b, 4)
    };

    concat_metadata_sources(&[
        &[io_type as u8],
        size.to_le_bytes().split_at(size_bytes).0,
        inner_metadata,
    ])
}

// TODO: Change `usize` to `u32` once stabilized `generic_const_exprs` feature allows us to do
//  `SIZE as usize`
// SAFETY: Any bit pattern is valid, so it is safe to implement `TrivialType` for this type
unsafe impl<const SIZE: usize, T> TrivialType for [T; SIZE]
where
    T: TrivialType,
{
    const METADATA: &[u8] = {
        // Strange syntax to allow Rust to extend the lifetime of metadata scratch automatically
        array_metadata(SIZE as u32, T::METADATA)
            .0
            .split_at(array_metadata(SIZE as u32, T::METADATA).1)
            .0
    };
}

// SAFETY: All `TrivialType` instances are valid for `IoType`
unsafe impl<T> IoType for T
where
    T: TrivialType,
{
    const METADATA: &[u8] = T::METADATA;

    type PointerType = T;

    #[inline(always)]
    fn size(&self) -> u32 {
        size_of::<T>() as u32
    }

    #[inline(always)]
    unsafe fn size_ptr(&self) -> impl Deref<Target = NonNull<u32>> {
        DerefWrapper(NonNull::from_ref(&T::SIZE))
    }

    #[inline(always)]
    unsafe fn size_mut_ptr(&mut self) -> impl DerefMut<Target = *mut u32> {
        DerefWrapper(ptr::null_mut())
    }

    #[inline(always)]
    fn capacity(&self) -> u32 {
        self.size()
    }

    #[inline(always)]
    unsafe fn capacity_ptr(&self) -> impl Deref<Target = NonNull<u32>> {
        DerefWrapper(NonNull::from_ref(&Self::SIZE))
    }

    #[inline(always)]
    #[track_caller]
    unsafe fn set_size(&mut self, size: u32) {
        debug_assert_eq!(size, T::SIZE, "`set_size` called with invalid input");
    }

    #[inline(always)]
    #[track_caller]
    unsafe fn from_ptr<'a>(
        ptr: &'a NonNull<Self::PointerType>,
        size: &'a u32,
        capacity: u32,
    ) -> impl Deref<Target = Self> + 'a {
        debug_assert!(ptr.is_aligned(), "Misaligned pointer");
        debug_assert_eq!(*size, T::SIZE, "Invalid size");
        debug_assert!(
            *size <= capacity,
            "Size {size} must not exceed capacity {capacity}"
        );

        // SAFETY: guaranteed by this function signature
        unsafe { ptr.as_ref() }
    }

    #[inline(always)]
    #[track_caller]
    unsafe fn from_mut_ptr<'a>(
        ptr: &'a mut NonNull<Self::PointerType>,
        _size: &'a mut *mut u32,
        capacity: u32,
    ) -> impl DerefMut<Target = Self> + 'a {
        debug_assert!(ptr.is_aligned(), "Misaligned pointer");
        debug_assert!(
            Self::SIZE <= capacity,
            "Size {} must not exceed capacity {capacity}",
            Self::SIZE
        );

        // SAFETY: guaranteed by this function signature
        unsafe { ptr.as_mut() }
    }

    #[inline(always)]
    unsafe fn as_ptr(&self) -> impl Deref<Target = NonNull<Self::PointerType>> {
        DerefWrapper(NonNull::from_ref(self))
    }

    #[inline(always)]
    unsafe fn as_mut_ptr(&mut self) -> impl DerefMut<Target = NonNull<Self::PointerType>> {
        DerefWrapper(NonNull::from_mut(self))
    }
}
