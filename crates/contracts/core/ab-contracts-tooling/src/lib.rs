//! Low-level tooling around Abundance contracts

#![feature(try_blocks)]

pub mod build;
pub mod convert;
pub mod recover;
pub mod target_specification;

/// Environment used in target specification
pub const TARGET_ENV: &str = "abundance";
