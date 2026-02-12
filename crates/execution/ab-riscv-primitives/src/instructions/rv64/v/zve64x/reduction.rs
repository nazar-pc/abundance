//! RV64 Zve64x integer reduction instructions

#[cfg(test)]
mod tests;

use crate::instructions::Instruction;
use crate::registers::general_purpose::Register;
use crate::registers::vector::VReg;
use ab_riscv_macros::instruction;
use core::fmt;
use core::marker::PhantomData;

/// RISC-V RV64 Zve64x integer reduction instruction.
///
/// Reduction operations take a vector source `vs2`, an initial scalar value in element 0 of `vs1`,
/// and write the scalar result to element 0 of `vd`.
///
/// Single-width reductions use OPMVV (funct3=0b010). Widening reductions use OPIVV (funct3=0b000).
#[instruction]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[rustfmt::skip]
pub(super) enum Rv64Zve64xReductionInstruction<Reg> {
    /// Sum reduction: `vredsum.vs vd, vs2, vs1, vm`
    Vredsum { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// AND reduction: `vredand.vs vd, vs2, vs1, vm`
    Vredand { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// OR reduction: `vredor.vs vd, vs2, vs1, vm`
    Vredor { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// XOR reduction: `vredxor.vs vd, vs2, vs1, vm`
    Vredxor { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// Unsigned minimum reduction: `vredminu.vs vd, vs2, vs1, vm`
    Vredminu { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// Signed minimum reduction: `vredmin.vs vd, vs2, vs1, vm`
    Vredmin { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// Unsigned maximum reduction: `vredmaxu.vs vd, vs2, vs1, vm`
    Vredmaxu { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// Signed maximum reduction: `vredmax.vs vd, vs2, vs1, vm`
    Vredmax { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// Widening unsigned sum reduction: `vwredsumu.vs vd, vs2, vs1, vm`
    Vwredsumu { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// Widening signed sum reduction: `vwredsum.vs vd, vs2, vs1, vm`
    Vwredsum { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    #[doc(hidden)]
    #[expect(dead_code, reason = "Only used for `Reg` generic")]
    Phantom(PhantomData<Reg>),
}

#[instruction]
impl<Reg> const Instruction for Rv64Zve64xReductionInstruction<Reg>
where
    Reg: [const] Register<Type = u64>,
{
    type Reg = Reg;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        let opcode = (instruction & 0b111_1111) as u8;

        // OP-V major opcode
        if opcode != 0b1010111 {
            None?;
        }

        let vd_bits = ((instruction >> 7) & 0x1f) as u8;
        let funct3 = ((instruction >> 12) & 0b111) as u8;
        let vs1_bits = ((instruction >> 15) & 0x1f) as u8;
        let vs2_bits = ((instruction >> 20) & 0x1f) as u8;
        let vm = ((instruction >> 25) & 1) != 0;
        let funct6 = ((instruction >> 26) & 0b11_1111) as u8;

        let vd = VReg::from_bits(vd_bits)?;
        let vs2 = VReg::from_bits(vs2_bits)?;
        let vs1 = VReg::from_bits(vs1_bits)?;

        match funct3 {
            // OPMVV: single-width integer reductions
            0b010 => match funct6 {
                0b000000 => Some(Self::Vredsum { vd, vs2, vs1, vm }),
                0b000001 => Some(Self::Vredand { vd, vs2, vs1, vm }),
                0b000010 => Some(Self::Vredor { vd, vs2, vs1, vm }),
                0b000011 => Some(Self::Vredxor { vd, vs2, vs1, vm }),
                0b000100 => Some(Self::Vredminu { vd, vs2, vs1, vm }),
                0b000101 => Some(Self::Vredmin { vd, vs2, vs1, vm }),
                0b000110 => Some(Self::Vredmaxu { vd, vs2, vs1, vm }),
                0b000111 => Some(Self::Vredmax { vd, vs2, vs1, vm }),
                _ => None,
            },
            // OPIVV: widening integer reductions
            0b000 => match funct6 {
                0b110000 => Some(Self::Vwredsumu { vd, vs2, vs1, vm }),
                0b110001 => Some(Self::Vwredsum { vd, vs2, vs1, vm }),
                _ => None,
            },
            _ => None,
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

impl<Reg> fmt::Display for Rv64Zve64xReductionInstruction<Reg> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[rustfmt::skip]
        match self {
            Self::Vredsum { vd, vs2, vs1, vm } => write!(f, "vredsum.vs {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::Vredand { vd, vs2, vs1, vm } => write!(f, "vredand.vs {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::Vredor { vd, vs2, vs1, vm } => write!(f, "vredor.vs {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::Vredxor { vd, vs2, vs1, vm } => write!(f, "vredxor.vs {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::Vredminu { vd, vs2, vs1, vm } => write!(f, "vredminu.vs {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::Vredmin { vd, vs2, vs1, vm } => write!(f, "vredmin.vs {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::Vredmaxu { vd, vs2, vs1, vm } => write!(f, "vredmaxu.vs {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::Vredmax { vd, vs2, vs1, vm } => write!(f, "vredmax.vs {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::Vwredsumu { vd, vs2, vs1, vm } => write!(f, "vwredsumu.vs {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::Vwredsum { vd, vs2, vs1, vm } => write!(f, "vwredsum.vs {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::Phantom(_) => unreachable!("Never constructed"),
        }
    }
}

/// Format mask suffix for display
#[inline(always)]
fn mask_suffix(vm: &bool) -> &'static str {
    if *vm { "" } else { ", v0.t" }
}
