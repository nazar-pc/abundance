#![expect(incomplete_features, reason = "generic_const_exprs")]
#![feature(
    array_chunks,
    generic_const_exprs,
    maybe_uninit_slice,
    maybe_uninit_uninit_array_transpose,
    ptr_as_ref_unchecked,
    trusted_len
)]
#![no_std]

pub mod balanced_hashed;
