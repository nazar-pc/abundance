//! Core primitives for the protocol

#![no_std]
#![warn(rust_2018_idioms, missing_debug_implementations, missing_docs)]
#![feature(array_chunks, const_trait_impl, const_try, portable_simd, step_trait)]
#![cfg_attr(feature = "alloc", feature(new_zeroed_alloc))]
#![expect(incomplete_features, reason = "generic_const_exprs")]
// TODO: This feature is not actually used in this crate, but is added as a workaround for
//  https://github.com/rust-lang/rust/issues/133199
#![feature(generic_const_exprs)]

pub mod block;
#[cfg(feature = "scale-codec")]
pub mod checksum;
pub mod hashes;
pub mod pieces;
pub mod pos;
pub mod pot;
pub mod sectors;
pub mod segments;
pub mod solutions;

#[cfg(feature = "alloc")]
extern crate alloc;

const _: () = {
    assert!(
        size_of::<usize>() >= size_of::<u32>(),
        "Must be at least 32-bit platform"
    );
};
