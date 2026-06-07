//! ZveXx carry/borrow arithmetic instructions

#[cfg(test)]
mod tests;

use crate::instructions::Instruction;
use crate::registers::general_purpose::Register;
use crate::registers::vector::VReg;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V ZveXx carry/borrow arithmetic instruction.
///
/// All use the OP-V major opcode (0b101_0111) with OPIVV/OPIVX/OPIVI funct3.
/// The `vm` encoding bit selects carry/borrow-in source or signals no carry:
///
/// - vadc:  funct6=0b010000, vm=0 (carry-in from v0, always)
/// - vmadc: funct6=0b010001, vm=0 (carry-in from v0) or vm=1 (no carry-in)
/// - vsbc:  funct6=0b010010, vm=0 (borrow-in from v0, always)
/// - vmsbc: funct6=0b010011, vm=0 (borrow-in from v0) or vm=1 (no borrow-in)
///
/// vadc/vsbc produce SEW-wide data results written to vd.
/// vmadc/vmsbc produce one mask bit per element (carry-out/borrow-out) written to vd.
#[instruction]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[rustfmt::skip]
#[doc(hidden)]
pub enum ZveXxCarryInstruction<Reg> {
    // vadc: add with carry-in from v0, write SEW-wide result to vd (data register)

    /// `vadc.vvm vd, vs2, vs1, v0`
    VadcVvm { vd: VReg, vs2: VReg, vs1: VReg },
    /// `vadc.vxm vd, vs2, rs1, v0`
    VadcVxm { vd: VReg, vs2: VReg, rs1: Reg },
    /// `vadc.vim vd, vs2, imm, v0`
    VadcVim { vd: VReg, vs2: VReg, imm: i8 },

    // vmadc: add and produce carry-out mask in vd
    // vm=0: carry-in from v0;  vm=1: no carry-in (treat carry-in as zero)

    /// `vmadc.vvm vd, vs2, vs1, v0` - with carry-in
    VmadcVvm { vd: VReg, vs2: VReg, vs1: VReg },
    /// `vmadc.vxm vd, vs2, rs1, v0` - with carry-in
    VmadcVxm { vd: VReg, vs2: VReg, rs1: Reg },
    /// `vmadc.vim vd, vs2, imm, v0` - with carry-in
    VmadcVim { vd: VReg, vs2: VReg, imm: i8 },
    /// `vmadc.vv vd, vs2, vs1` - no carry-in
    VmadcVv  { vd: VReg, vs2: VReg, vs1: VReg },
    /// `vmadc.vx vd, vs2, rs1` - no carry-in
    VmadcVx  { vd: VReg, vs2: VReg, rs1: Reg },
    /// `vmadc.vi vd, vs2, imm` - no carry-in
    VmadcVi  { vd: VReg, vs2: VReg, imm: i8 },

    // vsbc: subtract with borrow-in from v0, write SEW-wide result to vd (data register)
    // No immediate form in the spec.

    /// `vsbc.vvm vd, vs2, vs1, v0`
    VsbcVvm  { vd: VReg, vs2: VReg, vs1: VReg },
    /// `vsbc.vxm vd, vs2, rs1, v0`
    VsbcVxm  { vd: VReg, vs2: VReg, rs1: Reg },

    // vmsbc: subtract and produce borrow-out mask in vd
    // vm=0: borrow-in from v0;  vm=1: no borrow-in

    /// `vmsbc.vvm vd, vs2, vs1, v0` - with borrow-in
    VmsbcVvm { vd: VReg, vs2: VReg, vs1: VReg },
    /// `vmsbc.vxm vd, vs2, rs1, v0` - with borrow-in
    VmsbcVxm { vd: VReg, vs2: VReg, rs1: Reg },
    /// `vmsbc.vv vd, vs2, vs1` - no borrow-in
    VmsbcVv  { vd: VReg, vs2: VReg, vs1: VReg },
    /// `vmsbc.vx vd, vs2, rs1` - no borrow-in
    VmsbcVx  { vd: VReg, vs2: VReg, rs1: Reg },
}

