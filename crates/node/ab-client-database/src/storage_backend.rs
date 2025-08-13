use futures::channel::oneshot;
use std::mem::MaybeUninit;
use std::{fmt, io, mem};

/// A wrapper data structure with 4096 bytes alignment, which is the most common alignment for
/// direct I/O operations.
#[derive(Debug, Copy, Clone)]
#[repr(C, align(4096))]
pub struct AlignedPage([u8; AlignedPage::SIZE]);

const _: () = {
    assert!(align_of::<AlignedPage>() == AlignedPage::SIZE);
};

impl Default for AlignedPage {
    #[inline(always)]
    fn default() -> Self {
        Self([0; AlignedPage::SIZE])
    }
}

impl AlignedPage {
    /// 4096 is as a relatively safe size due to sector size on SSDs commonly being 512 or 4096
    /// bytes
    pub const SIZE: usize = 4096;

    /// Convert an exclusive slice to an uninitialized version
    pub fn as_uninit_slice_mut(value: &mut [Self]) -> &mut [MaybeUninit<Self>] {
        // SAFETY: Same layout
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion from slice to underlying representation for efficiency purposes
    #[inline(always)]
    pub fn slice_to_repr(value: &[Self]) -> &[[u8; AlignedPage::SIZE]] {
        // SAFETY: `AlignedPage` is `#[repr(C)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion from slice to underlying representation for efficiency purposes
    #[inline(always)]
    pub fn uninit_slice_to_repr(
        value: &[MaybeUninit<Self>],
    ) -> &[MaybeUninit<[u8; AlignedPage::SIZE]>] {
        // SAFETY: `AlignedPage` is `#[repr(C)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion from a slice of underlying representation for efficiency purposes.
    ///
    /// Returns `None` if not correctly aligned.
    #[inline]
    pub fn try_slice_from_repr(value: &[[u8; AlignedPage::SIZE]]) -> Option<&[Self]> {
        // SAFETY: All bit patterns are valid
        let (before, slice, after) = unsafe { value.align_to::<Self>() };

        if before.is_empty() && after.is_empty() {
            Some(slice)
        } else {
            None
        }
    }

    /// Convenient conversion from a slice of underlying representation for efficiency purposes.
    ///
    /// Returns `None` if not correctly aligned.
    #[inline]
    pub fn try_uninit_slice_from_repr(
        value: &[MaybeUninit<[u8; AlignedPage::SIZE]>],
    ) -> Option<&[MaybeUninit<Self>]> {
        // SAFETY: All bit patterns are valid
        let (before, slice, after) = unsafe { value.align_to::<MaybeUninit<Self>>() };

        if before.is_empty() && after.is_empty() {
            Some(slice)
        } else {
            None
        }
    }

    /// Convenient conversion from mutable slice to underlying representation for efficiency
    /// purposes
    #[inline(always)]
    pub fn slice_mut_to_repr(slice: &mut [Self]) -> &mut [[u8; AlignedPage::SIZE]] {
        // SAFETY: `AlignedSectorSize` is `#[repr(C)]` and its alignment is larger than inner value
        unsafe { mem::transmute(slice) }
    }

    /// Convenient conversion from mutable slice to underlying representation for efficiency
    /// purposes
    #[inline(always)]
    pub fn uninit_slice_mut_to_repr(
        slice: &mut [MaybeUninit<Self>],
    ) -> &mut [MaybeUninit<[u8; AlignedPage::SIZE]>] {
        // SAFETY: `AlignedSectorSize` is `#[repr(C)]` and its alignment is larger than inner value
        unsafe { mem::transmute(slice) }
    }

    /// Convenient conversion from a slice of underlying representation for efficiency purposes.
    ///
    /// Returns `None` if not correctly aligned.
    #[inline]
    pub fn try_slice_mut_from_repr(value: &mut [[u8; AlignedPage::SIZE]]) -> Option<&mut [Self]> {
        // SAFETY: All bit patterns are valid
        let (before, slice, after) = unsafe { value.align_to_mut::<Self>() };

        if before.is_empty() && after.is_empty() {
            Some(slice)
        } else {
            None
        }
    }

    /// Convenient conversion from a slice of underlying representation for efficiency purposes.
    ///
    /// Returns `None` if not correctly aligned.
    #[inline]
    pub fn try_uninit_slice_mut_from_repr(
        value: &mut [MaybeUninit<[u8; AlignedPage::SIZE]>],
    ) -> Option<&mut [MaybeUninit<Self>]> {
        // SAFETY: All bit patterns are valid
        let (before, slice, after) = unsafe { value.align_to_mut::<MaybeUninit<Self>>() };

        if before.is_empty() && after.is_empty() {
            Some(slice)
        } else {
            None
        }
    }
}

/// Storage backend to be used by [`ClientDatabase`]
///
/// [`ClientDatabase`]: crate::ClientDatabase
pub trait ClientDatabaseStorageBackend: fmt::Debug + Send + Sync + 'static {
    /// Total number of pages available for reads/writes
    fn num_pages(&self) -> u32;

    // TODO: Think whether `Vec` is the right wrapper here to avoid reallocations
    /// Reading into aligned memory.
    ///
    /// `length` is the number of [`AlignedPage`] units (pages) to read into (append to)
    /// `buffer`. `offset` is in pages too.
    fn read(
        &self,
        buffer: Vec<AlignedPage>,
        length: u32,
        offset: u32,
    ) -> oneshot::Receiver<io::Result<Vec<AlignedPage>>>;

    // TODO: Think whether `Vec` is the right wrapper here to avoid reallocations
    /// Writing from aligned memory.
    ///
    /// `offset` is in [`AlignedPage`] units (pages). After successful writing returns allocated
    /// pages back to the caller.
    fn write(
        &self,
        buffer: Vec<AlignedPage>,
        offset: u32,
    ) -> oneshot::Receiver<io::Result<Vec<AlignedPage>>>;
}
