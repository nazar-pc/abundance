use crate::rv64::test_utils::{execute, initialize_test_instruction_state};
use ab_riscv_primitives::instruction::rv64::b::zba::Rv64ZbaInstruction;
use ab_riscv_primitives::registers::EReg;

#[test]
fn test_add_uw() {
    let mut state = initialize_test_instruction_state([Rv64ZbaInstruction::AddUw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 0xFFFF_FFFF_8000_0000u64);
    state.regs.write(EReg::A1, 0x1000);

    execute(&mut state).unwrap();

    // Only the lower 32 bits are zero-extended
    assert_eq!(state.regs.read(EReg::A2), 0x8000_1000);
}

#[test]
fn test_sh1add() {
    let mut state = initialize_test_instruction_state([Rv64ZbaInstruction::Sh1add {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 10);
    state.regs.write(EReg::A1, 100);

    execute(&mut state).unwrap();

    // (10 << 1) + 100 = 20 + 100 = 120
    assert_eq!(state.regs.read(EReg::A2), 120);
}

#[test]
fn test_sh2add() {
    let mut state = initialize_test_instruction_state([Rv64ZbaInstruction::Sh2add {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 10);
    state.regs.write(EReg::A1, 100);

    execute(&mut state).unwrap();

    // (10 << 2) + 100 = 40 + 100 = 140
    assert_eq!(state.regs.read(EReg::A2), 140);
}

#[test]
fn test_sh3add() {
    let mut state = initialize_test_instruction_state([Rv64ZbaInstruction::Sh3add {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 10);
    state.regs.write(EReg::A1, 100);

    execute(&mut state).unwrap();

    // (10 << 3) + 100 = 80 + 100 = 180
    assert_eq!(state.regs.read(EReg::A2), 180);
}

#[test]
fn test_sh1add_uw() {
    let mut state = initialize_test_instruction_state([Rv64ZbaInstruction::Sh1addUw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 0xFFFF_FFFF_0000_000Au64);
    state.regs.write(EReg::A1, 100);

    execute(&mut state).unwrap();

    // Zero-extend lower 32 bits (10), shift left 1, add 100
    assert_eq!(state.regs.read(EReg::A2), 120);
}

#[test]
fn test_sh2add_uw() {
    let mut state = initialize_test_instruction_state([Rv64ZbaInstruction::Sh2addUw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 0xFFFF_FFFF_0000_000Au64);
    state.regs.write(EReg::A1, 100);

    execute(&mut state).unwrap();

    // Zero-extend lower 32 bits (10), shift left 2, add 100
    assert_eq!(state.regs.read(EReg::A2), 140);
}

#[test]
fn test_sh3add_uw() {
    let mut state = initialize_test_instruction_state([Rv64ZbaInstruction::Sh3addUw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 0xFFFF_FFFF_0000_000Au64);
    state.regs.write(EReg::A1, 100);

    execute(&mut state).unwrap();

    // Zero-extend lower 32 bits (10), shift left 3, add 100
    assert_eq!(state.regs.read(EReg::A2), 180);
}

#[test]
fn test_slli_uw() {
    let mut state = initialize_test_instruction_state([Rv64ZbaInstruction::SlliUw {
        rd: EReg::A2,
        rs1: EReg::A0,
        shamt: 4,
    }]);

    state.regs.write(EReg::A0, 0xFFFF_FFFF_0000_0001u64);

    execute(&mut state).unwrap();

    // Zero-extend lower 32 bits (1), then shift left 4
    assert_eq!(state.regs.read(EReg::A2), 0x10);
}

#[test]
fn test_slli_uw_max_shamt() {
    let mut state = initialize_test_instruction_state([Rv64ZbaInstruction::SlliUw {
        rd: EReg::A2,
        rs1: EReg::A0,
        shamt: 63,
    }]);

    state.regs.write(EReg::A0, 0xFFFF_FFFF_0000_0001u64);

    execute(&mut state).unwrap();

    // Zero-extend lower 32 bits (1), then shift left 63
    assert_eq!(state.regs.read(EReg::A2), 1u64 << 63);
}
