//! Collection of modules used for dealing with archived state of Subspace Network.
#![cfg_attr(not(feature = "std"), no_std)]
#![feature(array_chunks, iter_collect_into)]
#![expect(incomplete_features, reason = "generic_const_exprs")]
// TODO: This feature is not actually used in this crate, but is added as a workaround for
//  https://github.com/rust-lang/rust/issues/139376
#![feature(generic_const_exprs)]

pub mod archiver;
pub mod piece_reconstructor;
pub mod reconstructor;
