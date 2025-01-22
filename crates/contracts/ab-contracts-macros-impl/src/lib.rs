#![feature(extract_if, iter_map_windows, let_chains)]

//! See and use `ab-contracts-macros` crate instead, this is its implementation detail

mod contract;

use proc_macro::TokenStream;

/// `#[contract]` macro to derive smart contract implementation, see module description for
/// details.
///
/// This macro is supposed to be applied to an implementation of the struct that in turn implements
/// `IoType` trait.
#[proc_macro_attribute]
pub fn contract(_attr: TokenStream, item: TokenStream) -> TokenStream {
    contract::contract(item.into())
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}
