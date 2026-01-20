//! Macros for RISC-V primitives

#![feature(
    bool_to_result,
    map_try_insert,
    result_option_map_or_default,
    try_blocks
)]

#[cfg(feature = "build")]
mod build;

#[cfg(feature = "proc-macro")]
pub use ab_riscv_macros_impl::{instruction, instruction_execution};
#[cfg(feature = "build")]
pub use build::process_instruction_macros;
