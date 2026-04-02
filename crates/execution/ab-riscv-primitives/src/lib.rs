//! Composable RISC-V primitives (instructions, registers) and abstractions around them.
//!
//! The primitives are designed to be generic over the number of general purpose registers, and a
//! macro system allows composing base ISA like RV64 with a desired set of standard or custom
//! extensions/instructions. Trait abstractions are designed to allow expressing generic APIs
//! without hardcoding specific types whenever possible.
//!
//! The immediate needs dictate the current set of available instructions and extensions. Consider
//! contributing if you need something not yet available.
//!
//! `ab-riscv-interpreter` crate contains a complementary interpreter implementation, but these
//! primitives are completely independent.
//!
//! Does not require a standard library (`no_std`) or an allocator.

#![no_std]
#![feature(
    const_cmp,
    const_convert,
    const_default,
    const_destruct,
    const_index,
    const_ops,
    const_option_ops,
    const_trait_impl,
    const_try,
    const_try_residual,
    generic_const_exprs,
    stmt_expr_attributes,
    try_blocks
)]
#![expect(incomplete_features, reason = "generic_const_exprs")]

pub mod instructions;
pub mod privilege;
pub mod registers;
