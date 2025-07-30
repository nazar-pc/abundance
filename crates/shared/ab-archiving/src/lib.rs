//! Collection of modules used for dealing with archival history.
#![feature(iter_collect_into)]
#![expect(incomplete_features, reason = "generic_const_exprs")]
// TODO: This feature is not actually used in this crate, but is added as a workaround for
//  https://github.com/rust-lang/rust/issues/139376
#![feature(generic_const_exprs)]
#![no_std]

pub mod archiver;
pub mod objects;
pub mod piece_reconstructor;
pub mod reconstructor;

extern crate alloc;
