#![expect(clippy::unusual_byte_groupings, reason = "Test readability")]

use crate::instructions::Instruction;
use crate::instructions::rv32::zk::zbkb::Rv32ZbkbInstruction;
use crate::instructions::test_utils::make_r_type;
use crate::registers::general_purpose::Reg;

#[test]
fn test_pack() {
    // pack: opcode=0b0110011, funct3=0b100, funct7=0b0000100
    let inst = make_r_type(0b0110011, 1, 0b100, 2, 3, 0b0000100);
    let decoded = Rv32ZbkbInstruction::<Reg<u32>>::try_decode(inst);
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
fn test_pack_rs2_zero_returns_none() {
    // rs2=0 with pack encoding collides with zext.h (Zbb). Rv32ZbkbInstruction does not
    // own that encoding, so it must return None and defer to the Zbb decoder layer.
    let inst = make_r_type(0b0110011, 1, 0b100, 2, 0, 0b0000100);
    let decoded = Rv32ZbkbInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_packh() {
    // packh: opcode=0b0110011, funct3=0b111, funct7=0b0000100
    let inst = make_r_type(0b0110011, 1, 0b111, 2, 3, 0b0000100);
    let decoded = Rv32ZbkbInstruction::<Reg<u32>>::try_decode(inst);
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
    let decoded = Rv32ZbkbInstruction::<Reg<u32>>::try_decode(inst);
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
    // zip: OP-IMM, funct3=001, funct7=0000100, rs2=01111 (x15) -> funct12=0x08F
    let inst = 0b0000100_01111_00010_001_00001_0010011u32;
    let decoded = Rv32ZbkbInstruction::<Reg<u32>>::try_decode(inst);
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
    // unzip: OP-IMM, funct3=101, funct7=0000100, rs2=01111 (x15) -> funct12=0x08F
    let inst = 0b0000100_01111_00010_101_00001_0010011u32;
    let decoded = Rv32ZbkbInstruction::<Reg<u32>>::try_decode(inst);
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
    // zip (funct3=001) and unzip (funct3=101) share funct7=0000100 and rs2=01111,
    // giving funct12=0x08F in both cases when funct12=(funct7<<5)|rs2, but the full
    // I-type encoding differs because funct3 occupies bits [14:12] separately.
    // Verify the decoder distinguishes them.
    let zip_inst = 0b0000100_01111_00010_001_00001_0010011u32;
    let unzip_inst = 0b0000100_01111_00010_101_00001_0010011u32;
    assert_ne!(
        Rv32ZbkbInstruction::<Reg<u32>>::try_decode(zip_inst),
        Rv32ZbkbInstruction::<Reg<u32>>::try_decode(unzip_inst),
    );
}

#[test]
fn test_pack_wrong_funct7_returns_none() {
    // funct7=0b0000000 with pack's funct3=100 is just XOR; should not decode as pack
    let inst = make_r_type(0b0110011, 1, 0b100, 2, 3, 0b0000000);
    let decoded = Rv32ZbkbInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_packw_opcode_not_decoded_in_rv32() {
    // 0b0111011 (OP-32) does not exist in RV32; must return None
    let inst = make_r_type(0b0111011, 1, 0b100, 2, 3, 0b0000100);
    let decoded = Rv32ZbkbInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_unknown_opcode_returns_none() {
    let inst = make_r_type(0b0100011, 1, 0b100, 2, 3, 0b0000100);
    let decoded = Rv32ZbkbInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(decoded, None);
}
