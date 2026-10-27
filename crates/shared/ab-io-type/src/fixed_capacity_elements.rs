#[cfg(test)]
mod tests;

use crate::alignment::Alignment;
use crate::metadata::{IoTypeMetadataKind, MAX_METADATA_CAPACITY, concat_metadata_sources};
use crate::trivial_type::{TrivialType, try_skip_valid};
use crate::unaligned::Unaligned;
use core::fmt;

/// Size of the prefix of [`FixedCapacityElements`] that stores the number of elements.
///
/// The number of elements is stored as `u32`, the prefix is extended to
/// [`TrivialType::ALIGNMENT`] of elements if it is larger, such that elements are aligned without
/// padding before them.
const LEN_SIZE<Element: TrivialType>: usize =
    usize::from(Alignment::of::<Element>().get()).max(size_of::<u32>());
/// Ensures the capacity (encoded in metadata) and the size of [`FixedCapacityElements`] and
/// [`FixedCapacityString`] fit into `u32`
///
/// [`FixedCapacityString`]: crate::fixed_capacity_string::FixedCapacityString
pub(crate) const SUPPORTED_CAPACITY<Element: TrivialType, const CAPACITY: usize>: usize = {
    assert!(
        u32::MAX as usize >= CAPACITY,
        "Capacity must not exceed `u32::MAX`"
    );
    assert!(
        u32::MAX as usize >= size_of::<[Element; CAPACITY]>()
            && u32::MAX as usize - size_of::<[Element; CAPACITY]>() >= LEN_SIZE::<Element>,
        "Type size must be smaller than 2^32"
    );
    0
};

// TODO: Require `AnyBitPattern` for elements, such that parsing doesn't need to check unused
//  elements, which could then be filled with zeroes instead of requiring `Default`
/// Container for storing a number of elements limited by the specified fixed capacity.
///
/// See also [`FixedCapacityBytes`] and [`FixedCapacityString`].
///
/// In contrast to [`VariableElements`], which can store arbitrary number of elements and can
/// change the capacity, this container has fixed predefined capacity and occupies it regardless of
/// how many elements are actually stored inside. This might seem limiting but allows implementing
/// [`TrivialType`] trait, enabling its use for fields in data structures that derive
/// [`TrivialType`] themselves, which isn't the case with [`VariableElements`].
///
/// The length (the number of stored elements) is a little-endian `u32` in the first 4 bytes,
/// followed by zero bytes up to [`TrivialType::ALIGNMENT`] of elements if it is larger than 4
/// bytes, and `CAPACITY` elements after that. As a result, the container has the same alignment as
/// elements and no padding, regardless of the alignment of elements.
///
/// `CAPACITY` must not exceed `u32::MAX`, and the size of the container must be smaller than
/// 2^32.
///
/// Parsing rejects a length above `CAPACITY`, but doesn't check the rest of the length prefix,
/// which is set to zero bytes whenever the length is set. The unsafe methods of [`TrivialType`]
/// that skip parsing can create an instance with a larger length: methods don't panic then, but
/// such an instance has no accessible elements and nothing can be appended to it until a valid
/// length is set with [`Self::truncate()`] or [`Self::copy_from()`].
///
/// Elements beyond the length are not part of the value. [`Default`] and [`Self::try_from_slice()`]
/// fill them with default elements, while [`Self::truncate()`] and [`Self::copy_from()`] leave
/// them as they are, so bytes of instances with the same elements can differ. Parsing checks all
/// `CAPACITY` elements, they are values of `Element` too.
///
/// [`FixedCapacityBytes`]: crate::fixed_capacity_bytes::FixedCapacityBytes
/// [`FixedCapacityString`]: crate::fixed_capacity_string::FixedCapacityString
/// [`VariableElements`]: crate::variable_elements::VariableElements
#[derive(Copy, Clone)]
#[repr(C)]
pub struct FixedCapacityElements<Element, const CAPACITY: usize>
where
    Element: TrivialType,
{
    len: [u8; LEN_SIZE::<Element>],
    elements: [Element; CAPACITY],
    // TODO: Technically redundant with the where-clause, which is not enforced in generic code
    //  due to https://github.com/rust-lang/rust/issues/164019, remove once fixed. Every instance
    //  evaluates the check through this field. It is initialized in `const` blocks, which are
    //  evaluated even if optimizations remove the field.
    _supported: [(); SUPPORTED_CAPACITY::<Element, CAPACITY>],
}

