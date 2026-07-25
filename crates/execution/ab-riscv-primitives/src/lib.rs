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
//! Does not require a standard library (`no_std`) or an allocator, never panics, almost 100% of the
//! API is usable in const.
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
//! * A (version 2.1)
//! * M (version 2.0)
//! * B (version 1.0.0)
//! * Zaamo (version 1.0.0)
//! * Zalrsc (version 1.0.0)
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
//! * Zvbb (version 1.0.0)
//! * Zvbc (version 1.0.0)
//! * ZveXx (version 1.0.0), where `X` is anything allowed by the specification like Zve32x or
//!   Zve64x
//! * Zvkb (version 1.0.0)
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
//!
//! ## Design choices
//!
//! This crate was designed with a blockchain use case in mind, though it is in no way tied to any
//! particular blockchain and is completely general purpose. As a result, the implementation is
//! designed to be precise and non-ambiguous.
//!
//! A few key points:
//! * anything "reserved" in the specification is considered to be illegal
//! * anything "optional" in the specification is considered to be illegal
//! * anything "implementation-defined" in the specification is selected to be the most natural and
//!   deterministic
//! * type system is used to make the majority of invalid invariants impossible to represent in code
//!   and/or decode
//!
//! Examples:
//! * Zve64x extension instructions are purposefully restricted to what it is required to be capable
//!   of, although it would be cheaper to support the fuller feature set only required by V
//!   extension

#![no_std]
#![expect(incomplete_features, reason = "generic_const_*")]
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
    derive_const,
    exact_div,
    generic_const_args,
    generic_const_items,
    min_adt_const_params,
    macroless_generic_const_args,
    min_generic_const_args,
    never_type,
    stmt_expr_attributes,
    try_blocks
)]
#![cfg_attr(feature = "no-panic", feature(const_closures))]

pub mod instructions;
pub mod prelude;
pub mod privilege;
pub mod registers;
