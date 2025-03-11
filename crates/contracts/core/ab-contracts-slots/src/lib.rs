#![feature(
    box_vec_non_null,
    non_null_from_ref,
    pointer_is_aligned_to,
    ptr_as_ref_unchecked
)]
#![no_std]

pub mod aligned_buffer;
pub mod slots;

extern crate alloc;
