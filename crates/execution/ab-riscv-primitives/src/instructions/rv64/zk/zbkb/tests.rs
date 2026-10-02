#![expect(clippy::unusual_byte_groupings, reason = "Test readability")]

use crate::instructions::Instruction;
use crate::instructions::rv64::b::zbb::{Rv64ZbbInstruction, Rv64ZbbZbkbSharedInstruction};
use crate::instructions::rv64::zk::zbkb::Rv64ZbkbInstruction;
use crate::instructions::test_utils::{make_i_type, make_r_type};
use crate::registers::general_purpose::{Reg, Register};
use ab_riscv_macros::instruction;
use core::fmt;

#[instruction(inherit = [Rv64ZbbInstruction, Rv64ZbkbInstruction])]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
enum Rv64ZbbZbkbTestInstruction<Reg> {}

#[instruction]
const impl<Reg> Instruction for Rv64ZbbZbkbTestInstruction<Reg>
where
    Reg: [const] Register<Type = u64>,
{
    const ALIGNMENT: u8 = align_of::<u32>() as u8;

    type Reg = Reg;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        None
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

#[instruction]
impl<Reg> fmt::Display for Rv64ZbbZbkbTestInstruction<Reg>
where
    Reg: fmt::Display + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[instruction(inherit = [Rv64ZbkbInstruction, Rv64ZbbInstruction])]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
enum Rv64ZbkbZbbTestInstruction<Reg> {}

#[instruction]
const impl<Reg> Instruction for Rv64ZbkbZbbTestInstruction<Reg>
where
    Reg: [const] Register<Type = u64>,
{
    const ALIGNMENT: u8 = align_of::<u32>() as u8;

    type Reg = Reg;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        None
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

#[instruction]
impl<Reg> fmt::Display for Rv64ZbkbZbbTestInstruction<Reg>
where
    Reg: fmt::Display + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[test]
fn test_pack() {
    // pack: opcode=0b011_0011, funct3=0b100, funct7=0b000_0100
    let inst = make_r_type(0b011_0011, 1, 0b100, 2, 3, 0b000_0100);
    let decoded = Rv64ZbkbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbkbInstruction::Pack {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_packh() {
    // packh: opcode=0b011_0011, funct3=0b111, funct7=0b000_0100
    let inst = make_r_type(0b011_0011, 1, 0b111, 2, 3, 0b000_0100);
    let decoded = Rv64ZbkbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbkbInstruction::Packh {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_packw() {
    // packw: opcode=0b011_1011, funct3=0b100, funct7=0b000_0100
    let inst = make_r_type(0b011_1011, 1, 0b100, 2, 3, 0b000_0100);
    let decoded = Rv64ZbkbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbkbInstruction::Packw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_packw_rs2_zero() {
    // `packw rd, rs1, x0` has the same encoding as Zbb's `zext.h`, but without Zbb it is decoded as
    // `packw`
    let inst = make_r_type(0b011_1011, 1, 0b100, 2, 0, 0b000_0100);
    assert_eq!(
        Rv64ZbkbInstruction::<Reg<u64>>::try_decode(inst),
        Some(Rv64ZbkbInstruction::Packw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_packw_rs2_zero_with_zbb() {
    // With Zbb present, `zext.h` is decoded instead regardless of the inheritance order
    let inst = make_r_type(0b011_1011, 1, 0b100, 2, 0, 0b000_0100);
    assert_eq!(
        Rv64ZbbZbkbTestInstruction::<Reg<u64>>::try_decode(inst),
        Some(Rv64ZbbZbkbTestInstruction::Zexth {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
    assert_eq!(
        Rv64ZbkbZbbTestInstruction::<Reg<u64>>::try_decode(inst),
        Some(Rv64ZbkbZbbTestInstruction::Zexth {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
    // Non-zero `rs2` is still `packw`
    let inst = make_r_type(0b011_1011, 1, 0b100, 2, 3, 0b000_0100);
    assert_eq!(
        Rv64ZbbZbkbTestInstruction::<Reg<u64>>::try_decode(inst),
        Some(Rv64ZbbZbkbTestInstruction::Packw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
        })
    );
}

#[test]
fn test_brev8() {
    // brev8: OP-IMM, funct3=101, funct12=0b011010000111 = 0x687
    let inst = 0b011010000111_00010_101_00001_0010011u32;
    let decoded = Rv64ZbkbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbkbInstruction::Brev8 {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_pack_wrong_funct7_returns_none() {
    // funct7=0b000_0000 with pack's funct3=100 is just XOR; should not decode as pack
    let inst = make_r_type(0b011_0011, 1, 0b100, 2, 3, 0b000_0000);
    let decoded = Rv64ZbkbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_unknown_opcode_returns_none() {
    let inst = make_r_type(0b010_0011, 1, 0b100, 2, 3, 0b000_0100);
    let decoded = Rv64ZbkbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_shared_with_zbb() {
    let andn = make_r_type(0b011_0011, 1, 0b111, 2, 3, 0b010_0000);
    assert_eq!(
        Rv64ZbkbInstruction::<Reg<u64>>::try_decode(andn),
        Some(Rv64ZbkbInstruction::Andn {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
        })
    );
    let rev8 = make_i_type(0b001_0011, 1, 0b101, 2, 0b0110_1011_1000);
    assert_eq!(
        Rv64ZbkbInstruction::<Reg<u64>>::try_decode(rev8),
        Some(Rv64ZbkbInstruction::Rev8 {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_zbb_only_rejected() {
    let clz = make_i_type(0b001_0011, 1, 0b001, 2, 0b0110_0000_0000);
    let orc_b = make_i_type(0b001_0011, 1, 0b101, 2, 0b0010_1000_0111);
    let min = make_r_type(0b011_0011, 1, 0b100, 2, 3, 0b000_0101);
    for inst in [clz, orc_b, min] {
        assert_eq!(Rv64ZbkbInstruction::<Reg<u64>>::try_decode(inst), None);
        assert!(Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst).is_some());
    }
}
