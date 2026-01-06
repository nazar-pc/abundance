//! This module defines the RISC-V instruction set for the RV64 architecture

pub mod m_64_ext;
pub mod rv64;
#[cfg(test)]
mod test_utils;
pub mod tuples;

use crate::instruction::m_64_ext::M64ExtInstruction;
use crate::instruction::rv64::Rv64Instruction;
use crate::instruction::tuples::Tuple2Instruction;
use core::fmt;
use core::marker::Destruct;

/// Generic instruction
pub const trait GenericInstruction:
    fmt::Display + fmt::Debug + [const] Destruct + Copy + Sized
{
    /// Try to decode a single valid instruction
    fn try_decode(instruction: u32) -> Option<Self>;

    /// Instruction size in bytes
    fn size(&self) -> usize;
}

/// Generic base instruction
pub const trait GenericBaseInstruction: [const] GenericInstruction {
    /// Decode a single instruction
    fn decode(instruction: u32) -> Self;
}

/// Type alias for RV64IM/RV64EM instruction.
///
/// Whether RV64I or RV64E base is used depends on the register type used.
pub type Rv64MInstruction<Reg> = Tuple2Instruction<M64ExtInstruction<Reg>, Rv64Instruction<Reg>>;
