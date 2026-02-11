//! RV64 Zve64x fixed-point arithmetic instructions

#[cfg(test)]
mod tests;

use crate::instruction::Instruction;
use crate::instruction::rv64::v::zve64x::VReg;
use crate::registers::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV64 Zve64x fixed-point arithmetic instruction.
///
/// Includes saturating add/subtract, averaging add/subtract, fractional multiply, scaling shifts,
/// and narrowing clips. All use the OP-V major opcode (0b1010111).
#[instruction]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[rustfmt::skip]
pub(super) enum Rv64Zve64xFixedPointInstruction<Reg> {
    /// `vsaddu.vv vd, vs2, vs1, vm` - Saturating unsigned add, vector-vector
    VsadduVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vsaddu.vx vd, vs2, rs1, vm` - Saturating unsigned add, vector-scalar
    VsadduVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vsaddu.vi vd, vs2, imm, vm` - Saturating unsigned add, vector-immediate
    VsadduVi { vd: VReg, vs2: VReg, imm: i8, vm: bool },
    /// `vsadd.vv vd, vs2, vs1, vm` - Saturating signed add, vector-vector
    VsaddVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vsadd.vx vd, vs2, rs1, vm` - Saturating signed add, vector-scalar
    VsaddVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vsadd.vi vd, vs2, imm, vm` - Saturating signed add, vector-immediate
    VsaddVi { vd: VReg, vs2: VReg, imm: i8, vm: bool },
    /// `vssubu.vv vd, vs2, vs1, vm` - Saturating unsigned subtract, vector-vector
    VssubuVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vssubu.vx vd, vs2, rs1, vm` - Saturating unsigned subtract, vector-scalar
    VssubuVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vssub.vv vd, vs2, vs1, vm` - Saturating signed subtract, vector-vector
    VssubVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vssub.vx vd, vs2, rs1, vm` - Saturating signed subtract, vector-scalar
    VssubVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },

    /// `vaaddu.vv vd, vs2, vs1, vm` - Averaging unsigned add, vector-vector
    VaadduVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vaaddu.vx vd, vs2, rs1, vm` - Averaging unsigned add, vector-scalar
    VaadduVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vaadd.vv vd, vs2, vs1, vm` - Averaging signed add, vector-vector
    VaaddVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vaadd.vx vd, vs2, rs1, vm` - Averaging signed add, vector-scalar
    VaaddVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vasubu.vv vd, vs2, vs1, vm` - Averaging unsigned subtract, vector-vector
    VasubuVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vasubu.vx vd, vs2, rs1, vm` - Averaging unsigned subtract, vector-scalar
    VasubuVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vasub.vv vd, vs2, vs1, vm` - Averaging signed subtract, vector-vector
    VasubVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vasub.vx vd, vs2, rs1, vm` - Averaging signed subtract, vector-scalar
    VasubVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },

    /// `vsmul.vv vd, vs2, vs1, vm` - Fractional multiply with rounding and saturation
    VsmulVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vsmul.vx vd, vs2, rs1, vm` - Fractional multiply with rounding and saturation
    VsmulVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },

    /// `vssrl.vv vd, vs2, vs1, vm` - Scaling shift right logical, vector-vector
    VssrlVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vssrl.vx vd, vs2, rs1, vm` - Scaling shift right logical, vector-scalar
    VssrlVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vssrl.vi vd, vs2, imm, vm` - Scaling shift right logical, vector-immediate
    VssrlVi { vd: VReg, vs2: VReg, imm: u8, vm: bool },
    /// `vssra.vv vd, vs2, vs1, vm` - Scaling shift right arithmetic, vector-vector
    VssraVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vssra.vx vd, vs2, rs1, vm` - Scaling shift right arithmetic, vector-scalar
    VssraVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vssra.vi vd, vs2, imm, vm` - Scaling shift right arithmetic, vector-immediate
    VssraVi { vd: VReg, vs2: VReg, imm: u8, vm: bool },

    /// `vnclipu.wv vd, vs2, vs1, vm` - Narrowing unsigned clip, vector-vector
    VnclipuWv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vnclipu.wx vd, vs2, rs1, vm` - Narrowing unsigned clip, vector-scalar
    VnclipuWx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vnclipu.wi vd, vs2, imm, vm` - Narrowing unsigned clip, vector-immediate
    VnclipuWi { vd: VReg, vs2: VReg, imm: u8, vm: bool },
    /// `vnclip.wv vd, vs2, vs1, vm` - Narrowing signed clip, vector-vector
    VnclipWv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vnclip.wx vd, vs2, rs1, vm` - Narrowing signed clip, vector-scalar
    VnclipWx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vnclip.wi vd, vs2, imm, vm` - Narrowing signed clip, vector-immediate
    VnclipWi { vd: VReg, vs2: VReg, imm: u8, vm: bool },
}

#[instruction]
impl<Reg> const Instruction for Rv64Zve64xFixedPointInstruction<Reg>
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

        match funct6 {
            // Saturating add/sub - OPIVV / OPIVX / OPIVI
            // vsaddu: funct6=100000
            0b100000 => match funct3 {
                // OPIVV
                0b000 => {
                    let vs1 = VReg::from_bits(vs1_bits)?;
                    Some(Self::VsadduVv { vd, vs2, vs1, vm })
                }
                // OPIVX
                0b100 => {
                    let rs1 = Reg::from_bits(vs1_bits)?;
                    Some(Self::VsadduVx { vd, vs2, rs1, vm })
                }
                // OPIVI
                0b011 => {
                    let imm = vs1_bits.cast_signed() << 3 >> 3;
                    Some(Self::VsadduVi { vd, vs2, imm, vm })
                }
                _ => None,
            },
            // vsadd: funct6=100001
            0b100001 => match funct3 {
                // OPIVV
                0b000 => {
                    let vs1 = VReg::from_bits(vs1_bits)?;
                    Some(Self::VsaddVv { vd, vs2, vs1, vm })
                }
                // OPIVX
                0b100 => {
                    let rs1 = Reg::from_bits(vs1_bits)?;
                    Some(Self::VsaddVx { vd, vs2, rs1, vm })
                }
                // OPIVI
                0b011 => {
                    let imm = vs1_bits.cast_signed() << 3 >> 3;
                    Some(Self::VsaddVi { vd, vs2, imm, vm })
                }
                _ => None,
            },
            // vssubu: funct6=100010 (VV/VX only)
            0b100010 => match funct3 {
                // OPIVV
                0b000 => {
                    let vs1 = VReg::from_bits(vs1_bits)?;
                    Some(Self::VssubuVv { vd, vs2, vs1, vm })
                }
                // OPIVX
                0b100 => {
                    let rs1 = Reg::from_bits(vs1_bits)?;
                    Some(Self::VssubuVx { vd, vs2, rs1, vm })
                }
                _ => None,
            },
            // vssub: funct6=100011 (VV/VX only)
            0b100011 => match funct3 {
                // OPIVV
                0b000 => {
                    let vs1 = VReg::from_bits(vs1_bits)?;
                    Some(Self::VssubVv { vd, vs2, vs1, vm })
                }
                // OPIVX
                0b100 => {
                    let rs1 = Reg::from_bits(vs1_bits)?;
                    Some(Self::VssubVx { vd, vs2, rs1, vm })
                }
                _ => None,
            },

            // Averaging add/sub - OPMVV / OPMVX
            // vaaddu: funct6=001000
            0b001000 => match funct3 {
                // OPMVV
                0b010 => {
                    let vs1 = VReg::from_bits(vs1_bits)?;
                    Some(Self::VaadduVv { vd, vs2, vs1, vm })
                }
                // OPMVX
                0b110 => {
                    let rs1 = Reg::from_bits(vs1_bits)?;
                    Some(Self::VaadduVx { vd, vs2, rs1, vm })
                }
                _ => None,
            },
            // vaadd: funct6=001001
            0b001001 => match funct3 {
                // OPMVV
                0b010 => {
                    let vs1 = VReg::from_bits(vs1_bits)?;
                    Some(Self::VaaddVv { vd, vs2, vs1, vm })
                }
                // OPMVX
                0b110 => {
                    let rs1 = Reg::from_bits(vs1_bits)?;
                    Some(Self::VaaddVx { vd, vs2, rs1, vm })
                }
                _ => None,
            },
            // vasubu: funct6=001010
            0b001010 => match funct3 {
                // OPMVV
                0b010 => {
                    let vs1 = VReg::from_bits(vs1_bits)?;
                    Some(Self::VasubuVv { vd, vs2, vs1, vm })
                }
                // OPMVX
                0b110 => {
                    let rs1 = Reg::from_bits(vs1_bits)?;
                    Some(Self::VasubuVx { vd, vs2, rs1, vm })
                }
                _ => None,
            },
            // vasub: funct6=001011
            0b001011 => match funct3 {
                // OPMVV
                0b010 => {
                    let vs1 = VReg::from_bits(vs1_bits)?;
                    Some(Self::VasubVv { vd, vs2, vs1, vm })
                }
                // OPMVX
                0b110 => {
                    let rs1 = Reg::from_bits(vs1_bits)?;
                    Some(Self::VasubVx { vd, vs2, rs1, vm })
                }
                _ => None,
            },

            // Fractional multiply - OPMVV / OPMVX
            // vsmul: funct6=100111
            0b100111 => match funct3 {
                // OPMVV
                0b010 => {
                    let vs1 = VReg::from_bits(vs1_bits)?;
                    Some(Self::VsmulVv { vd, vs2, vs1, vm })
                }
                // OPMVX
                0b110 => {
                    let rs1 = Reg::from_bits(vs1_bits)?;
                    Some(Self::VsmulVx { vd, vs2, rs1, vm })
                }
                _ => None,
            },

            // Scaling shifts - OPIVV / OPIVX / OPIVI
            // vssrl: funct6=101000
            0b101000 => match funct3 {
                // OPIVV
                0b000 => {
                    let vs1 = VReg::from_bits(vs1_bits)?;
                    Some(Self::VssrlVv { vd, vs2, vs1, vm })
                }
                // OPIVX
                0b100 => {
                    let rs1 = Reg::from_bits(vs1_bits)?;
                    Some(Self::VssrlVx { vd, vs2, rs1, vm })
                }
                // OPIVI
                0b011 => {
                    let imm = vs1_bits;
                    Some(Self::VssrlVi { vd, vs2, imm, vm })
                }
                _ => None,
            },
            // vssra: funct6=101001
            0b101001 => match funct3 {
                // OPIVV
                0b000 => {
                    let vs1 = VReg::from_bits(vs1_bits)?;
                    Some(Self::VssraVv { vd, vs2, vs1, vm })
                }
                // OPIVX
                0b100 => {
                    let rs1 = Reg::from_bits(vs1_bits)?;
                    Some(Self::VssraVx { vd, vs2, rs1, vm })
                }
                // OPIVI
                0b011 => {
                    let imm = vs1_bits;
                    Some(Self::VssraVi { vd, vs2, imm, vm })
                }
                _ => None,
            },

            // Narrowing clips - OPIVV / OPIVX / OPIVI
            // vnclipu: funct6=101110
            0b101110 => match funct3 {
                // OPIVV
                0b000 => {
                    let vs1 = VReg::from_bits(vs1_bits)?;
                    Some(Self::VnclipuWv { vd, vs2, vs1, vm })
                }
                // OPIVX
                0b100 => {
                    let rs1 = Reg::from_bits(vs1_bits)?;
                    Some(Self::VnclipuWx { vd, vs2, rs1, vm })
                }
                // OPIVI
                0b011 => {
                    let imm = vs1_bits;
                    Some(Self::VnclipuWi { vd, vs2, imm, vm })
                }
                _ => None,
            },
            // vnclip: funct6=101111
            0b101111 => match funct3 {
                // OPIVV
                0b000 => {
                    let vs1 = VReg::from_bits(vs1_bits)?;
                    Some(Self::VnclipWv { vd, vs2, vs1, vm })
                }
                // OPIVX
                0b100 => {
                    let rs1 = Reg::from_bits(vs1_bits)?;
                    Some(Self::VnclipWx { vd, vs2, rs1, vm })
                }
                // OPIVI
                0b011 => {
                    let imm = vs1_bits;
                    Some(Self::VnclipWi { vd, vs2, imm, vm })
                }
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

impl<Reg> fmt::Display for Rv64Zve64xFixedPointInstruction<Reg>
where
    Reg: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[rustfmt::skip]
        match self {
            Self::VsadduVv { vd, vs2, vs1, vm } => write!(f, "vsaddu.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VsadduVx { vd, vs2, rs1, vm } => write!(f, "vsaddu.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VsadduVi { vd, vs2, imm, vm } => write!(f, "vsaddu.vi {vd}, {vs2}, {imm}{}", mask_suffix(vm)),
            Self::VsaddVv { vd, vs2, vs1, vm } => write!(f, "vsadd.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VsaddVx { vd, vs2, rs1, vm } => write!(f, "vsadd.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VsaddVi { vd, vs2, imm, vm } => write!(f, "vsadd.vi {vd}, {vs2}, {imm}{}", mask_suffix(vm)),
            Self::VssubuVv { vd, vs2, vs1, vm } => write!(f, "vssubu.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VssubuVx { vd, vs2, rs1, vm } => write!(f, "vssubu.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VssubVv { vd, vs2, vs1, vm } => write!(f, "vssub.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VssubVx { vd, vs2, rs1, vm } => write!(f, "vssub.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VaadduVv { vd, vs2, vs1, vm } => write!(f, "vaaddu.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VaadduVx { vd, vs2, rs1, vm } => write!(f, "vaaddu.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VaaddVv { vd, vs2, vs1, vm } => write!(f, "vaadd.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VaaddVx { vd, vs2, rs1, vm } => write!(f, "vaadd.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VasubuVv { vd, vs2, vs1, vm } => write!(f, "vasubu.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VasubuVx { vd, vs2, rs1, vm } => write!(f, "vasubu.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VasubVv { vd, vs2, vs1, vm } => write!(f, "vasub.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VasubVx { vd, vs2, rs1, vm } => write!(f, "vasub.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VsmulVv { vd, vs2, vs1, vm } => write!(f, "vsmul.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VsmulVx { vd, vs2, rs1, vm } => write!(f, "vsmul.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VssrlVv { vd, vs2, vs1, vm } => write!(f, "vssrl.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VssrlVx { vd, vs2, rs1, vm } => write!(f, "vssrl.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VssrlVi { vd, vs2, imm, vm } => write!(f, "vssrl.vi {vd}, {vs2}, {imm}{}", mask_suffix(vm)),
            Self::VssraVv { vd, vs2, vs1, vm } => write!(f, "vssra.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VssraVx { vd, vs2, rs1, vm } => write!(f, "vssra.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VssraVi { vd, vs2, imm, vm } => write!(f, "vssra.vi {vd}, {vs2}, {imm}{}", mask_suffix(vm)),
            Self::VnclipuWv { vd, vs2, vs1, vm } => write!(f, "vnclipu.wv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VnclipuWx { vd, vs2, rs1, vm } => write!(f, "vnclipu.wx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VnclipuWi { vd, vs2, imm, vm } => write!(f, "vnclipu.wi {vd}, {vs2}, {imm}{}", mask_suffix(vm)),
            Self::VnclipWv { vd, vs2, vs1, vm } => write!(f, "vnclip.wv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VnclipWx { vd, vs2, rs1, vm } => write!(f, "vnclip.wx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VnclipWi { vd, vs2, imm, vm } => write!(f, "vnclip.wi {vd}, {vs2}, {imm}{}", mask_suffix(vm)),
        }
    }
}

/// Format mask suffix for display
#[inline(always)]
fn mask_suffix(vm: &bool) -> &'static str {
    if *vm { "" } else { ", v0.t" }
}
