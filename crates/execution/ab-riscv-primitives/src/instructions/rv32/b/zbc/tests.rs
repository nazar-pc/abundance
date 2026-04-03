use crate::instructions::Instruction;
use crate::instructions::rv32::b::zbc::Rv32ZbcInstruction;
use crate::instructions::test_utils::make_r_type;
use crate::registers::general_purpose::Reg;

#[test]
fn test_clmul() {
    let inst = make_r_type(0b0110011, 1, 0b001, 2, 3, 0b0000101);
    let decoded = Rv32ZbcInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbcInstruction::Clmul {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_clmulh() {
    let inst = make_r_type(0b0110011, 1, 0b011, 2, 3, 0b0000101);
    let decoded = Rv32ZbcInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbcInstruction::Clmulh {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_clmulr() {
    let inst = make_r_type(0b0110011, 1, 0b010, 2, 3, 0b0000101);
    let decoded = Rv32ZbcInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbcInstruction::Clmulr {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}