impl<Element, const CAPACITY: usize> fmt::Debug for FixedCapacityElements<Element, CAPACITY>
where
    Element: TrivialType + fmt::Debug,
    [(); SUPPORTED_CAPACITY::<Element, CAPACITY>]:,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FixedCapacityElements")
            .field("len", &self.len())
            .field("elements", &self.get_elements())
            .finish()
    }
}

impl<Element, const CAPACITY: usize> Default for FixedCapacityElements<Element, CAPACITY>
where
    Element: TrivialType + Default,
    [(); SUPPORTED_CAPACITY::<Element, CAPACITY>]:,
{
    #[inline(always)]
    fn default() -> Self {
        Self {
            len: [0; LEN_SIZE::<Element>],
            elements: [Element::default(); CAPACITY],
            _supported: const { [(); SUPPORTED_CAPACITY::<Element, CAPACITY>] },
        }
    }
}

// SAFETY: The length prefix consists of bytes and elements are `TrivialType`. There is no padding
// before elements: the size of the length prefix is a multiple of `Element::ALIGNMENT`, and so of
// the alignment of elements, which is a smaller or equal power of two (`Alignment::of()` checks
// it). There is no padding after elements either: their size is a multiple of their alignment,
// which is the alignment of the container. `ALIGNMENT` is that of elements, like in metadata. The
// parsing step checks the length and every element.
unsafe impl<Element, const CAPACITY: usize> TrivialType for FixedCapacityElements<Element, CAPACITY>
where
    Element: TrivialType,
    [(); SUPPORTED_CAPACITY::<Element, CAPACITY>]:,
{
    const ALIGNMENT: Alignment = Element::ALIGNMENT;
    // Casting `CAPACITY` to `u32` is lossless, which is checked by the `SUPPORTED_CAPACITY` bound
    const METADATA: &[u8] = {
        #[inline(always)]
        const fn metadata(
            capacity: u32,
            inner_metadata: &[u8],
        ) -> ([u8; MAX_METADATA_CAPACITY], usize) {
            concat_metadata_sources(&[
                &[IoTypeMetadataKind::FixedCapacityElements as u8],
                &capacity.to_le_bytes(),
                inner_metadata,
            ])
        }

        // The reference extends the lifetime of metadata scratch to that of the constant
        let (metadata_scratch, metadata_size) = &metadata(CAPACITY as u32, Element::METADATA);
        metadata_scratch.split_at(*metadata_size).0
    };

    #[inline(always)]
    #[expect(
        clippy::cast_ptr_alignment,
        reason = "The length is only read as `u32` when `bytes` are aligned for it"
    )]
    unsafe fn parse_step<const ALIGNED: bool>(bytes: &[u8]) -> Option<(&Unaligned<Self>, &[u8])> {
        let (bytes, remainder) = bytes.split_at_checked(size_of::<Self>())?;
        let len = if ALIGNED && align_of::<Self>() >= align_of::<u32>() {
            // SAFETY: `bytes` are aligned for `Self` (function contract), whose alignment is at
            // least that of `u32` as checked above, and start with the length prefix of at least
            // 4 bytes
            u32::from_le(unsafe { bytes.as_ptr().cast::<u32>().read() })
        } else {
            u32::from_le_bytes(*bytes.first_chunk()?)
        };
        // Casting `CAPACITY` to `u32` is lossless, which is checked by the `SUPPORTED_CAPACITY`
        // bound
        if len > CAPACITY as u32 {
            return None;
        }
        // SAFETY: If `ALIGNED`, `bytes` are aligned for `Self` (function contract), which has the
        // alignment of elements, and elements start after the length prefix, whose size is a
        // multiple of their alignment (see above)
        unsafe { try_skip_valid::<Element, ALIGNED>(bytes.get(LEN_SIZE::<Element>..)?, CAPACITY) }?;
        // SAFETY: The length prefix consists of bytes, the length doesn't exceed `CAPACITY` and
        // every element is a valid value as checked above, they follow each other without padding
        // (see above), `Unaligned<Self>` has alignment 1 and the size of `Self`
        let value = unsafe { bytes.as_ptr().cast::<Unaligned<Self>>().as_ref_unchecked() };
        Some((value, remainder))
    }
}

