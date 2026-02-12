use crate::instructions::Instruction;
use crate::instructions::rv64::b::zbs::Rv64ZbsInstruction;
use crate::instructions::test_utils::{make_i_type_with_shamt, make_r_type};
use crate::registers::Reg;

#[test]
fn test_bset() {
    let inst = make_r_type(0b0110011, 1, 0b001, 2, 3, 0b0010100);
    let decoded = Rv64ZbsInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbsInstruction::Bset {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_bseti() {
    let inst = make_i_type_with_shamt(0b0010011, 1, 0b001, 2, 5, 0b001010);
    let decoded = Rv64ZbsInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbsInstruction::Bseti {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            shamt: 5
        })
    );
}

#[test]
fn test_bclr() {
    let inst = make_r_type(0b0110011, 1, 0b001, 2, 3, 0b0100100);
    let decoded = Rv64ZbsInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbsInstruction::Bclr {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_bclri() {
    let inst = make_i_type_with_shamt(0b0010011, 1, 0b001, 2, 10, 0b010010);
    let decoded = Rv64ZbsInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbsInstruction::Bclri {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            shamt: 10
        })
    );
}

#[test]
fn test_binv() {
    let inst = make_r_type(0b0110011, 1, 0b001, 2, 3, 0b0110100);
    let decoded = Rv64ZbsInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbsInstruction::Binv {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_binvi() {
    let inst = make_i_type_with_shamt(0b0010011, 1, 0b001, 2, 63, 0b011010);
    let decoded = Rv64ZbsInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbsInstruction::Binvi {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            shamt: 63
        })
    );
}

#[test]
fn test_bext() {
    let inst = make_r_type(0b0110011, 1, 0b101, 2, 3, 0b0100100);
    let decoded = Rv64ZbsInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbsInstruction::Bext {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_bexti() {
    let inst = make_i_type_with_shamt(0b0010011, 1, 0b101, 2, 31, 0b010010);
    let decoded = Rv64ZbsInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbsInstruction::Bexti {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            shamt: 31
        })
    );
}
