//! Compliance tests for RISC-V interpreter
#![feature(const_trait_impl, const_try, const_try_residual, try_blocks)]
#![expect(incomplete_features, reason = "generic_const_exprs")]
// TODO: This feature is not actually used in this crate, but is added as a workaround for
//  https://github.com/rust-lang/rust/issues/141492
#![feature(generic_const_exprs)]
#![no_std]

pub mod rv64;

/// Path to the `riscv-arch-test` repo
pub const RISCV_ARCH_TEST_REPO_PATH: &str = concat!(env!("OUT_DIR"), "/riscv-arch-test");