impl<Element, const CAPACITY: usize> FixedCapacityElements<Element, CAPACITY>
where
    Element: TrivialType,
    [(); SUPPORTED_CAPACITY::<Element, CAPACITY>]:,
{
    /// Try to create an instance from provided elements.
    ///
    /// Unused capacity is filled with default elements.
    ///
    /// Returns `None` if provided elements do not fit into the capacity.
    #[inline(always)]
    pub fn try_from_slice(elements: &[Element]) -> Option<Self>
    where
        Element: Default,
    {
        let mut instance = Self::default();
        if !instance.copy_from(elements) {
            return None;
        }

        Some(instance)
    }

    /// Access to stored elements.
    ///
    /// Returns no elements if the number of elements exceeds `CAPACITY`, which is only possible for
    /// an instance created from invalid bytes.
    #[inline(always)]
    pub fn get_elements(&self) -> &[Element] {
        self.elements.get(..self.len() as usize).unwrap_or_default()
    }

    /// Exclusive access to stored elements.
    ///
    /// Returns no elements if the number of elements exceeds `CAPACITY`, which is only possible for
    /// an instance created from invalid bytes.
    #[inline(always)]
    pub fn get_elements_mut(&mut self) -> &mut [Element] {
        let len = self.len() as usize;
        self.elements.get_mut(..len).unwrap_or_default()
    }

    /// Number of stored elements.
    ///
    /// May exceed `CAPACITY` for an instance created from invalid bytes.
    #[inline(always)]
    pub const fn len(&self) -> u32 {
        u32::from_le_bytes(
            *self
                .len
                .first_chunk()
                .expect("Length prefix is at least 4 bytes; qed"),
        )
    }

    /// Returns `true` if there are no stored elements
    #[inline(always)]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Append some elements.
    ///
    /// `true` is returned on success, but if there isn't enough capacity left, `false` is.
    #[inline(always)]
    #[must_use = "Operation may fail"]
    pub fn append(&mut self, elements: &[Element]) -> bool {
        let len = self.len();
        let Some(new_len) = u32::try_from(elements.len())
            .ok()
            .and_then(|count| len.checked_add(count))
        else {
            return false;
        };
        // `None` if the new length exceeds the capacity
        let Some(target) = self.elements.get_mut(len as usize..new_len as usize) else {
            return false;
        };

        target.copy_from_slice(elements);
        self.set_len(new_len);

        true
    }

    /// Truncate stored elements to this length.
    ///
    /// Returns `true` on success or `false` if `new_len` is larger than [`Self::len()`].
    #[inline(always)]
    #[must_use = "Operation may fail"]
    pub const fn truncate(&mut self, new_len: u32) -> bool {
        if new_len > self.len() {
            return false;
        }

        self.set_len(new_len);

        true
    }

    /// Copy from specified elements, replacing stored elements.
    ///
    /// Returns `false` if capacity is not enough to copy contents of `src`
    #[inline(always)]
    #[must_use = "Operation may fail"]
    pub fn copy_from(&mut self, src: &[Element]) -> bool {
        let Ok(new_len) = u32::try_from(src.len()) else {
            return false;
        };
        let Some(target) = self.elements.get_mut(..src.len()) else {
            return false;
        };

        target.copy_from_slice(src);
        self.set_len(new_len);

        true
    }

    /// Store the length in the first 4 bytes of the length prefix, the rest of it is zeroed
    #[inline(always)]
    const fn set_len(&mut self, len: u32) {
        self.len = [0; LEN_SIZE::<Element>];
        *self
            .len
            .first_chunk_mut()
            .expect("Length prefix is at least 4 bytes; qed") = len.to_le_bytes();
    }
}
