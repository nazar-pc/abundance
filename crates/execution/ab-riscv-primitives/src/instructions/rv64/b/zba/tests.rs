use crate::instructions::Instruction;
use crate::instructions::rv64::b::zba::Rv64ZbaInstruction;
use crate::instructions::test_utils::{make_i_type_with_shamt, make_r_type};
use crate::registers::general_purpose::Reg;

#[test]
fn test_sh1add() {
    let inst = make_r_type(0b0110011, 1, 0b010, 2, 3, 0b0010000);
    let decoded = Rv64ZbaInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbaInstruction::Sh1add {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_sh2add() {
    let inst = make_r_type(0b0110011, 1, 0b100, 2, 3, 0b0010000);
    let decoded = Rv64ZbaInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbaInstruction::Sh2add {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_sh3add() {
    let inst = make_r_type(0b0110011, 1, 0b110, 2, 3, 0b0010000);
    let decoded = Rv64ZbaInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbaInstruction::Sh3add {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_add_uw() {
    let inst = make_r_type(0b0111011, 1, 0b000, 2, 3, 0b0000100);
    let decoded = Rv64ZbaInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbaInstruction::AddUw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_slli_uw() {
    let inst = make_i_type_with_shamt(0b0111011, 1, 0b001, 2, 5, 0b000010);
    let decoded = Rv64ZbaInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbaInstruction::SlliUw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            shamt: 5
        })
    );
}

#[test]
fn test_sh1add_uw() {
    let inst = make_r_type(0b0111011, 1, 0b010, 2, 3, 0b0010000);
    let decoded = Rv64ZbaInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbaInstruction::Sh1addUw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_sh2add_uw() {
    let inst = make_r_type(0b0111011, 1, 0b100, 2, 3, 0b0010000);
    let decoded = Rv64ZbaInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbaInstruction::Sh2addUw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_sh3add_uw() {
    let inst = make_r_type(0b0111011, 1, 0b110, 2, 3, 0b0010000);
    let decoded = Rv64ZbaInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbaInstruction::Sh3addUw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}
