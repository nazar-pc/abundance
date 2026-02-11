//! RV64 Zve64x extension (Vector Extension for Embedded Processors, ELEN=64, integer-only)

mod arith;
mod config;
mod fixed_point;
mod load;
mod mask;
mod muldiv;
mod perm;
mod reduction;
mod store;
mod widen_narrow;

use crate::instruction::Instruction;
use crate::instruction::rv64::v::zve64x::arith::Rv64Zve64xArithInstruction;
use crate::instruction::rv64::v::zve64x::config::Rv64Zve64xConfigInstruction;
use crate::instruction::rv64::v::zve64x::fixed_point::Rv64Zve64xFixedPointInstruction;
use crate::instruction::rv64::v::zve64x::load::Rv64Zve64xLoadInstruction;
use crate::instruction::rv64::v::zve64x::mask::Rv64Zve64xMaskInstruction;
use crate::instruction::rv64::v::zve64x::muldiv::Rv64Zve64xMulDivInstruction;
use crate::instruction::rv64::v::zve64x::perm::Rv64Zve64xPermInstruction;
use crate::instruction::rv64::v::zve64x::reduction::Rv64Zve64xReductionInstruction;
use crate::instruction::rv64::v::zve64x::store::Rv64Zve64xStoreInstruction;
use crate::instruction::rv64::v::zve64x::widen_narrow::Rv64Zve64xWidenNarrowInstruction;
use crate::registers::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V vector register (v0-v31)
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum VReg {
    /// Vector register v0 (also used as mask register)
    V0 = 0,
    /// Vector register v1
    V1 = 1,
    /// Vector register v2
    V2 = 2,
    /// Vector register v3
    V3 = 3,
    /// Vector register v4
    V4 = 4,
    /// Vector register v5
    V5 = 5,
    /// Vector register v6
    V6 = 6,
    /// Vector register v7
    V7 = 7,
    /// Vector register v8
    V8 = 8,
    /// Vector register v9
    V9 = 9,
    /// Vector register v10
    V10 = 10,
    /// Vector register v11
    V11 = 11,
    /// Vector register v12
    V12 = 12,
    /// Vector register v13
    V13 = 13,
    /// Vector register v14
    V14 = 14,
    /// Vector register v15
    V15 = 15,
    /// Vector register v16
    V16 = 16,
    /// Vector register v17
    V17 = 17,
    /// Vector register v18
    V18 = 18,
    /// Vector register v19
    V19 = 19,
    /// Vector register v20
    V20 = 20,
    /// Vector register v21
    V21 = 21,
    /// Vector register v22
    V22 = 22,
    /// Vector register v23
    V23 = 23,
    /// Vector register v24
    V24 = 24,
    /// Vector register v25
    V25 = 25,
    /// Vector register v26
    V26 = 26,
    /// Vector register v27
    V27 = 27,
    /// Vector register v28
    V28 = 28,
    /// Vector register v29
    V29 = 29,
    /// Vector register v30
    V30 = 30,
    /// Vector register v31
    V31 = 31,
}

impl VReg {
    /// Create a vector register from its 5-bit encoding
    #[inline(always)]
    pub const fn from_bits(bits: u8) -> Option<Self> {
        match bits {
            0 => Some(Self::V0),
            1 => Some(Self::V1),
            2 => Some(Self::V2),
            3 => Some(Self::V3),
            4 => Some(Self::V4),
            5 => Some(Self::V5),
            6 => Some(Self::V6),
            7 => Some(Self::V7),
            8 => Some(Self::V8),
            9 => Some(Self::V9),
            10 => Some(Self::V10),
            11 => Some(Self::V11),
            12 => Some(Self::V12),
            13 => Some(Self::V13),
            14 => Some(Self::V14),
            15 => Some(Self::V15),
            16 => Some(Self::V16),
            17 => Some(Self::V17),
            18 => Some(Self::V18),
            19 => Some(Self::V19),
            20 => Some(Self::V20),
            21 => Some(Self::V21),
            22 => Some(Self::V22),
            23 => Some(Self::V23),
            24 => Some(Self::V24),
            25 => Some(Self::V25),
            26 => Some(Self::V26),
            27 => Some(Self::V27),
            28 => Some(Self::V28),
            29 => Some(Self::V29),
            30 => Some(Self::V30),
            31 => Some(Self::V31),
            _ => None,
        }
    }

    /// Return the 5-bit encoding of this register
    #[inline(always)]
    pub const fn bits(self) -> u8 {
        self as u8
    }
}

impl fmt::Display for VReg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{}", *self as u8)
    }
}

impl fmt::Debug for VReg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// Element width for vector memory operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Eew {
    /// 8-bit elements
    E8 = 0b000,
    /// 16-bit elements
    E16 = 0b101,
    /// 32-bit elements
    E32 = 0b110,
    /// 64-bit elements
    E64 = 0b111,
}

impl Eew {
    /// Decode the width field (bits[14:12]) into an element width
    #[inline(always)]
    const fn from_width(width: u8) -> Option<Self> {
        match width {
            0b000 => Some(Self::E8),
            0b101 => Some(Self::E16),
            0b110 => Some(Self::E32),
            0b111 => Some(Self::E64),
            _ => None,
        }
    }

    /// Return the number of bits
    #[inline(always)]
    pub const fn bits(self) -> u16 {
        match self {
            Self::E8 => 8,
            Self::E16 => 16,
            Self::E32 => 32,
            Self::E64 => 64,
        }
    }
}

impl fmt::Display for Eew {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.bits().fmt(f)
    }
}

/// RISC-V RV64 Zve64x instruction
#[instruction(
    ignore = [Phantom],
    inherit = [
        Rv64Zve64xConfigInstruction,
        Rv64Zve64xLoadInstruction,
        Rv64Zve64xStoreInstruction,
        Rv64Zve64xArithInstruction,
        Rv64Zve64xMulDivInstruction,
        Rv64Zve64xWidenNarrowInstruction,
        Rv64Zve64xFixedPointInstruction,
        Rv64Zve64xMaskInstruction,
        Rv64Zve64xReductionInstruction,
        Rv64Zve64xPermInstruction,
    ],
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rv64Zve64xInstruction<Reg> {}

#[instruction]
impl<Reg> const Instruction for Rv64Zve64xInstruction<Reg>
where
    Reg: [const] Register<Type = u64>,
{
    type Reg = Reg;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        None
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

#[instruction]
impl<Reg> fmt::Display for Rv64Zve64xInstruction<Reg>
where
    Reg: fmt::Display + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}