#[instruction]
impl<Reg> const Instruction for ZveXxCarryInstruction<Reg>
where
    Reg: [const] Register,
{
    type Reg = Reg;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        let opcode = (instruction & 0b111_1111) as u8;

        if opcode != 0b101_0111 {
            None?;
        }

        let vd_bits = ((instruction >> 7) & 0x1f) as u8;
        let funct3 = ((instruction >> 12) & 0b111) as u8;
        let vs1_bits = ((instruction >> 15) & 0x1f) as u8;
        let vs2_bits = ((instruction >> 20) & 0x1f) as u8;
        let vm = ((instruction >> 25) & 1) as u8;
        let funct6 = ((instruction >> 26) & 0b11_1111) as u8;

        let vd = VReg::from_bits(vd_bits)?;
        let vs2 = VReg::from_bits(vs2_bits)?;

        match funct3 {
            // OPIVV
            0b000 => {
                let vs1 = VReg::from_bits(vs1_bits)?;
                match (funct6, vm) {
                    (0b01_0000, 0) => Some(Self::VadcVvm { vd, vs2, vs1 }),
                    (0b01_0001, 0) => Some(Self::VmadcVvm { vd, vs2, vs1 }),
                    (0b01_0001, 1) => Some(Self::VmadcVv { vd, vs2, vs1 }),
                    (0b01_0010, 0) => Some(Self::VsbcVvm { vd, vs2, vs1 }),
                    (0b01_0011, 0) => Some(Self::VmsbcVvm { vd, vs2, vs1 }),
                    (0b01_0011, 1) => Some(Self::VmsbcVv { vd, vs2, vs1 }),
                    _ => None,
                }
            }
            // OPIVX
            0b100 => {
                let rs1 = Reg::from_bits(vs1_bits)?;
                match (funct6, vm) {
                    (0b01_0000, 0) => Some(Self::VadcVxm { vd, vs2, rs1 }),
                    (0b01_0001, 0) => Some(Self::VmadcVxm { vd, vs2, rs1 }),
                    (0b01_0001, 1) => Some(Self::VmadcVx { vd, vs2, rs1 }),
                    (0b01_0010, 0) => Some(Self::VsbcVxm { vd, vs2, rs1 }),
                    (0b01_0011, 0) => Some(Self::VmsbcVxm { vd, vs2, rs1 }),
                    (0b01_0011, 1) => Some(Self::VmsbcVx { vd, vs2, rs1 }),
                    _ => None,
                }
            }
            // OPIVI - only vadc and vmadc have immediate forms; vsbc/vmsbc do not
            0b011 => {
                let imm = (vs1_bits << 3).cast_signed() >> 3u8;
                match (funct6, vm) {
                    (0b01_0000, 0) => Some(Self::VadcVim { vd, vs2, imm }),
                    (0b01_0001, 0) => Some(Self::VmadcVim { vd, vs2, imm }),
                    (0b01_0001, 1) => Some(Self::VmadcVi { vd, vs2, imm }),
                    _ => None,
                }
            }
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
impl<Reg> fmt::Display for ZveXxCarryInstruction<Reg>
where
    Reg: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[rustfmt::skip]
        match self {
            Self::VadcVvm  { vd, vs2, vs1 } => write!(f, "vadc.vvm {vd}, {vs2}, {vs1}, v0"),
            Self::VadcVxm  { vd, vs2, rs1 } => write!(f, "vadc.vxm {vd}, {vs2}, {rs1}, v0"),
            Self::VadcVim  { vd, vs2, imm } => write!(f, "vadc.vim {vd}, {vs2}, {imm}, v0"),
            Self::VmadcVvm { vd, vs2, vs1 } => write!(f, "vmadc.vvm {vd}, {vs2}, {vs1}, v0"),
            Self::VmadcVxm { vd, vs2, rs1 } => write!(f, "vmadc.vxm {vd}, {vs2}, {rs1}, v0"),
            Self::VmadcVim { vd, vs2, imm } => write!(f, "vmadc.vim {vd}, {vs2}, {imm}, v0"),
            Self::VmadcVv  { vd, vs2, vs1 } => write!(f, "vmadc.vv {vd}, {vs2}, {vs1}"),
            Self::VmadcVx  { vd, vs2, rs1 } => write!(f, "vmadc.vx {vd}, {vs2}, {rs1}"),
            Self::VmadcVi  { vd, vs2, imm } => write!(f, "vmadc.vi {vd}, {vs2}, {imm}"),
            Self::VsbcVvm  { vd, vs2, vs1 } => write!(f, "vsbc.vvm {vd}, {vs2}, {vs1}, v0"),
            Self::VsbcVxm  { vd, vs2, rs1 } => write!(f, "vsbc.vxm {vd}, {vs2}, {rs1}, v0"),
            Self::VmsbcVvm { vd, vs2, vs1 } => write!(f, "vmsbc.vvm {vd}, {vs2}, {vs1}, v0"),
            Self::VmsbcVxm { vd, vs2, rs1 } => write!(f, "vmsbc.vxm {vd}, {vs2}, {rs1}, v0"),
            Self::VmsbcVv  { vd, vs2, vs1 } => write!(f, "vmsbc.vv {vd}, {vs2}, {vs1}"),
            Self::VmsbcVx  { vd, vs2, rs1 } => write!(f, "vmsbc.vx {vd}, {vs2}, {rs1}"),
        }
    }
}
