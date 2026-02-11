#![no_std]
#![feature(
    const_cmp,
    const_convert,
    const_default,
    const_destruct,
    const_index,
    const_option_ops,
    const_ops,
    const_trait_impl,
    const_try,
    const_try_residual,
    generic_const_exprs,
    stmt_expr_attributes,
    try_blocks
)]
#![expect(incomplete_features, reason = "generic_const_exprs")]

pub mod instruction;
pub mod registers;
