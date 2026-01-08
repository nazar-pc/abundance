//! This module defines the RISC-V instruction set for the RV64 architecture

pub mod b_64_ext;
pub mod m_64_ext;
pub mod rv64;
#[cfg(test)]
mod test_utils;
pub mod tuples;

use crate::instruction::b_64_ext::BZbc64ExtInstruction;
use crate::instruction::m_64_ext::M64ExtInstruction;
use crate::instruction::rv64::Rv64Instruction;
use crate::instruction::tuples::{Tuple2Instruction, Tuple3Instruction};
use crate::registers::GenericRegister;
use core::fmt;
use core::marker::Destruct;

/// Generic instruction
pub const trait GenericInstruction:
    fmt::Display + fmt::Debug + [const] Destruct + Copy + Sized
{
    /// A register type used by the instruction
    type Reg: GenericRegister;

    /// Try to decode a single valid instruction
    fn try_decode(instruction: u32) -> Option<Self>;

    /// Instruction size in bytes
    fn size(&self) -> usize;
}

/// Generic base instruction
pub const trait GenericBaseInstruction: [const] GenericInstruction {
    /// Lower-level instruction like [`Rv64Instruction`]
    type Base: GenericBaseInstruction;

    /// Create an instruction from a lower-level base instruction
    fn from_base(base: Self::Base) -> Self;

    /// Decode a single instruction
    fn decode(instruction: u32) -> Self;
}

/// Type alias for RV64IM/RV64EM instruction.
///
/// Whether RV64I or RV64E base is used depends on the register type used.
pub type Rv64MInstruction<Reg> = Tuple2Instruction<M64ExtInstruction<Reg>, Rv64Instruction<Reg>>;

/// Type alias for RV64IMBZbc/RV64EMBZbc instruction.
///
/// Whether RV64I or RV64E base is used depends on the register type used.
pub type Rv64MBZbcInstruction<Reg> =
    Tuple3Instruction<M64ExtInstruction<Reg>, BZbc64ExtInstruction<Reg>, Rv64Instruction<Reg>>;
