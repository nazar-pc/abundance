//! This module defines the RISC-V instruction set for the RV64 architecture

pub mod rv64;
#[cfg(test)]
mod test_utils;

use crate::registers::Register;
use core::fmt;
use core::marker::Destruct;

/// Generic instruction
pub const trait Instruction:
    fmt::Display + fmt::Debug + [const] Destruct + Copy + Sized
{
    /// A register type used by the instruction
    type Reg: Register;

    /// Try to decode a single valid instruction
    fn try_decode(instruction: u32) -> Option<Self>;

    /// Instruction alignment in bytes
    fn alignment() -> u8;

    /// Instruction size in bytes
    fn size(&self) -> u8;
}
