use crate::instructions::Instruction;
use crate::instructions::rv32::zk::zbkx::Rv32ZbkxInstruction;
use crate::instructions::test_utils::make_r_type;
use crate::registers::general_purpose::Reg;

#[test]
fn test_xperm4() {
    let inst = make_r_type(0b011_0011, 1, 0b010, 2, 3, 0b001_0100);
    let decoded = Rv32ZbkxInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbkxInstruction::Xperm4 {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_xperm8() {
    let inst = make_r_type(0b011_0011, 1, 0b100, 2, 3, 0b001_0100);
    let decoded = Rv32ZbkxInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbkxInstruction::Xperm8 {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_wrong_funct7_no_decode() {
    let inst = make_r_type(0b011_0011, 1, 0b010, 2, 3, 0b000_0000);
    let decoded = Rv32ZbkxInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_wrong_opcode_no_decode() {
    // OP-32 opcode is RV64-only and must not decode under RV32
    let inst = make_r_type(0b011_1011, 1, 0b000, 2, 3, 0b001_0100);
    let decoded = Rv32ZbkxInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(decoded, None);
}
