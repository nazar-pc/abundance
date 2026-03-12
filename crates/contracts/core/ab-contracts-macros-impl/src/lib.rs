//! See and use `ab-contracts-macros` crate instead, this is its implementation detail

#![feature(exact_size_is_empty)]

mod contract;

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn contract(_attr: TokenStream, item: TokenStream) -> TokenStream {
    contract::contract(item.into())
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}
