#![expect(clippy::unusual_byte_groupings, reason = "Test readability")]

use crate::instructions::Instruction;
use crate::instructions::rv64::zk::zbkb::Rv64ZbkbInstruction;
use crate::instructions::test_utils::make_r_type;
use crate::registers::general_purpose::Reg;

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
fn test_packw_rs2_zero_returns_none() {
    // rs2=0 with packw encoding is zext.h (Zbb); must not decode as packw
    let inst = make_r_type(0b011_1011, 1, 0b100, 2, 0, 0b000_0100);
    let decoded = Rv64ZbkbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
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
