//! Core primitives for the protocol

#![cfg_attr(target_os = "none", no_std)]
#![warn(rust_2018_idioms, missing_debug_implementations, missing_docs)]
#![feature(
    array_chunks,
    const_trait_impl,
    const_try,
    portable_simd,
    ptr_as_ref_unchecked,
    step_trait,
    trusted_len
)]
#![cfg_attr(feature = "alloc", feature(new_zeroed_alloc))]
#![expect(incomplete_features, reason = "generic_const_exprs")]
// TODO: This feature is not actually used in this crate, but is added as a workaround for
//  https://github.com/rust-lang/rust/issues/141492
#![feature(generic_const_exprs)]

pub mod address;
pub mod balance;
pub mod block;
#[cfg(feature = "scale-codec")]
pub mod checksum;
pub mod ed25519;
pub mod hashes;
pub mod pieces;
pub mod pos;
pub mod pot;
pub mod sectors;
pub mod segments;
pub mod shard;
pub mod solutions;
pub mod transaction;

#[cfg(any(feature = "alloc", not(target_os = "none")))]
extern crate alloc;

const _: () = {
    assert!(
        size_of::<usize>() >= size_of::<u32>(),
        "Must be at least 32-bit platform"
    );
};
