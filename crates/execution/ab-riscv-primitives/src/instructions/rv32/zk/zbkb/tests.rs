#![expect(clippy::unusual_byte_groupings, reason = "Test readability")]

use crate::hart::{BasicHart, HartConfig};
use crate::instructions::rv32::b::zbb::{Rv32ZbbInstruction, Rv32ZbbZbkbSharedInstruction};
use crate::instructions::rv32::zk::zbkb::Rv32ZbkbInstruction;
use crate::instructions::test_utils::{make_i_type, make_r_type};
use crate::instructions::{ImplementedExtension, Instruction, IsaExtension};
use crate::registers::general_purpose::{Reg, Register};
use ab_riscv_macros::instruction;
use core::fmt;

#[instruction(inherit = [Rv32ZbbInstruction, Rv32ZbkbInstruction])]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
enum Rv32ZbbZbkbTestInstruction<Hart>
where
    Hart: HartConfig, {}

#[instruction]
const impl<Reg, Hart> Instruction for Rv32ZbbZbkbTestInstruction<Hart>
where
    Reg: [const] Register<Type = u32>,
    Hart: [const] HartConfig<Reg = Reg>,
{
    const OWN_ISA_EXTENSIONS: &'static [IsaExtension] = &[];

    const ALIGNMENT: u8 = align_of::<u32>() as u8;

    type Hart = Hart;

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
impl<Reg, Hart> fmt::Display for Rv32ZbbZbkbTestInstruction<Hart>
where
    Reg: fmt::Display + Copy,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[instruction(inherit = [Rv32ZbkbInstruction, Rv32ZbbInstruction])]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
enum Rv32ZbkbZbbTestInstruction<Hart>
where
    Hart: HartConfig, {}

#[instruction]
const impl<Reg, Hart> Instruction for Rv32ZbkbZbbTestInstruction<Hart>
where
    Reg: [const] Register<Type = u32>,
    Hart: [const] HartConfig<Reg = Reg>,
{
    const OWN_ISA_EXTENSIONS: &'static [IsaExtension] = &[];

    const ALIGNMENT: u8 = align_of::<u32>() as u8;

    type Hart = Hart;

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
impl<Reg, Hart> fmt::Display for Rv32ZbkbZbbTestInstruction<Hart>
where
    Reg: fmt::Display + Copy,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[instruction(
    ignore = [Zexth],
    inherit = [Rv32ZbbInstruction, Rv32ZbkbInstruction],
)]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
enum Rv32ZbbZbkbWithoutZexthTestInstruction<Hart>
where
    Hart: HartConfig, {}

#[instruction]
const impl<Reg, Hart> Instruction for Rv32ZbbZbkbWithoutZexthTestInstruction<Hart>
where
    Reg: [const] Register<Type = u32>,
    Hart: [const] HartConfig<Reg = Reg>,
{
    const OWN_ISA_EXTENSIONS: &'static [IsaExtension] = &[];

    const ALIGNMENT: u8 = align_of::<u32>() as u8;

    type Hart = Hart;

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
impl<Reg, Hart> fmt::Display for Rv32ZbbZbkbWithoutZexthTestInstruction<Hart>
where
    Reg: fmt::Display + Copy,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[test]
fn test_pack() {
    // pack: opcode=0b011_0011, funct3=0b100, funct7=0b000_0100
    let inst = make_r_type(0b011_0011, 1, 0b100, 2, 3, 0b000_0100);
    let decoded = Rv32ZbkbInstruction::<BasicHart<Reg<u32>>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbkbInstruction::Pack {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_pack_rs2_zero() {
    // `pack rd, rs1, x0` has the same encoding as Zbb's `zext.h`, but without Zbb it is decoded as
    // `pack`
    let inst = make_r_type(0b011_0011, 1, 0b100, 2, 0, 0b000_0100);
    assert_eq!(
        Rv32ZbkbInstruction::<BasicHart<Reg<u32>>>::try_decode(inst),
        Some(Rv32ZbkbInstruction::Pack {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_pack_rs2_zero_with_zbb() {
    // With Zbb present, `zext.h` is decoded instead regardless of the inheritance order
    let inst = make_r_type(0b011_0011, 1, 0b100, 2, 0, 0b000_0100);
    assert_eq!(
        Rv32ZbbZbkbTestInstruction::<BasicHart<Reg<u32>>>::try_decode(inst),
        Some(Rv32ZbbZbkbTestInstruction::Zexth {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
    assert_eq!(
        Rv32ZbkbZbbTestInstruction::<BasicHart<Reg<u32>>>::try_decode(inst),
        Some(Rv32ZbkbZbbTestInstruction::Zexth {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
    // Non-zero `rs2` is still `pack`
    let inst = make_r_type(0b011_0011, 1, 0b100, 2, 3, 0b000_0100);
    assert_eq!(
        Rv32ZbbZbkbTestInstruction::<BasicHart<Reg<u32>>>::try_decode(inst),
        Some(Rv32ZbbZbkbTestInstruction::Pack {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
        })
    );
}

#[test]
fn test_pack_rs2_zero_with_zbb_without_zext_h() {
    // With `zext.h` ignored, `pack` is decoded instead even though Zbb is present
    let inst = make_r_type(0b011_0011, 1, 0b100, 2, 0, 0b000_0100);
    assert_eq!(
        Rv32ZbbZbkbWithoutZexthTestInstruction::<BasicHart<Reg<u32>>>::try_decode(inst),
        Some(Rv32ZbbZbkbWithoutZexthTestInstruction::Pack {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_packh() {
    // packh: opcode=0b011_0011, funct3=0b111, funct7=0b000_0100
    let inst = make_r_type(0b011_0011, 1, 0b111, 2, 3, 0b000_0100);
    let decoded = Rv32ZbkbInstruction::<BasicHart<Reg<u32>>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbkbInstruction::Packh {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_brev8() {
    // brev8: OP-IMM, funct3=101, funct12=0b011010000111 = 0x687
    let inst = 0b011010000111_00010_101_00001_0010011u32;
    let decoded = Rv32ZbkbInstruction::<BasicHart<Reg<u32>>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbkbInstruction::Brev8 {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_zip() {
    // zip: OP-IMM, funct3=001, funct7=000_0100, rs2=01111 (x15) -> funct12=0x08F
    let inst = 0b0000100_01111_00010_001_00001_0010011u32;
    let decoded = Rv32ZbkbInstruction::<BasicHart<Reg<u32>>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbkbInstruction::Zip {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_unzip() {
    // unzip: OP-IMM, funct3=101, funct7=000_0100, rs2=01111 (x15) -> funct12=0x08F
    let inst = 0b0000100_01111_00010_101_00001_0010011u32;
    let decoded = Rv32ZbkbInstruction::<BasicHart<Reg<u32>>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbkbInstruction::Unzip {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_zip_unzip_are_inverses_encoding() {
    // zip (funct3=001) and unzip (funct3=101) share funct7=000_0100 and rs2=0_1111,
    // giving funct12=0x08F in both cases when funct12=(funct7<<5)|rs2, but the full
    // I-type encoding differs because funct3 occupies bits [14:12] separately.
    // Verify the decoder distinguishes them.
    let zip_inst = 0b0000100_01111_00010_001_00001_0010011u32;
    let unzip_inst = 0b0000100_01111_00010_101_00001_0010011u32;
    assert_ne!(
        Rv32ZbkbInstruction::<BasicHart<Reg<u32>>>::try_decode(zip_inst),
        Rv32ZbkbInstruction::<BasicHart<Reg<u32>>>::try_decode(unzip_inst),
    );
}

#[test]
fn test_pack_wrong_funct7_returns_none() {
    // funct7=0b000_0000 with pack's funct3=100 is just XOR; should not decode as pack
    let inst = make_r_type(0b011_0011, 1, 0b100, 2, 3, 0b000_0000);
    let decoded = Rv32ZbkbInstruction::<BasicHart<Reg<u32>>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_packw_opcode_not_decoded_in_rv32() {
    // 0b011_1011 (OP-32) does not exist in RV32; must return None
    let inst = make_r_type(0b011_1011, 1, 0b100, 2, 3, 0b000_0100);
    let decoded = Rv32ZbkbInstruction::<BasicHart<Reg<u32>>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_unknown_opcode_returns_none() {
    let inst = make_r_type(0b010_0011, 1, 0b100, 2, 3, 0b000_0100);
    let decoded = Rv32ZbkbInstruction::<BasicHart<Reg<u32>>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_shared_with_zbb() {
    let andn = make_r_type(0b011_0011, 1, 0b111, 2, 3, 0b010_0000);
    assert_eq!(
        Rv32ZbkbInstruction::<BasicHart<Reg<u32>>>::try_decode(andn),
        Some(Rv32ZbkbInstruction::Andn {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
        })
    );
    let rev8 = make_i_type(0b001_0011, 1, 0b101, 2, 0b0110_1001_1000);
    assert_eq!(
        Rv32ZbkbInstruction::<BasicHart<Reg<u32>>>::try_decode(rev8),
        Some(Rv32ZbkbInstruction::Rev8 {
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
        assert_eq!(
            Rv32ZbkbInstruction::<BasicHart<Reg<u32>>>::try_decode(inst),
            None
        );
        assert!(Rv32ZbbInstruction::<BasicHart<Reg<u32>>>::try_decode(inst).is_some());
    }
}
