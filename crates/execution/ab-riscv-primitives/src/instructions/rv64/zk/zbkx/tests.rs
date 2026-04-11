use crate::instructions::Instruction;
use crate::instructions::rv64::zk::zbkx::Rv64ZbkxInstruction;
use crate::instructions::test_utils::make_r_type;
use crate::registers::general_purpose::Reg;

#[test]
fn test_xperm4() {
    let inst = make_r_type(0b0110011, 1, 0b010, 2, 3, 0b0010100);
    let decoded = Rv64ZbkxInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbkxInstruction::Xperm4 {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_xperm8() {
    let inst = make_r_type(0b0110011, 1, 0b100, 2, 3, 0b0010100);
    let decoded = Rv64ZbkxInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbkxInstruction::Xperm8 {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_wrong_funct7_no_decode() {
    // funct3=0b010 (SLT), funct7=0b0000000: wrong funct7 must not decode as Zbkx
    let inst = make_r_type(0b0110011, 1, 0b010, 2, 3, 0b0000000);
    let decoded = Rv64ZbkxInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_wrong_opcode_no_decode() {
    // OP-32 opcode (0b0111011) is not valid for Zbkx
    let inst = make_r_type(0b0111011, 1, 0b000, 2, 3, 0b0010100);
    let decoded = Rv64ZbkxInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}
