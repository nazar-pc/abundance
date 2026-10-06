use crate::hart::BasicHart;
use crate::instructions::Instruction;
use crate::instructions::rv64::zk::zkn::zknd::{Rv64ZkndInstruction, Rv64ZkndKsRnum};
use crate::instructions::rv64::zk::zkn::zkne::Rv64ZkneInstruction;
use crate::instructions::test_utils::make_r_type;
use crate::registers::general_purpose::Reg;

#[test]
fn test_aes64es() {
    let inst = make_r_type(0b011_0011, 1, 0b000, 2, 3, 0b00_11001);
    let decoded = Rv64ZkneInstruction::<BasicHart<Reg<u64>>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZkneInstruction::Aes64Es {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
        })
    );
}

#[test]
fn test_aes64esm() {
    let inst = make_r_type(0b011_0011, 1, 0b000, 2, 3, 0b00_11011);
    let decoded = Rv64ZkneInstruction::<BasicHart<Reg<u64>>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZkneInstruction::Aes64Esm {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
        })
    );
}

#[test]
fn test_wrong_funct3_rejected() {
    let inst = make_r_type(0b011_0011, 1, 0b001, 2, 3, 0b00_11001);
    let decoded = Rv64ZkneInstruction::<BasicHart<Reg<u64>>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_wrong_funct7_rejected() {
    // funct7 from aes64ds - must not match Zkne
    let inst = make_r_type(0b011_0011, 1, 0b000, 2, 3, 0b00_11101);
    let decoded = Rv64ZkneInstruction::<BasicHart<Reg<u64>>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_wrong_opcode_rejected() {
    // I-type opcode (0x13) must not match
    let inst = make_r_type(0b001_0011, 1, 0b000, 2, 3, 0b00_11001);
    let decoded = Rv64ZkneInstruction::<BasicHart<Reg<u64>>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_shared_key_schedule() {
    // aes64ks1i rnum=3: imm12 = 0x313
    let ks1i = (0x313 << 20) | (2 << 15) | (0b001 << 12) | (1 << 7) | 0b001_0011;
    assert_eq!(
        Rv64ZkneInstruction::<BasicHart<Reg<u64>>>::try_decode(ks1i),
        Some(Rv64ZkneInstruction::Aes64Ks1i {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rnum: Rv64ZkndKsRnum::R3,
            rs2: Reg::Zero,
        })
    );
    let ks2 = make_r_type(0b011_0011, 1, 0b000, 2, 3, 0b011_1111);
    assert_eq!(
        Rv64ZkneInstruction::<BasicHart<Reg<u64>>>::try_decode(ks2),
        Some(Rv64ZkneInstruction::Aes64Ks2 {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
        })
    );
}

#[test]
fn test_decryption_rejected() {
    let aes64ds = make_r_type(0b011_0011, 1, 0b000, 2, 3, 0b001_1101);
    let aes64dsm = make_r_type(0b011_0011, 1, 0b000, 2, 3, 0b001_1111);
    // aes64im: imm12 = 0x300
    let aes64im = (0x300 << 20) | (2 << 15) | (0b001 << 12) | (1 << 7) | 0b001_0011;
    for inst in [aes64ds, aes64dsm, aes64im] {
        assert_eq!(
            Rv64ZkneInstruction::<BasicHart<Reg<u64>>>::try_decode(inst),
            None
        );
        assert!(Rv64ZkndInstruction::<BasicHart<Reg<u64>>>::try_decode(inst).is_some());
    }
}
