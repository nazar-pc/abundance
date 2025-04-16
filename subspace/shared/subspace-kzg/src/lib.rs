//! KZG primitives for Subspace Network
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod tests;

extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::string::String;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use core::mem;
use derive_more::{AsRef, Deref, DerefMut, From, Into};
use kzg::Fr;
use rust_kzg_blst::types::fr::FsFr;
use static_assertions::const_assert_eq;
use subspace_core_primitives::ScalarBytes;

/// Representation of a single BLS12-381 scalar value.
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, Deref, DerefMut)]
#[repr(transparent)]
pub struct Scalar(FsFr);

const_assert_eq!(
    mem::size_of::<Option<Scalar>>(),
    mem::size_of::<Option<FsFr>>()
);
const_assert_eq!(
    mem::align_of::<Option<Scalar>>(),
    mem::align_of::<Option<FsFr>>()
);

impl From<&[u8; ScalarBytes::SAFE_BYTES]> for Scalar {
    #[inline]
    fn from(value: &[u8; ScalarBytes::SAFE_BYTES]) -> Self {
        let mut bytes = [0u8; ScalarBytes::FULL_BYTES];
        bytes[1..].copy_from_slice(value);
        Self::try_from(bytes).expect("Safe bytes always fit into scalar and thus succeed; qed")
    }
}

impl From<[u8; ScalarBytes::SAFE_BYTES]> for Scalar {
    #[inline]
    fn from(value: [u8; ScalarBytes::SAFE_BYTES]) -> Self {
        Self::from(&value)
    }
}

impl TryFrom<&[u8; ScalarBytes::FULL_BYTES]> for Scalar {
    type Error = String;

    #[inline]
    fn try_from(value: &[u8; ScalarBytes::FULL_BYTES]) -> Result<Self, Self::Error> {
        Self::try_from(*value)
    }
}

impl TryFrom<[u8; ScalarBytes::FULL_BYTES]> for Scalar {
    type Error = String;

    #[inline]
    fn try_from(value: [u8; ScalarBytes::FULL_BYTES]) -> Result<Self, Self::Error> {
        FsFr::from_bytes(&value).map(Scalar)
    }
}

impl TryFrom<&ScalarBytes> for Scalar {
    type Error = String;

    #[inline]
    fn try_from(value: &ScalarBytes) -> Result<Self, Self::Error> {
        FsFr::from_bytes(value.as_ref()).map(Scalar)
    }
}

impl TryFrom<ScalarBytes> for Scalar {
    type Error = String;

    #[inline]
    fn try_from(value: ScalarBytes) -> Result<Self, Self::Error> {
        Self::try_from(&value)
    }
}

impl From<&Scalar> for [u8; ScalarBytes::FULL_BYTES] {
    #[inline]
    fn from(value: &Scalar) -> Self {
        value.0.to_bytes()
    }
}

impl From<Scalar> for [u8; ScalarBytes::FULL_BYTES] {
    #[inline]
    fn from(value: Scalar) -> Self {
        Self::from(&value)
    }
}

impl From<&Scalar> for ScalarBytes {
    #[inline]
    fn from(value: &Scalar) -> Self {
        ScalarBytes::from(value.0.to_bytes())
    }
}

impl From<Scalar> for ScalarBytes {
    #[inline]
    fn from(value: Scalar) -> Self {
        Self::from(&value)
    }
}

impl Scalar {
    /// Convert scalar into bytes
    pub fn to_bytes(&self) -> [u8; ScalarBytes::FULL_BYTES] {
        self.into()
    }

    /// Convert scalar into safe bytes, returns `None` if not possible to convert due to larger
    /// internal value
    pub fn try_to_safe_bytes(&self) -> Option<[u8; ScalarBytes::SAFE_BYTES]> {
        let bytes = self.to_bytes();
        if bytes[0] == 0 {
            Some(bytes[1..].try_into().expect("Correct length; qed"))
        } else {
            None
        }
    }

    /// Convenient conversion from slice of scalar to underlying representation for efficiency
    /// purposes.
    #[inline]
    pub fn slice_to_repr(value: &[Self]) -> &[FsFr] {
        // SAFETY: `Scalar` is `#[repr(transparent)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion from slice of underlying representation to scalar for efficiency
    /// purposes.
    #[inline]
    pub fn slice_from_repr(value: &[FsFr]) -> &[Self] {
        // SAFETY: `Scalar` is `#[repr(transparent)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion from slice of optional scalar to underlying representation for efficiency
    /// purposes.
    #[inline]
    pub fn slice_option_to_repr(value: &[Option<Self>]) -> &[Option<FsFr>] {
        // SAFETY: `Scalar` is `#[repr(transparent)]` containing `#[repr(C)]` and we assume the
        // compiler lays out optional `repr(C)` plain old data arrays the same as their optional
        // transparent wrappers
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion from slice of optional underlying representation to scalar for efficiency
    /// purposes.
    #[inline]
    pub fn slice_option_from_repr(value: &[Option<FsFr>]) -> &[Option<Self>] {
        // SAFETY: `Scalar` is `#[repr(transparent)]` containing `#[repr(C)]` and we assume the
        // compiler lays out optional `repr(C)` plain old data arrays the same as their optional
        // transparent wrappers
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion from mutable slice of scalar to underlying representation for
    /// efficiency purposes.
    #[inline]
    pub fn slice_mut_to_repr(value: &mut [Self]) -> &mut [FsFr] {
        // SAFETY: `Scalar` is `#[repr(transparent)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion from mutable slice of underlying representation to scalar for
    /// efficiency purposes.
    #[inline]
    pub fn slice_mut_from_repr(value: &mut [FsFr]) -> &mut [Self] {
        // SAFETY: `Scalar` is `#[repr(transparent)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion from vector of scalar to underlying representation for efficiency
    /// purposes.
    #[inline]
    pub fn vec_to_repr(value: Vec<Self>) -> Vec<FsFr> {
        // SAFETY: `Scalar` is `#[repr(transparent)]` and guaranteed to have the same memory
        //  layout, original vector is not dropped
        unsafe {
            let mut value = mem::ManuallyDrop::new(value);
            Vec::from_raw_parts(
                value.as_mut_ptr() as *mut FsFr,
                value.len(),
                value.capacity(),
            )
        }
    }

    /// Convenient conversion from vector of underlying representation to scalar for efficiency
    /// purposes.
    #[inline]
    pub fn vec_from_repr(value: Vec<FsFr>) -> Vec<Self> {
        // SAFETY: `Scalar` is `#[repr(transparent)]` and guaranteed to have the same memory
        //  layout, original vector is not dropped
        unsafe {
            let mut value = mem::ManuallyDrop::new(value);
            Vec::from_raw_parts(
                value.as_mut_ptr() as *mut Self,
                value.len(),
                value.capacity(),
            )
        }
    }
}
