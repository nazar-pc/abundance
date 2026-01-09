use crate::instruction::Instruction;
use crate::instruction::b_64_ext::zbc_64_ext::Zbc64ExtInstruction;
use crate::instruction::test_utils::make_r_type;
use crate::registers::Reg;

#[test]
fn test_clmul() {
    let inst = make_r_type(0b0110011, 1, 0b001, 2, 3, 0b0000101);
    let decoded = Zbc64ExtInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbc64ExtInstruction::Clmul {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_clmulh() {
    let inst = make_r_type(0b0110011, 1, 0b011, 2, 3, 0b0000101);
    let decoded = Zbc64ExtInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbc64ExtInstruction::Clmulh {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_clmulr() {
    let inst = make_r_type(0b0110011, 1, 0b010, 2, 3, 0b0000101);
    let decoded = Zbc64ExtInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbc64ExtInstruction::Clmulr {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}
