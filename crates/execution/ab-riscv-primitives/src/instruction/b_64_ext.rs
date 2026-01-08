//! RV64 B extension

pub mod zba_64_ext;
pub mod zbb_64_ext;
pub mod zbc_64_ext;
pub mod zbs_64_ext;

use crate::instruction::GenericInstruction;
use crate::instruction::b_64_ext::zba_64_ext::Zba64ExtInstruction;
use crate::instruction::b_64_ext::zbb_64_ext::Zbb64ExtInstruction;
use crate::instruction::b_64_ext::zbc_64_ext::Zbc64ExtInstruction;
use crate::instruction::b_64_ext::zbs_64_ext::Zbs64ExtInstruction;
use crate::instruction::rv64::Rv64Instruction;
use crate::registers::GenericRegister;
use core::fmt;

/// RISC-V B (Zba + Zbb + Zbs) + Zbc instruction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BZbc64ExtInstruction<Reg> {
    Zba(Zba64ExtInstruction<Reg>),
    Zbb(Zbb64ExtInstruction<Reg>),
    Zbc(Zbc64ExtInstruction<Reg>),
    Zbs(Zbs64ExtInstruction<Reg>),
}

impl<Reg> const GenericInstruction for BZbc64ExtInstruction<Reg>
where
    Reg: [const] GenericRegister<Type = u64>,
{
    type Base = Rv64Instruction<Reg>;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        if let Some(instruction) = Zba64ExtInstruction::<Reg>::try_decode(instruction) {
            Some(Self::Zba(instruction))
        } else if let Some(instruction) = Zbb64ExtInstruction::<Reg>::try_decode(instruction) {
            Some(Self::Zbb(instruction))
        } else if let Some(instruction) = Zbc64ExtInstruction::<Reg>::try_decode(instruction) {
            Some(Self::Zbc(instruction))
        } else if let Some(instruction) = Zbs64ExtInstruction::<Reg>::try_decode(instruction) {
            Some(Self::Zbs(instruction))
        } else {
            None
        }
    }

    #[inline(always)]
    fn size(&self) -> usize {
        size_of::<u32>()
    }
}

impl<Reg> fmt::Display for BZbc64ExtInstruction<Reg>
where
    Reg: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BZbc64ExtInstruction::Zba(instruction) => instruction.fmt(f),
            BZbc64ExtInstruction::Zbb(instruction) => instruction.fmt(f),
            BZbc64ExtInstruction::Zbc(instruction) => instruction.fmt(f),
            BZbc64ExtInstruction::Zbs(instruction) => instruction.fmt(f),
        }
    }
}
