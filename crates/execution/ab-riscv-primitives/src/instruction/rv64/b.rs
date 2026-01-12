//! RV64 B extension

pub mod zba;
pub mod zbb;
pub mod zbc;
pub mod zbs;

use crate::instruction::Instruction;
use crate::instruction::rv64::Rv64Instruction;
use crate::instruction::rv64::b::zba::Rv64ZbaInstruction;
use crate::instruction::rv64::b::zbb::Rv64ZbbInstruction;
use crate::instruction::rv64::b::zbc::Rv64ZbcInstruction;
use crate::instruction::rv64::b::zbs::Rv64ZbsInstruction;
use crate::registers::Register;
use core::fmt;

/// RISC-V RV64 B (Zba + Zbb + Zbs) instruction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rv64BInstruction<Reg> {
    Zba(Rv64ZbaInstruction<Reg>),
    Zbb(Rv64ZbbInstruction<Reg>),
    Zbs(Rv64ZbsInstruction<Reg>),
}

impl<Reg> const Instruction for Rv64BInstruction<Reg>
where
    Reg: [const] Register<Type = u64>,
{
    type Base = Rv64Instruction<Reg>;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        if let Some(instruction) = Rv64ZbaInstruction::<Reg>::try_decode(instruction) {
            Some(Self::Zba(instruction))
        } else if let Some(instruction) = Rv64ZbbInstruction::<Reg>::try_decode(instruction) {
            Some(Self::Zbb(instruction))
        } else if let Some(instruction) = Rv64ZbsInstruction::<Reg>::try_decode(instruction) {
            Some(Self::Zbs(instruction))
        } else {
            None
        }
    }

    #[inline(always)]
    fn alignment() -> u8 {
        size_of::<u32>() as u8
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

impl<Reg> fmt::Display for Rv64BInstruction<Reg>
where
    Reg: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Rv64BInstruction::Zba(instruction) => instruction.fmt(f),
            Rv64BInstruction::Zbb(instruction) => instruction.fmt(f),
            Rv64BInstruction::Zbs(instruction) => instruction.fmt(f),
        }
    }
}

/// RISC-V RV64 B (Zba + Zbb + Zbs) + Zbc instruction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rv64BZbcInstruction<Reg> {
    Zba(Rv64ZbaInstruction<Reg>),
    Zbb(Rv64ZbbInstruction<Reg>),
    Zbc(Rv64ZbcInstruction<Reg>),
    Zbs(Rv64ZbsInstruction<Reg>),
}

impl<Reg> const Instruction for Rv64BZbcInstruction<Reg>
where
    Reg: [const] Register<Type = u64>,
{
    type Base = Rv64Instruction<Reg>;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        if let Some(instruction) = Rv64ZbaInstruction::<Reg>::try_decode(instruction) {
            Some(Self::Zba(instruction))
        } else if let Some(instruction) = Rv64ZbbInstruction::<Reg>::try_decode(instruction) {
            Some(Self::Zbb(instruction))
        } else if let Some(instruction) = Rv64ZbcInstruction::<Reg>::try_decode(instruction) {
            Some(Self::Zbc(instruction))
        } else if let Some(instruction) = Rv64ZbsInstruction::<Reg>::try_decode(instruction) {
            Some(Self::Zbs(instruction))
        } else {
            None
        }
    }

    #[inline(always)]
    fn alignment() -> u8 {
        size_of::<u32>() as u8
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

impl<Reg> fmt::Display for Rv64BZbcInstruction<Reg>
where
    Reg: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Rv64BZbcInstruction::Zba(instruction) => instruction.fmt(f),
            Rv64BZbcInstruction::Zbb(instruction) => instruction.fmt(f),
            Rv64BZbcInstruction::Zbc(instruction) => instruction.fmt(f),
            Rv64BZbcInstruction::Zbs(instruction) => instruction.fmt(f),
        }
    }
}
