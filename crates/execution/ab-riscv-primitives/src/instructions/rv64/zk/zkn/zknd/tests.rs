use crate::instructions::Instruction;
use crate::instructions::rv64::zk::zkn::zknd::{Rv64ZkndInstruction, Rv64ZkndKsRnum};
use crate::instructions::test_utils::make_r_type;
use crate::registers::general_purpose::Reg;

fn make_i_type(opcode: u32, rd: u32, funct3: u32, rs1: u32, imm12: u32) -> u32 {
    (imm12 << 20) | (rs1 << 15) | (funct3 << 12) | (rd << 7) | opcode
}

// R-type instruction decoding

#[test]
fn test_aes64ds() {
    let inst = make_r_type(0b011_0011, 1, 0b000, 2, 3, 0b001_1101);
    let decoded = Rv64ZkndInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZkndInstruction::Aes64Ds {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
        })
    );
}

#[test]
fn test_aes64dsm() {
    let inst = make_r_type(0b011_0011, 1, 0b000, 2, 3, 0b001_1111);
    let decoded = Rv64ZkndInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZkndInstruction::Aes64Dsm {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
        })
    );
}

#[test]
fn test_aes64ks2() {
    let inst = make_r_type(0b011_0011, 1, 0b000, 2, 3, 0b011_1111);
    let decoded = Rv64ZkndInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZkndInstruction::Aes64Ks2 {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
        })
    );
}

#[test]
fn test_wrong_funct3_rejected() {
    let inst = make_r_type(0b011_0011, 1, 0b001, 2, 3, 0b001_1101);
    let decoded = Rv64ZkndInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

// aes64im decoding

#[test]
fn test_aes64im() {
    // imm12=0x300: funct7=0b001_1000, rs2=0b0_0000
    let inst = make_i_type(0b001_0011, 1, 0b001, 2, 0x300);
    let decoded = Rv64ZkndInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZkndInstruction::Aes64Im {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_aes64im_nonzero_rs2_rejected() {
    // imm12=0x301: rnum=1 in the rs2 field; not a valid aes64im
    // (decodes as aes64ks1i rnum=1 instead - covered in ks1i tests)
    let inst = make_i_type(0b001_0011, 1, 0b001, 2, 0x301);
    let decoded = Rv64ZkndInstruction::<Reg<u64>>::try_decode(inst);
    assert_ne!(
        decoded,
        Some(Rv64ZkndInstruction::Aes64Im {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
}

// aes64ks1i decoding
//
// Both aes64im and aes64ks1i share opcode=0x13, funct3=0b001, and
// bits[31:25]=0b001_1000 (funct7=0x18). They are distinguished by bit 24 of the instruction (bit 4
// of imm12):
//
//   bit 4 = 0: aes64im      imm12 = 0x300        (rs2 field = 0b00000)
//   bit 4 = 1: aes64ks1i    imm12 = 0x310..=0x31A (rnum in bits[23:20])
//
// imm12=0x31B..=0x31F (rnum 11..=15) are reserved and must be rejected.

#[test]
fn test_aes64ks1i_rnum_0() {
    // imm12 = 0x310: bit4=1, rnum=0
    let inst = make_i_type(0b001_0011, 1, 0b001, 2, 0x310);
    let decoded = Rv64ZkndInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZkndInstruction::Aes64Ks1i {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rnum: Rv64ZkndKsRnum::R0,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_aes64ks1i_rnum_7() {
    // imm12 = 0x317: bit4=1, rnum=7
    let inst = make_i_type(0b001_0011, 1, 0b001, 2, 0x317);
    let decoded = Rv64ZkndInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZkndInstruction::Aes64Ks1i {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rnum: Rv64ZkndKsRnum::R7,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_aes64ks1i_rnum_10() {
    // imm12 = 0x31A: bit4=1, rnum=10
    let inst = make_i_type(0b001_0011, 1, 0b001, 2, 0x31A);
    let decoded = Rv64ZkndInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZkndInstruction::Aes64Ks1i {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rnum: Rv64ZkndKsRnum::Final,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_aes64ks1i_rnum_11_rejected() {
    // rnum=0xB is illegal per spec
    let inst = make_i_type(0b001_0011, 1, 0b001, 2, 0x31B);
    let decoded = Rv64ZkndInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_aes64im_bit4_zero_rnum0() {
    // imm12=0x300: bit4=0, rnum=0 - this is aes64im, NOT aes64ks1i
    let inst = make_i_type(0b001_0011, 1, 0b001, 2, 0x300);
    let decoded = Rv64ZkndInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZkndInstruction::Aes64Im {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_aes64ks1i_bit4_zero_nonzero_rnum_rejected() {
    // imm12=0x301..=0x30A: bit4=0, rnum 1..=10 - these are NOT valid ks1i or im encodings
    for rnum in 0x1u32..=0xAu32 {
        let inst = make_i_type(0b001_0011, 1, 0b001, 2, 0x300 | rnum);
        let decoded = Rv64ZkndInstruction::<Reg<u64>>::try_decode(inst);
        assert_eq!(
            decoded,
            None,
            "imm12=0x{:03X} should be rejected",
            0x300 | rnum
        );
    }
}
