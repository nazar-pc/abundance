use crate::instructions::Instruction;
use crate::instructions::rv64::zk::zkn::zkne::Rv64ZkneInstruction;
use crate::instructions::test_utils::make_r_type;
use crate::registers::general_purpose::Reg;

#[test]
fn test_aes64es() {
    let inst = make_r_type(0b0110011, 1, 0b000, 2, 3, 0b0011001);
    let decoded = Rv64ZkneInstruction::<Reg<u64>>::try_decode(inst);
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
    let inst = make_r_type(0b0110011, 1, 0b000, 2, 3, 0b0011011);
    let decoded = Rv64ZkneInstruction::<Reg<u64>>::try_decode(inst);
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
    let inst = make_r_type(0b0110011, 1, 0b001, 2, 3, 0b0011001);
    let decoded = Rv64ZkneInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_wrong_funct7_rejected() {
    // funct7 from aes64ds - must not match Zkne
    let inst = make_r_type(0b0110011, 1, 0b000, 2, 3, 0b0011101);
    let decoded = Rv64ZkneInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_wrong_opcode_rejected() {
    // I-type opcode (0x13) must not match
    let inst = make_r_type(0b0010011, 1, 0b000, 2, 3, 0b0011001);
    let decoded = Rv64ZkneInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}
