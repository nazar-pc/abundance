use crate::instruction::GenericInstruction;
use crate::instruction::b_64_ext::zbc_64_ext::Zbc64ExtInstruction;
use crate::instruction::test_utils::make_r_type;
use crate::registers::Reg64;

#[test]
fn test_clmul() {
    let inst = make_r_type(0b0110011, 1, 0b001, 2, 3, 0b0000101);
    let decoded = Zbc64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbc64ExtInstruction::Clmul {
            rd: Reg64::Ra,
            rs1: Reg64::Sp,
            rs2: Reg64::Gp
        })
    );
}

#[test]
fn test_clmulh() {
    let inst = make_r_type(0b0110011, 1, 0b011, 2, 3, 0b0000101);
    let decoded = Zbc64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbc64ExtInstruction::Clmulh {
            rd: Reg64::Ra,
            rs1: Reg64::Sp,
            rs2: Reg64::Gp
        })
    );
}

#[test]
fn test_clmulr() {
    let inst = make_r_type(0b0110011, 1, 0b010, 2, 3, 0b0000101);
    let decoded = Zbc64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbc64ExtInstruction::Clmulr {
            rd: Reg64::Ra,
            rs1: Reg64::Sp,
            rs2: Reg64::Gp
        })
    );
}
