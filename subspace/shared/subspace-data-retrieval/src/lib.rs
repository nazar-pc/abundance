//! Fetching data from the archived history of the Subspace Distributed Storage Network.

#![feature(exact_size_is_empty, generic_arg_infer, trusted_len)]

pub mod object_fetcher;
pub mod piece_fetcher;
pub mod piece_getter;
pub mod segment_downloading;
