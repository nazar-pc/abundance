//! RV64 Zve64x integer arithmetic instructions (single-width)

#[cfg(test)]
mod tests;

use crate::instructions::Instruction;
use crate::registers::general_purpose::Register;
use crate::registers::vector::VReg;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV64 Zve64x single-width integer arithmetic instruction.
///
/// Covers: add, sub, reverse-sub, bitwise logic, shifts, compares, min/max. All use the OP-V major
/// opcode (0b1010111) with OPIVV/OPIVX/OPIVI funct3.
///
/// Vector arithmetic format:
/// `[funct6(6)|vm(1)|vs2(5)|vs1/rs1/imm(5)|funct3(3)|vd(5)|1010111(7)]`
///
/// funct3 selects an operand type:
/// - OPIVV = 0b000: vector-vector
/// - OPIVX = 0b100: vector-scalar (x register)
/// - OPIVI = 0b011: vector-immediate (5-bit signed or unsigned)
#[instruction]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[rustfmt::skip]
pub(super) enum Rv64Zve64xArithInstruction<Reg> {
    // Single-Width Integer Add/Subtract (Section 12.1)

    /// `vadd.vv vd, vs2, vs1, vm`
    VaddVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vadd.vx vd, vs2, rs1, vm`
    VaddVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vadd.vi vd, vs2, imm, vm`
    VaddVi { vd: VReg, vs2: VReg, imm: i8, vm: bool },
    /// `vsub.vv vd, vs2, vs1, vm`
    VsubVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vsub.vx vd, vs2, rs1, vm`
    VsubVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vrsub.vx vd, vs2, rs1, vm`
    VrsubVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vrsub.vi vd, vs2, imm, vm`
    VrsubVi { vd: VReg, vs2: VReg, imm: i8, vm: bool },

    // Bitwise Logical (Section 12.5)

    /// `vand.vv vd, vs2, vs1, vm`
    VandVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vand.vx vd, vs2, rs1, vm`
    VandVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vand.vi vd, vs2, imm, vm`
    VandVi { vd: VReg, vs2: VReg, imm: i8, vm: bool },
    /// `vor.vv vd, vs2, vs1, vm`
    VorVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vor.vx vd, vs2, rs1, vm`
    VorVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vor.vi vd, vs2, imm, vm`
    VorVi { vd: VReg, vs2: VReg, imm: i8, vm: bool },
    /// `vxor.vv vd, vs2, vs1, vm`
    VxorVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vxor.vx vd, vs2, rs1, vm`
    VxorVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vxor.vi vd, vs2, imm, vm`
    VxorVi { vd: VReg, vs2: VReg, imm: i8, vm: bool },

    // Single-Width Shift (Section 12.6)

    /// `vsll.vv vd, vs2, vs1, vm`
    VsllVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vsll.vx vd, vs2, rs1, vm`
    VsllVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vsll.vi vd, vs2, uimm, vm`
    VsllVi { vd: VReg, vs2: VReg, uimm: u8, vm: bool },
    /// `vsrl.vv vd, vs2, vs1, vm`
    VsrlVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vsrl.vx vd, vs2, rs1, vm`
    VsrlVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vsrl.vi vd, vs2, uimm, vm`
    VsrlVi { vd: VReg, vs2: VReg, uimm: u8, vm: bool },
    /// `vsra.vv vd, vs2, vs1, vm`
    VsraVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vsra.vx vd, vs2, rs1, vm`
    VsraVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vsra.vi vd, vs2, uimm, vm`
    VsraVi { vd: VReg, vs2: VReg, uimm: u8, vm: bool },

    // Integer Min/Max (Section 12.9)

    /// `vminu.vv vd, vs2, vs1, vm`
    VminuVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vminu.vx vd, vs2, rs1, vm`
    VminuVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vmin.vv vd, vs2, vs1, vm`
    VminVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vmin.vx vd, vs2, rs1, vm`
    VminVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vmaxu.vv vd, vs2, vs1, vm`
    VmaxuVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vmaxu.vx vd, vs2, rs1, vm`
    VmaxuVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vmax.vv vd, vs2, vs1, vm`
    VmaxVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vmax.vx vd, vs2, rs1, vm`
    VmaxVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },

    // Integer Compare (Section 12.8)

    /// `vmseq.vv vd, vs2, vs1, vm`
    VmseqVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vmseq.vx vd, vs2, rs1, vm`
    VmseqVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vmseq.vi vd, vs2, imm, vm`
    VmseqVi { vd: VReg, vs2: VReg, imm: i8, vm: bool },
    /// `vmsne.vv vd, vs2, vs1, vm`
    VmsneVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vmsne.vx vd, vs2, rs1, vm`
    VmsneVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vmsne.vi vd, vs2, imm, vm`
    VmsneVi { vd: VReg, vs2: VReg, imm: i8, vm: bool },
    /// `vmsltu.vv vd, vs2, vs1, vm`
    VmsltuVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vmsltu.vx vd, vs2, rs1, vm`
    VmsltuVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vmslt.vv vd, vs2, vs1, vm`
    VmsltVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vmslt.vx vd, vs2, rs1, vm`
    VmsltVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vmsleu.vv vd, vs2, vs1, vm`
    VmsleuVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vmsleu.vx vd, vs2, rs1, vm`
    VmsleuVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vmsleu.vi vd, vs2, imm, vm`
    VmsleuVi { vd: VReg, vs2: VReg, imm: i8, vm: bool },
    /// `vmsle.vv vd, vs2, vs1, vm`
    VmsleVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vmsle.vx vd, vs2, rs1, vm`
    VmsleVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vmsle.vi vd, vs2, imm, vm`
    VmsleVi { vd: VReg, vs2: VReg, imm: i8, vm: bool },
    /// `vmsgtu.vx vd, vs2, rs1, vm`
    VmsgtuVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vmsgtu.vi vd, vs2, imm, vm`
    VmsgtuVi { vd: VReg, vs2: VReg, imm: i8, vm: bool },
    /// `vmsgt.vx vd, vs2, rs1, vm`
    VmsgtVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vmsgt.vi vd, vs2, imm, vm`
    VmsgtVi { vd: VReg, vs2: VReg, imm: i8, vm: bool },
}

