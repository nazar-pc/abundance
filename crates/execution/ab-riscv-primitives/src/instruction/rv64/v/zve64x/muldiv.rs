//! RV64 Zve64x multiply and divide instructions

#[cfg(test)]
mod tests;

use crate::instruction::Instruction;
use crate::instruction::rv64::v::zve64x::VReg;
use crate::registers::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV64 Zve64x multiply and divide instruction.
///
/// Includes single-width multiply, integer divide, widening multiply, single-width multiply-add,
/// and widening multiply-add instructions. All use the OP-V major opcode (0b1010111) with OPMVV
/// (funct3=0b010) or OPMVX (funct3=0b110).
///
/// Note: In Zve64x, `vmulh`, `vmulhu`, `vmulhsu`, and `vsmul` are not supported for SEW=64 (would
/// require a 128-bit multiplier). The decoder still recognizes these encodings; the SEW restriction
/// is enforced at execution time via `vtype`.
#[instruction]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[rustfmt::skip]
pub(super) enum Rv64Zve64xMulDivInstruction<Reg> {
    // Single-width integer multiply (Section 12.10)

    /// `vmul.vv vd, vs2, vs1, vm` - signed multiply, low bits
    VmulVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vmul.vx vd, vs2, rs1, vm` - signed multiply, low bits
    VmulVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vmulh.vv vd, vs2, vs1, vm` - signed×signed multiply, high bits
    VmulhVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vmulh.vx vd, vs2, rs1, vm` - signed×signed multiply, high bits
    VmulhVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vmulhu.vv vd, vs2, vs1, vm` - unsigned×unsigned multiply, high bits
    VmulhuVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vmulhu.vx vd, vs2, rs1, vm` - unsigned×unsigned multiply, high bits
    VmulhuVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vmulhsu.vv vd, vs2, vs1, vm` - signed×unsigned multiply, high bits
    VmulhsuVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vmulhsu.vx vd, vs2, rs1, vm` - signed×unsigned multiply, high bits
    VmulhsuVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },

    // Integer divide (Section 12.11)

    /// `vdivu.vv vd, vs2, vs1, vm` - unsigned divide
    VdivuVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vdivu.vx vd, vs2, rs1, vm` - unsigned divide
    VdivuVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vdiv.vv vd, vs2, vs1, vm` - signed divide
    VdivVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vdiv.vx vd, vs2, rs1, vm` - signed divide
    VdivVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vremu.vv vd, vs2, vs1, vm` - unsigned remainder
    VremuVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vremu.vx vd, vs2, rs1, vm` - unsigned remainder
    VremuVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vrem.vv vd, vs2, vs1, vm` - signed remainder
    VremVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vrem.vx vd, vs2, rs1, vm` - signed remainder
    VremVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },

    // Widening integer multiply (Section 12.12)

    /// `vwmul.vv vd, vs2, vs1, vm` - signed widening multiply
    VwmulVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vwmul.vx vd, vs2, rs1, vm` - signed widening multiply
    VwmulVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vwmulu.vv vd, vs2, vs1, vm` - unsigned widening multiply
    VwmuluVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vwmulu.vx vd, vs2, rs1, vm` - unsigned widening multiply
    VwmuluVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vwmulsu.vv vd, vs2, vs1, vm` - signed×unsigned widening multiply
    VwmulsuVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vwmulsu.vx vd, vs2, rs1, vm` - signed×unsigned widening multiply
    VwmulsuVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },

    // Single-width integer multiply-add (Section 12.13)

    /// `vmacc.vv vd, vs1, vs2, vm` - vd = vd + vs1 * vs2
    VmaccVv { vd: VReg, vs1: VReg, vs2: VReg, vm: bool },
    /// `vmacc.vx vd, rs1, vs2, vm` - vd = vd + rs1 * vs2
    VmaccVx { vd: VReg, rs1: Reg, vs2: VReg, vm: bool },
    /// `vnmsac.vv vd, vs1, vs2, vm` - vd = vd - vs1 * vs2
    VnmsacVv { vd: VReg, vs1: VReg, vs2: VReg, vm: bool },
    /// `vnmsac.vx vd, rs1, vs2, vm` - vd = vd - rs1 * vs2
    VnmsacVx { vd: VReg, rs1: Reg, vs2: VReg, vm: bool },
    /// `vmadd.vv vd, vs1, vs2, vm` - vd = vs1 * vd + vs2
    VmaddVv { vd: VReg, vs1: VReg, vs2: VReg, vm: bool },
    /// `vmadd.vx vd, rs1, vs2, vm` - vd = rs1 * vd + vs2
    VmaddVx { vd: VReg, rs1: Reg, vs2: VReg, vm: bool },
    /// `vnmsub.vv vd, vs1, vs2, vm` - vd = -(vs1 * vd - vs2)
    VnmsubVv { vd: VReg, vs1: VReg, vs2: VReg, vm: bool },
    /// `vnmsub.vx vd, rs1, vs2, vm` - vd = -(rs1 * vd - vs2)
    VnmsubVx { vd: VReg, rs1: Reg, vs2: VReg, vm: bool },

    // Widening integer multiply-add (Section 12.14)

    /// `vwmaccu.vv vd, vs1, vs2, vm` - unsigned widening multiply-add
    VwmaccuVv { vd: VReg, vs1: VReg, vs2: VReg, vm: bool },
    /// `vwmaccu.vx vd, rs1, vs2, vm` - unsigned widening multiply-add
    VwmaccuVx { vd: VReg, rs1: Reg, vs2: VReg, vm: bool },
    /// `vwmacc.vv vd, vs1, vs2, vm` - signed widening multiply-add
    VwmaccVv { vd: VReg, vs1: VReg, vs2: VReg, vm: bool },
    /// `vwmacc.vx vd, rs1, vs2, vm` - signed widening multiply-add
    VwmaccVx { vd: VReg, rs1: Reg, vs2: VReg, vm: bool },
    /// `vwmaccsu.vv vd, vs1, vs2, vm` - signed×unsigned widening multiply-add
    VwmaccsuVv { vd: VReg, vs1: VReg, vs2: VReg, vm: bool },
    /// `vwmaccsu.vx vd, rs1, vs2, vm` - signed×unsigned widening multiply-add
    VwmaccsuVx { vd: VReg, rs1: Reg, vs2: VReg, vm: bool },
    /// `vwmaccus.vx vd, rs1, vs2, vm` - unsigned×signed widening multiply-add (vx only)
    VwmaccusVx { vd: VReg, rs1: Reg, vs2: VReg, vm: bool },
}

#[instruction]
impl<Reg> const Instruction for Rv64Zve64xMulDivInstruction<Reg>
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
        let vs1_or_rs1_bits = ((instruction >> 15) & 0x1f) as u8;
        let vs2_bits = ((instruction >> 20) & 0x1f) as u8;
        let vm = ((instruction >> 25) & 1) != 0;
        let funct6 = ((instruction >> 26) & 0b111111) as u8;

        let vd = VReg::from_bits(vd_bits)?;
        let vs2 = VReg::from_bits(vs2_bits)?;

        match funct3 {
            // OPMVV: vector-vector
            0b010 => {
                let vs1 = VReg::from_bits(vs1_or_rs1_bits)?;
                match funct6 {
                    // Integer divide
                    0b100000 => Some(Self::VdivuVv { vd, vs2, vs1, vm }),
                    0b100001 => Some(Self::VdivVv { vd, vs2, vs1, vm }),
                    0b100010 => Some(Self::VremuVv { vd, vs2, vs1, vm }),
                    0b100011 => Some(Self::VremVv { vd, vs2, vs1, vm }),
                    // Single-width multiply
                    0b100100 => Some(Self::VmulhuVv { vd, vs2, vs1, vm }),
                    0b100101 => Some(Self::VmulVv { vd, vs2, vs1, vm }),
                    0b100110 => Some(Self::VmulhsuVv { vd, vs2, vs1, vm }),
                    0b100111 => Some(Self::VmulhVv { vd, vs2, vs1, vm }),
                    // Single-width multiply-add
                    0b101001 => Some(Self::VmaddVv { vd, vs1, vs2, vm }),
                    0b101011 => Some(Self::VnmsubVv { vd, vs1, vs2, vm }),
                    0b101101 => Some(Self::VmaccVv { vd, vs1, vs2, vm }),
                    0b101111 => Some(Self::VnmsacVv { vd, vs1, vs2, vm }),
                    // Widening multiply
                    0b111000 => Some(Self::VwmuluVv { vd, vs2, vs1, vm }),
                    0b111010 => Some(Self::VwmulsuVv { vd, vs2, vs1, vm }),
                    0b111011 => Some(Self::VwmulVv { vd, vs2, vs1, vm }),
                    // Widening multiply-add
                    0b111100 => Some(Self::VwmaccuVv { vd, vs1, vs2, vm }),
                    0b111101 => Some(Self::VwmaccVv { vd, vs1, vs2, vm }),
                    0b111111 => Some(Self::VwmaccsuVv { vd, vs1, vs2, vm }),
                    _ => None,
                }
            }
            // OPMVX: vector-scalar
            0b110 => {
                let rs1 = Reg::from_bits(vs1_or_rs1_bits)?;
                match funct6 {
                    // Integer divide
                    0b100000 => Some(Self::VdivuVx { vd, vs2, rs1, vm }),
                    0b100001 => Some(Self::VdivVx { vd, vs2, rs1, vm }),
                    0b100010 => Some(Self::VremuVx { vd, vs2, rs1, vm }),
                    0b100011 => Some(Self::VremVx { vd, vs2, rs1, vm }),
                    // Single-width multiply
                    0b100100 => Some(Self::VmulhuVx { vd, vs2, rs1, vm }),
                    0b100101 => Some(Self::VmulVx { vd, vs2, rs1, vm }),
                    0b100110 => Some(Self::VmulhsuVx { vd, vs2, rs1, vm }),
                    0b100111 => Some(Self::VmulhVx { vd, vs2, rs1, vm }),
                    // Single-width multiply-add
                    0b101001 => Some(Self::VmaddVx { vd, rs1, vs2, vm }),
                    0b101011 => Some(Self::VnmsubVx { vd, rs1, vs2, vm }),
                    0b101101 => Some(Self::VmaccVx { vd, rs1, vs2, vm }),
                    0b101111 => Some(Self::VnmsacVx { vd, rs1, vs2, vm }),
                    // Widening multiply
                    0b111000 => Some(Self::VwmuluVx { vd, vs2, rs1, vm }),
                    0b111010 => Some(Self::VwmulsuVx { vd, vs2, rs1, vm }),
                    0b111011 => Some(Self::VwmulVx { vd, vs2, rs1, vm }),
                    // Widening multiply-add
                    0b111100 => Some(Self::VwmaccuVx { vd, rs1, vs2, vm }),
                    0b111101 => Some(Self::VwmaccVx { vd, rs1, vs2, vm }),
                    0b111110 => Some(Self::VwmaccusVx { vd, rs1, vs2, vm }),
                    0b111111 => Some(Self::VwmaccsuVx { vd, rs1, vs2, vm }),
                    _ => None,
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

impl<Reg> fmt::Display for Rv64Zve64xMulDivInstruction<Reg>
where
    Reg: fmt::Display,
{
    #[rustfmt::skip]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[rustfmt::skip]
        match self {
            // Single-width multiply
            Self::VmulVv { vd, vs2, vs1, vm } => write!(f, "vmul.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VmulVx { vd, vs2, rs1, vm } => write!(f, "vmul.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VmulhVv { vd, vs2, vs1, vm } => write!(f, "vmulh.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VmulhVx { vd, vs2, rs1, vm } => write!(f, "vmulh.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VmulhuVv { vd, vs2, vs1, vm } => write!(f, "vmulhu.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VmulhuVx { vd, vs2, rs1, vm } => write!(f, "vmulhu.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VmulhsuVv { vd, vs2, vs1, vm } => write!(f, "vmulhsu.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VmulhsuVx { vd, vs2, rs1, vm } => write!(f, "vmulhsu.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            // Integer divide
            Self::VdivuVv { vd, vs2, vs1, vm } => write!(f, "vdivu.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VdivuVx { vd, vs2, rs1, vm } => write!(f, "vdivu.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VdivVv { vd, vs2, vs1, vm } => write!(f, "vdiv.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VdivVx { vd, vs2, rs1, vm } => write!(f, "vdiv.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VremuVv { vd, vs2, vs1, vm } => write!(f, "vremu.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VremuVx { vd, vs2, rs1, vm } => write!(f, "vremu.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VremVv { vd, vs2, vs1, vm } => write!(f, "vrem.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VremVx { vd, vs2, rs1, vm } => write!(f, "vrem.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            // Widening multiply
            Self::VwmulVv { vd, vs2, vs1, vm } => write!(f, "vwmul.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VwmulVx { vd, vs2, rs1, vm } => write!(f, "vwmul.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VwmuluVv { vd, vs2, vs1, vm } => write!(f, "vwmulu.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VwmuluVx { vd, vs2, rs1, vm } => write!(f, "vwmulu.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VwmulsuVv { vd, vs2, vs1, vm } => write!(f, "vwmulsu.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VwmulsuVx { vd, vs2, rs1, vm } => write!(f, "vwmulsu.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            // Single-width multiply-add
            Self::VmaccVv { vd, vs1, vs2, vm } => write!(f, "vmacc.vv {vd}, {vs1}, {vs2}{}", mask_suffix(vm)),
            Self::VmaccVx { vd, rs1, vs2, vm } => write!(f, "vmacc.vx {vd}, {rs1}, {vs2}{}", mask_suffix(vm)),
            Self::VnmsacVv { vd, vs1, vs2, vm } => write!(f, "vnmsac.vv {vd}, {vs1}, {vs2}{}", mask_suffix(vm)),
            Self::VnmsacVx { vd, rs1, vs2, vm } => write!(f, "vnmsac.vx {vd}, {rs1}, {vs2}{}", mask_suffix(vm)),
            Self::VmaddVv { vd, vs1, vs2, vm } => write!(f, "vmadd.vv {vd}, {vs1}, {vs2}{}", mask_suffix(vm)),
            Self::VmaddVx { vd, rs1, vs2, vm } => write!(f, "vmadd.vx {vd}, {rs1}, {vs2}{}", mask_suffix(vm)),
            Self::VnmsubVv { vd, vs1, vs2, vm } => write!(f, "vnmsub.vv {vd}, {vs1}, {vs2}{}", mask_suffix(vm)),
            Self::VnmsubVx { vd, rs1, vs2, vm } => write!(f, "vnmsub.vx {vd}, {rs1}, {vs2}{}", mask_suffix(vm)),
            // Widening multiply-add
            Self::VwmaccuVv { vd, vs1, vs2, vm } => write!(f, "vwmaccu.vv {vd}, {vs1}, {vs2}{}", mask_suffix(vm)),
            Self::VwmaccuVx { vd, rs1, vs2, vm } => write!(f, "vwmaccu.vx {vd}, {rs1}, {vs2}{}", mask_suffix(vm)),
            Self::VwmaccVv { vd, vs1, vs2, vm } => write!(f, "vwmacc.vv {vd}, {vs1}, {vs2}{}", mask_suffix(vm)),
            Self::VwmaccVx { vd, rs1, vs2, vm } => write!(f, "vwmacc.vx {vd}, {rs1}, {vs2}{}", mask_suffix(vm)),
            Self::VwmaccsuVv { vd, vs1, vs2, vm } => write!(f, "vwmaccsu.vv {vd}, {vs1}, {vs2}{}", mask_suffix(vm)),
            Self::VwmaccsuVx { vd, rs1, vs2, vm } => write!(f, "vwmaccsu.vx {vd}, {rs1}, {vs2}{}", mask_suffix(vm)),
            Self::VwmaccusVx { vd, rs1, vs2, vm } => write!(f, "vwmaccus.vx {vd}, {rs1}, {vs2}{}", mask_suffix(vm)),
        }
    }
}

/// Format mask suffix for display
#[inline(always)]
fn mask_suffix(vm: &bool) -> &'static str {
    if *vm { "" } else { ", v0.t" }
}
