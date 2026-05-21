use crate::instructions::Instruction;
use crate::instructions::rv32::b::zbs::Rv32ZbsInstruction;
use crate::instructions::test_utils::{make_i_type_with_shamt, make_r_type};
use crate::registers::general_purpose::Reg;

#[test]
fn test_bset() {
    let inst = make_r_type(0b011_0011, 1, 0b001, 2, 3, 0b001_0100);
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
    let inst = make_i_type_with_shamt(0b001_0011, 1, 0b001, 2, 5, 0b00_1010);
    let decoded = Rv32ZbsInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbsInstruction::Bseti {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            shamt: 5,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_bclr() {
    let inst = make_r_type(0b011_0011, 1, 0b001, 2, 3, 0b010_0100);
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
    let inst = make_i_type_with_shamt(0b001_0011, 1, 0b001, 2, 10, 0b01_0010);
    let decoded = Rv32ZbsInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbsInstruction::Bclri {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            shamt: 10,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_binv() {
    let inst = make_r_type(0b011_0011, 1, 0b001, 2, 3, 0b011_0100);
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
    let inst = make_i_type_with_shamt(0b001_0011, 1, 0b001, 2, 31, 0b01_1010);
    let decoded = Rv32ZbsInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbsInstruction::Binvi {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            shamt: 31,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_bext() {
    let inst = make_r_type(0b011_0011, 1, 0b101, 2, 3, 0b010_0100);
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
    let inst = make_i_type_with_shamt(0b001_0011, 1, 0b101, 2, 31, 0b01_0010);
    let decoded = Rv32ZbsInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbsInstruction::Bexti {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            shamt: 31,
            rs2: Reg::Zero,
        })
    );
}
