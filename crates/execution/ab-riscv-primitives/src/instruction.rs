//! This module defines the RISC-V instruction set for the RV64 architecture

pub mod b_64_ext;
pub mod m_64_ext;
pub mod rv64;
#[cfg(test)]
mod test_utils;
pub mod tuples;

use crate::registers::GenericRegister;
use core::fmt;
use core::marker::Destruct;

/// Generic instruction
pub const trait GenericInstruction:
    fmt::Display + fmt::Debug + [const] Destruct + Copy + Sized
{
    /// Lower-level instruction like [`Rv64Instruction`]
    ///
    /// [`Rv64Instruction`]: rv64::Rv64Instruction
    type Base: GenericBaseInstruction;

    /// Try to decode a single valid instruction
    fn try_decode(instruction: u32) -> Option<Self>;

    /// Instruction size in bytes
    fn size(&self) -> usize;
}

/// Generic base instruction
pub const trait GenericBaseInstruction: [const] GenericInstruction {
    /// A register type used by the instruction
    type Reg: GenericRegister;

    /// Create an instruction from a lower-level base instruction
    fn from_base(base: Self::Base) -> Self;

    /// Decode a single instruction
    fn decode(instruction: u32) -> Self;
}
