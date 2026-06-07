//! Composable RISC-V primitives (instructions, registers) and abstractions around them.
//!
//! The primitives are designed to be generic over the number of general purpose registers, and a
//! macro system allows composing base ISA like RV32/RV64 with a desired set of standard or custom
//! extensions/instructions. Trait abstractions are designed to allow expressing generic APIs
//! without hardcoding specific types whenever possible.
//!
//! The immediate needs dictate the current set of available instructions and extensions. Consider
//! contributing if you need something not yet available.
//!
//! `ab-riscv-interpreter` crate contains a complementary interpreter implementation, but these
//! primitives are completely independent.
//!
//! `ab-riscv-act4-runner` crate in the repository contains a complementary RISC-V Architectural
//! Certification Tests runner for <https://github.com/riscv-non-isa/riscv-arch-test> that ensures
//! correct implementation.
//!
//! Does not require a standard library (`no_std`) or an allocator.
//!
//! ## Supported ISA variants and extensions
//!
//! ISA variants:
//! * RV32I (version 2.1)
//! * RV32E (version 2.0)
//! * RV64I (version 2.1)
//! * RV64E (version 2.0)
//!
//! Extensions:
//! * M (version 2.0)
//! * B (version 1.0.0)
//! * Zba (version 1.0.0)
//! * Zbb (version 1.0.0)
//! * Zbc (version 1.0.0)
//! * Zbkb (version 1.0.1)
//! * Zbkc (version 1.0.1)
//! * Zbkx (version 1.0.1)
//! * Zbs (version 1.0.0)
//! * Zca (version 1.0.0)
//! * Zcb (version 1.0.0)
//! * (experimental) Zcmp (version 1.0.0)
//! * Zkn (version 1.0.1)
//! * Zknd (version 1.0.1)
//! * Zkne (version 1.0.1)
//! * Zknh (version 1.0.1)
//! * Zicond (version 2.0)
//! * Zicsr (version 2.0)
//! * ZveXx (version 1.0.0), where `X` is anything allowed by the specification like Zve32x or
//!   Zve64x
//! * Zvl*b (version 1.0.0), where `*` is anything allowed by the specification like Zvl128b or
//!   Zvl512b
//!
//! All extensions except experimental pass all relevant RISC-V Architectural Certification Tests
//! (ACTs) using the ACT4 framework.
//!
//! Any permutation of compatible extensions is supported.
//!
//! Experimental extensions are known to have bugs and need more work. They are not tested against
//! ACTs yet.

#![no_std]
#![feature(
    const_cmp,
    const_convert,
    const_default,
    const_destruct,
    const_ops,
    const_option_ops,
    const_trait_impl,
    const_try,
    const_try_residual,
    exact_div,
    generic_const_exprs,
    stmt_expr_attributes,
    try_blocks
)]
#![expect(incomplete_features, reason = "generic_const_exprs")]

pub mod instructions;
pub mod prelude;
pub mod privilege;
pub mod registers;
