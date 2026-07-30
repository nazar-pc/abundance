//! RV64 Zawrs extension

#[cfg(test)]
mod tests;

use crate::instructions::Instruction;
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV64 Zawrs instruction (Wait-on-Reservation-Set)
#[instruction]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
pub enum Rv64ZawrsInstruction<Reg> {
    /// Wait-on-Reservation-Set, no timeout
    WrsNto,
    /// Wait-on-Reservation-Set, short timeout
    WrsSto,
}

#[instruction]
const impl<Reg> Instruction for Rv64ZawrsInstruction<Reg>
where
    Reg: [const] Register<Type = u64>,
{
    type Reg = Reg;

    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn try_decode(instruction: u32) -> Option<Self> {
        let opcode = (instruction & 0b111_1111) as u8;
        let rd_bits = ((instruction >> 7) & 0x1f) as u8;
        let funct3 = ((instruction >> 12) & 0b111) as u8;
        let rs1_bits = ((instruction >> 15) & 0x1f) as u8;
        let imm = (instruction >> 20) & 0xfff;

        match (opcode, funct3, rd_bits, rs1_bits, imm) {
            (0b111_0011, 0b000, 0, 0, 0x00d) => Some(Self::WrsNto),
            (0b111_0011, 0b000, 0, 0, 0x01d) => Some(Self::WrsSto),
            _ => None,
        }
    }

    #[inline(always)]
    fn alignment() -> u8 {
        align_of::<u32>() as u8
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

#[instruction]
impl<Reg> fmt::Display for Rv64ZawrsInstruction<Reg>
where
    Reg: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrsNto => write!(f, "wrs.nto"),
            Self::WrsSto => write!(f, "wrs.sto"),
        }
    }
}