#[instruction]
impl<Reg> const Instruction for Rv64Zve64xArithInstruction<Reg>
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

        match funct3 {
            // OPIVV
            0b000 => {
                let vs1 = VReg::from_bits(vs1_bits)?;
                match funct6 {
                    0b000000 => Some(Self::VaddVv { vd, vs2, vs1, vm }),
                    0b000010 => Some(Self::VsubVv { vd, vs2, vs1, vm }),
                    0b001001 => Some(Self::VandVv { vd, vs2, vs1, vm }),
                    0b001010 => Some(Self::VorVv { vd, vs2, vs1, vm }),
                    0b001011 => Some(Self::VxorVv { vd, vs2, vs1, vm }),
                    0b100101 => Some(Self::VsllVv { vd, vs2, vs1, vm }),
                    0b101000 => Some(Self::VsrlVv { vd, vs2, vs1, vm }),
                    0b101001 => Some(Self::VsraVv { vd, vs2, vs1, vm }),
                    0b000100 => Some(Self::VminuVv { vd, vs2, vs1, vm }),
                    0b000101 => Some(Self::VminVv { vd, vs2, vs1, vm }),
                    0b000110 => Some(Self::VmaxuVv { vd, vs2, vs1, vm }),
                    0b000111 => Some(Self::VmaxVv { vd, vs2, vs1, vm }),
                    0b011000 => Some(Self::VmseqVv { vd, vs2, vs1, vm }),
                    0b011001 => Some(Self::VmsneVv { vd, vs2, vs1, vm }),
                    0b011010 => Some(Self::VmsltuVv { vd, vs2, vs1, vm }),
                    0b011011 => Some(Self::VmsltVv { vd, vs2, vs1, vm }),
                    0b011100 => Some(Self::VmsleuVv { vd, vs2, vs1, vm }),
                    0b011101 => Some(Self::VmsleVv { vd, vs2, vs1, vm }),
                    _ => None,
                }
            }
            // OPIVX
            0b100 => {
                let rs1 = Reg::from_bits(vs1_bits)?;
                match funct6 {
                    0b000000 => Some(Self::VaddVx { vd, vs2, rs1, vm }),
                    0b000010 => Some(Self::VsubVx { vd, vs2, rs1, vm }),
                    0b000011 => Some(Self::VrsubVx { vd, vs2, rs1, vm }),
                    0b001001 => Some(Self::VandVx { vd, vs2, rs1, vm }),
                    0b001010 => Some(Self::VorVx { vd, vs2, rs1, vm }),
                    0b001011 => Some(Self::VxorVx { vd, vs2, rs1, vm }),
                    0b100101 => Some(Self::VsllVx { vd, vs2, rs1, vm }),
                    0b101000 => Some(Self::VsrlVx { vd, vs2, rs1, vm }),
                    0b101001 => Some(Self::VsraVx { vd, vs2, rs1, vm }),
                    0b000100 => Some(Self::VminuVx { vd, vs2, rs1, vm }),
                    0b000101 => Some(Self::VminVx { vd, vs2, rs1, vm }),
                    0b000110 => Some(Self::VmaxuVx { vd, vs2, rs1, vm }),
                    0b000111 => Some(Self::VmaxVx { vd, vs2, rs1, vm }),
                    0b011000 => Some(Self::VmseqVx { vd, vs2, rs1, vm }),
                    0b011001 => Some(Self::VmsneVx { vd, vs2, rs1, vm }),
                    0b011010 => Some(Self::VmsltuVx { vd, vs2, rs1, vm }),
                    0b011011 => Some(Self::VmsltVx { vd, vs2, rs1, vm }),
                    0b011100 => Some(Self::VmsleuVx { vd, vs2, rs1, vm }),
                    0b011101 => Some(Self::VmsleVx { vd, vs2, rs1, vm }),
                    0b011110 => Some(Self::VmsgtuVx { vd, vs2, rs1, vm }),
                    0b011111 => Some(Self::VmsgtVx { vd, vs2, rs1, vm }),
                    _ => None,
                }
            }
            // OPIVI
            0b011 => {
                match funct6 {
                    // Shift immediates are unsigned (uimm[4:0])
                    0b100101 => {
                        let uimm = vs1_bits;
                        Some(Self::VsllVi { vd, vs2, uimm, vm })
                    }
                    0b101000 => {
                        let uimm = vs1_bits;
                        Some(Self::VsrlVi { vd, vs2, uimm, vm })
                    }
                    0b101001 => {
                        let uimm = vs1_bits;
                        Some(Self::VsraVi { vd, vs2, uimm, vm })
                    }
                    // Compare immediates with unsigned interpretation
                    0b011110 => {
                        // Sign-extend a 5-bit immediate to i8
                        let imm = (vs1_bits << 3).cast_signed() >> 3;
                        Some(Self::VmsgtuVi { vd, vs2, imm, vm })
                    }
                    0b011111 => {
                        // Sign-extend a 5-bit immediate to i8
                        let imm = (vs1_bits << 3).cast_signed() >> 3;
                        Some(Self::VmsgtVi { vd, vs2, imm, vm })
                    }
                    // All other OPIVI use sign-extended 5-bit immediate
                    _ => {
                        // Sign-extend a 5-bit immediate to i8
                        let imm = (vs1_bits << 3).cast_signed() >> 3;
                        match funct6 {
                            0b000000 => Some(Self::VaddVi { vd, vs2, imm, vm }),
                            0b000011 => Some(Self::VrsubVi { vd, vs2, imm, vm }),
                            0b001001 => Some(Self::VandVi { vd, vs2, imm, vm }),
                            0b001010 => Some(Self::VorVi { vd, vs2, imm, vm }),
                            0b001011 => Some(Self::VxorVi { vd, vs2, imm, vm }),
                            0b011000 => Some(Self::VmseqVi { vd, vs2, imm, vm }),
                            0b011001 => Some(Self::VmsneVi { vd, vs2, imm, vm }),
                            0b011100 => Some(Self::VmsleuVi { vd, vs2, imm, vm }),
                            0b011101 => Some(Self::VmsleVi { vd, vs2, imm, vm }),
                            _ => None,
                        }
                    }
                }
            }
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

impl<Reg> fmt::Display for Rv64Zve64xArithInstruction<Reg>
where
    Reg: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[rustfmt::skip]
        match self {
            // Add/Sub
            Self::VaddVv { vd, vs2, vs1, vm } => write!(f, "vadd.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VaddVx { vd, vs2, rs1, vm } => write!(f, "vadd.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VaddVi { vd, vs2, imm, vm } => write!(f, "vadd.vi {vd}, {vs2}, {imm}{}", mask_suffix(vm)),
            Self::VsubVv { vd, vs2, vs1, vm } => write!(f, "vsub.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VsubVx { vd, vs2, rs1, vm } => write!(f, "vsub.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VrsubVx { vd, vs2, rs1, vm } => write!(f, "vrsub.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VrsubVi { vd, vs2, imm, vm } => write!(f, "vrsub.vi {vd}, {vs2}, {imm}{}", mask_suffix(vm)),
            // Logic
            Self::VandVv { vd, vs2, vs1, vm } => write!(f, "vand.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VandVx { vd, vs2, rs1, vm } => write!(f, "vand.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VandVi { vd, vs2, imm, vm } => write!(f, "vand.vi {vd}, {vs2}, {imm}{}", mask_suffix(vm)),
            Self::VorVv { vd, vs2, vs1, vm } => write!(f, "vor.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VorVx { vd, vs2, rs1, vm } => write!(f, "vor.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VorVi { vd, vs2, imm, vm } => write!(f, "vor.vi {vd}, {vs2}, {imm}{}", mask_suffix(vm)),
            Self::VxorVv { vd, vs2, vs1, vm } => write!(f, "vxor.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VxorVx { vd, vs2, rs1, vm } => write!(f, "vxor.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VxorVi { vd, vs2, imm, vm } => write!(f, "vxor.vi {vd}, {vs2}, {imm}{}", mask_suffix(vm)),
            // Shift
            Self::VsllVv { vd, vs2, vs1, vm } => write!(f, "vsll.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VsllVx { vd, vs2, rs1, vm } => write!(f, "vsll.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VsllVi { vd, vs2, uimm, vm } => write!(f, "vsll.vi {vd}, {vs2}, {uimm}{}", mask_suffix(vm)),
            Self::VsrlVv { vd, vs2, vs1, vm } => write!(f, "vsrl.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VsrlVx { vd, vs2, rs1, vm } => write!(f, "vsrl.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VsrlVi { vd, vs2, uimm, vm } => write!(f, "vsrl.vi {vd}, {vs2}, {uimm}{}", mask_suffix(vm)),
            Self::VsraVv { vd, vs2, vs1, vm } => write!(f, "vsra.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VsraVx { vd, vs2, rs1, vm } => write!(f, "vsra.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VsraVi { vd, vs2, uimm, vm } => write!(f, "vsra.vi {vd}, {vs2}, {uimm}{}", mask_suffix(vm)),
            // Min/Max
            Self::VminuVv { vd, vs2, vs1, vm } => write!(f, "vminu.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VminuVx { vd, vs2, rs1, vm } => write!(f, "vminu.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VminVv { vd, vs2, vs1, vm } => write!(f, "vmin.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VminVx { vd, vs2, rs1, vm } => write!(f, "vmin.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VmaxuVv { vd, vs2, vs1, vm } => write!(f, "vmaxu.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VmaxuVx { vd, vs2, rs1, vm } => write!(f, "vmaxu.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VmaxVv { vd, vs2, vs1, vm } => write!(f, "vmax.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VmaxVx { vd, vs2, rs1, vm } => write!(f, "vmax.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            // Compare
            Self::VmseqVv { vd, vs2, vs1, vm } => write!(f, "vmseq.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VmseqVx { vd, vs2, rs1, vm } => write!(f, "vmseq.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VmseqVi { vd, vs2, imm, vm } => write!(f, "vmseq.vi {vd}, {vs2}, {imm}{}", mask_suffix(vm)),
            Self::VmsneVv { vd, vs2, vs1, vm } => write!(f, "vmsne.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VmsneVx { vd, vs2, rs1, vm } => write!(f, "vmsne.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VmsneVi { vd, vs2, imm, vm } => write!(f, "vmsne.vi {vd}, {vs2}, {imm}{}", mask_suffix(vm)),
            Self::VmsltuVv { vd, vs2, vs1, vm } => write!(f, "vmsltu.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VmsltuVx { vd, vs2, rs1, vm } => write!(f, "vmsltu.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VmsltVv { vd, vs2, vs1, vm } => write!(f, "vmslt.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VmsltVx { vd, vs2, rs1, vm } => write!(f, "vmslt.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VmsleuVv { vd, vs2, vs1, vm } => write!(f, "vmsleu.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VmsleuVx { vd, vs2, rs1, vm } => write!(f, "vmsleu.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VmsleuVi { vd, vs2, imm, vm } => write!(f, "vmsleu.vi {vd}, {vs2}, {imm}{}", mask_suffix(vm)),
            Self::VmsleVv { vd, vs2, vs1, vm } => write!(f, "vmsle.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VmsleVx { vd, vs2, rs1, vm } => write!(f, "vmsle.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VmsleVi { vd, vs2, imm, vm } => write!(f, "vmsle.vi {vd}, {vs2}, {imm}{}", mask_suffix(vm)),
            Self::VmsgtuVx { vd, vs2, rs1, vm } => write!(f, "vmsgtu.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VmsgtuVi { vd, vs2, imm, vm } => write!(f, "vmsgtu.vi {vd}, {vs2}, {imm}{}", mask_suffix(vm)),
            Self::VmsgtVx { vd, vs2, rs1, vm } => write!(f, "vmsgt.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VmsgtVi { vd, vs2, imm, vm } => write!(f, "vmsgt.vi {vd}, {vs2}, {imm}{}", mask_suffix(vm)),
        }
    }
}

/// Format mask suffix for display
#[inline(always)]
fn mask_suffix(vm: &bool) -> &'static str {
    if *vm { "" } else { ", v0.t" }
}
