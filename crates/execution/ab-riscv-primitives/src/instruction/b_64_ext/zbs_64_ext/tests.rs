use crate::instruction::GenericInstruction;
use crate::instruction::b_64_ext::zbs_64_ext::Zbs64ExtInstruction;
use crate::instruction::test_utils::{make_i_type_with_shamt, make_r_type};
use crate::registers::Reg64;

#[test]
fn test_bset() {
    let inst = make_r_type(0b0110011, 1, 0b001, 2, 3, 0b0010100);
    let decoded = Zbs64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbs64ExtInstruction::Bset {
            rd: Reg64::Ra,
            rs1: Reg64::Sp,
            rs2: Reg64::Gp
        })
    );
}

#[test]
fn test_bseti() {
    let inst = make_i_type_with_shamt(0b0010011, 1, 0b001, 2, 5, 0b001010);
    let decoded = Zbs64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbs64ExtInstruction::Bseti {
            rd: Reg64::Ra,
            rs1: Reg64::Sp,
            shamt: 5
        })
    );
}

#[test]
fn test_bclr() {
    let inst = make_r_type(0b0110011, 1, 0b001, 2, 3, 0b0100100);
    let decoded = Zbs64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbs64ExtInstruction::Bclr {
            rd: Reg64::Ra,
            rs1: Reg64::Sp,
            rs2: Reg64::Gp
        })
    );
}

#[test]
fn test_bclri() {
    let inst = make_i_type_with_shamt(0b0010011, 1, 0b001, 2, 10, 0b010010);
    let decoded = Zbs64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbs64ExtInstruction::Bclri {
            rd: Reg64::Ra,
            rs1: Reg64::Sp,
            shamt: 10
        })
    );
}

#[test]
fn test_binv() {
    let inst = make_r_type(0b0110011, 1, 0b001, 2, 3, 0b0110100);
    let decoded = Zbs64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbs64ExtInstruction::Binv {
            rd: Reg64::Ra,
            rs1: Reg64::Sp,
            rs2: Reg64::Gp
        })
    );
}

#[test]
fn test_binvi() {
    let inst = make_i_type_with_shamt(0b0010011, 1, 0b001, 2, 63, 0b011010);
    let decoded = Zbs64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbs64ExtInstruction::Binvi {
            rd: Reg64::Ra,
            rs1: Reg64::Sp,
            shamt: 63
        })
    );
}

#[test]
fn test_bext() {
    let inst = make_r_type(0b0110011, 1, 0b101, 2, 3, 0b0100100);
    let decoded = Zbs64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbs64ExtInstruction::Bext {
            rd: Reg64::Ra,
            rs1: Reg64::Sp,
            rs2: Reg64::Gp
        })
    );
}

#[test]
fn test_bexti() {
    let inst = make_i_type_with_shamt(0b0010011, 1, 0b101, 2, 31, 0b010010);
    let decoded = Zbs64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbs64ExtInstruction::Bexti {
            rd: Reg64::Ra,
            rs1: Reg64::Sp,
            shamt: 31
        })
    );
}
