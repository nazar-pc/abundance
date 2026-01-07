#![no_std]
#![feature(
    const_cmp,
    const_convert,
    const_default,
    const_destruct,
    const_index,
    const_option_ops,
    const_trait_impl,
    const_try,
    generic_const_exprs
)]
#![expect(incomplete_features, reason = "generic_const_exprs")]

pub mod instruction;
pub mod registers;
