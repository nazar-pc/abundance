//! Macros for RISC-V primitives

#![cfg_attr(feature = "build", feature(map_try_insert, try_blocks))]

#[cfg(feature = "build")]
mod build;

#[cfg(feature = "proc-macro")]
pub use ab_riscv_macros_impl::{instruction, instruction_execution};
#[cfg(feature = "build")]
pub use build::process_instruction_macros;
