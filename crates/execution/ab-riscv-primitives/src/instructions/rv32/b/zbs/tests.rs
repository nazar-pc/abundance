use crate::instructions::Instruction;
use crate::instructions::rv32::b::zbs::Rv32ZbsInstruction;
use crate::instructions::test_utils::{make_i_type_with_shamt, make_r_type};
use crate::registers::general_purpose::Reg;

#[test]
fn test_bset() {
    let inst = make_r_type(0b0110011, 1, 0b001, 2, 3, 0b0010100);
    let decoded = Rv32ZbsInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbsInstruction::Bset {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_bseti() {
    let inst = make_i_type_with_shamt(0b0010011, 1, 0b001, 2, 5, 0b001010);
    let decoded = Rv32ZbsInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbsInstruction::Bseti {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            shamt: 5
        })
    );
}

#[test]
fn test_bclr() {
    let inst = make_r_type(0b0110011, 1, 0b001, 2, 3, 0b0100100);
    let decoded = Rv32ZbsInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbsInstruction::Bclr {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_bclri() {
    let inst = make_i_type_with_shamt(0b0010011, 1, 0b001, 2, 10, 0b010010);
    let decoded = Rv32ZbsInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbsInstruction::Bclri {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            shamt: 10
        })
    );
}

#[test]
fn test_binv() {
    let inst = make_r_type(0b0110011, 1, 0b001, 2, 3, 0b0110100);
    let decoded = Rv32ZbsInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbsInstruction::Binv {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_binvi() {
    let inst = make_i_type_with_shamt(0b0010011, 1, 0b001, 2, 31, 0b011010);
    let decoded = Rv32ZbsInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbsInstruction::Binvi {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            shamt: 31
        })
    );
}

#[test]
fn test_bext() {
    let inst = make_r_type(0b0110011, 1, 0b101, 2, 3, 0b0100100);
    let decoded = Rv32ZbsInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbsInstruction::Bext {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_bexti() {
    let inst = make_i_type_with_shamt(0b0010011, 1, 0b101, 2, 31, 0b010010);
    let decoded = Rv32ZbsInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbsInstruction::Bexti {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            shamt: 31
        })
    );
}
