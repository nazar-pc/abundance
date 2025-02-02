use crate::metadata::{IoTypeMetadataKind, MAX_METADATA_CAPACITY, concat_metadata_sources};
use crate::{DerefWrapper, IoType};
pub use ab_contracts_trivial_type_derive::TrivialType;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;
use core::{mem, ptr, slice};

/// Simple wrapper data type that is designed in such a way that its serialization/deserialization
/// is the same as the type itself.
///
/// # Safety
/// This trait is used for types with memory transmutation capabilities, it must not be relied on
/// with untrusted data. Serializing and deserializing of types that implement this trait is simply
/// casting of underlying memory, as the result all the types implementing this trait must not use
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
    // TODO: Compact metadata without field and struct names
    /// Data structure metadata in binary form, describing shape and types of the contents, see
    /// [`IoTypeMetadataKind`] for encoding details.
    const METADATA: &[u8];

    /// Create a reference to a type, which is represented by provided memory.
    ///
    /// Memory must be correctly aligned or else `None` will be returned, but padding beyond the
    /// size of the type is allowed.
    ///
    /// # Safety
    /// Input bytes must be previously produced by taking underlying bytes of the same type.
    #[inline]
    unsafe fn from_bytes(bytes: &[u8]) -> Option<&Self> {
        let (before, slice, _) = unsafe { bytes.align_to::<Self>() };

        before.is_empty().then(|| slice.first()).flatten()
    }

    /// Create a mutable reference to a type, which is represented by provided memory.
    ///
    /// Memory must be correctly aligned or else `None` will be returned, but padding beyond the
    /// size of the type is allowed.
    ///
    /// # Safety
    /// Input bytes must be previously produced by taking underlying bytes of the same type.
    #[inline]
    unsafe fn from_bytes_mut(bytes: &mut [u8]) -> Option<&mut Self> {
        let (before, slice, _) = unsafe { bytes.align_to_mut::<Self>() };

        before.is_empty().then(|| slice.first_mut()).flatten()
    }

    /// Access underlying byte representation of a data structure
    #[inline]
    fn as_bytes(&self) -> &[u8] {
        let self_ptr = unsafe { mem::transmute::<*const Self, *const u8>(self) };
        unsafe { slice::from_raw_parts(self_ptr, size_of::<Self>()) }
    }

    /// Access underlying mutable byte representation of a data structure.
    ///
    /// # Safety
    /// While calling this function is technically safe, modifying returned memory buffer may result
    /// in broken invariants of underlying data structure and should be done with extra care.
    #[inline]
    unsafe fn as_bytes_mut(&mut self) -> &mut [u8] {
        let self_ptr = unsafe { mem::transmute::<*mut Self, *mut u8>(self) };
        unsafe { slice::from_raw_parts_mut(self_ptr, size_of::<Self>()) }
    }
}

unsafe impl TrivialType for () {
    const METADATA: &[u8] = &[IoTypeMetadataKind::Unit as u8];
}
unsafe impl TrivialType for bool {
    const METADATA: &[u8] = &[IoTypeMetadataKind::Bool as u8];
}
unsafe impl TrivialType for u8 {
    const METADATA: &[u8] = &[IoTypeMetadataKind::U8 as u8];
}
unsafe impl TrivialType for u16 {
    const METADATA: &[u8] = &[IoTypeMetadataKind::U16 as u8];
}
unsafe impl TrivialType for u32 {
    const METADATA: &[u8] = &[IoTypeMetadataKind::U32 as u8];
}
unsafe impl TrivialType for u64 {
    const METADATA: &[u8] = &[IoTypeMetadataKind::U64 as u8];
}
unsafe impl TrivialType for u128 {
    const METADATA: &[u8] = &[IoTypeMetadataKind::U128 as u8];
}
unsafe impl TrivialType for i8 {
    const METADATA: &[u8] = &[IoTypeMetadataKind::I8 as u8];
}
unsafe impl TrivialType for i16 {
    const METADATA: &[u8] = &[IoTypeMetadataKind::I16 as u8];
}
unsafe impl TrivialType for i32 {
    const METADATA: &[u8] = &[IoTypeMetadataKind::I32 as u8];
}
unsafe impl TrivialType for i64 {
    const METADATA: &[u8] = &[IoTypeMetadataKind::I64 as u8];
}
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

unsafe impl<T> IoType for T
where
    T: TrivialType,
{
    const METADATA: &[u8] = T::METADATA;

    type PointerType = T;

    #[inline]
    fn size(&self) -> u32 {
        size_of::<T>() as u32
    }

    #[inline]
    unsafe fn size_ptr(&self) -> impl Deref<Target = NonNull<u32>> {
        DerefWrapper(NonNull::from_ref(&T::SIZE))
    }

    #[inline]
    unsafe fn size_mut_ptr(&mut self) -> impl DerefMut<Target = *mut u32> {
        DerefWrapper(ptr::null_mut())
    }

    #[inline]
    fn capacity(&self) -> u32 {
        self.size()
    }

    #[inline]
    unsafe fn capacity_ptr(&self) -> impl Deref<Target = NonNull<u32>> {
        DerefWrapper(NonNull::from_ref(&(size_of::<T>() as u32)))
    }

    #[inline]
    unsafe fn set_size(&mut self, size: u32) {
        debug_assert!(
            size == size_of::<Self>() as u32,
            "`set_size` called with invalid input"
        );
    }

    #[inline]
    unsafe fn from_ptr<'a>(
        ptr: &'a NonNull<Self::PointerType>,
        size: &'a u32,
        capacity: u32,
    ) -> impl Deref<Target = Self> + 'a {
        debug_assert!(ptr.is_aligned(), "Misaligned pointer");
        debug_assert!(*size as usize == size_of::<Self>(), "Invalid size");
        debug_assert!(*size <= capacity, "Size must not exceed capacity");

        // SAFETY: guaranteed by this function signature
        unsafe { ptr.as_ref() }
    }

    #[inline]
    unsafe fn from_mut_ptr<'a>(
        ptr: &'a mut NonNull<Self::PointerType>,
        _size: &'a mut *mut u32,
        capacity: u32,
    ) -> impl DerefMut<Target = Self> + 'a {
        debug_assert!(ptr.is_aligned(), "Misaligned pointer");
        debug_assert!(Self::SIZE <= capacity, "Size must not exceed capacity");

        // SAFETY: guaranteed by this function signature
        unsafe { ptr.as_mut() }
    }

    #[inline]
    unsafe fn as_ptr(&self) -> impl Deref<Target = NonNull<Self::PointerType>> {
        DerefWrapper(NonNull::from_ref(self))
    }

    #[inline]
    unsafe fn as_mut_ptr(&mut self) -> impl DerefMut<Target = NonNull<Self::PointerType>> {
        DerefWrapper(NonNull::from_mut(self))
    }
}
