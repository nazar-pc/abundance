use crate::RegisterFile;
use crate::rv32::test_utils::{execute, initialize_state};
use ab_riscv_primitives::prelude::*;
// xperm4 tests

#[test]
fn test_xperm4_basic() {
    let mut state = initialize_state([Rv32ZbkxInstruction::Xperm4 {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    // Nibbles 0–7 of rs1: nibble i = i
    state.regs.write(Reg::A0, 0x7654_3210);
    // Identity permutation
    state.regs.write(Reg::A1, 0x7654_3210);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0x7654_3210);
}

#[test]
fn test_xperm4_out_of_bounds_zeroed() {
    let mut state = initialize_state([Rv32ZbkxInstruction::Xperm4 {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0x7654_3210);
    // Nibble indices: 0,1,2,3 (in), 8,9,10,11 (out of bounds for RV32)
    state.regs.write(Reg::A1, 0xBA98_3210);

    execute(&mut state).unwrap();

    // Lower nibbles: indices 0–3 -> values 0–3; upper nibbles: indices 8–11 -> 0
    assert_eq!(state.regs.read(Reg::A2), 0x0000_3210);
}

#[test]
fn test_xperm4_constant_index() {
    let mut state = initialize_state([Rv32ZbkxInstruction::Xperm4 {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    // rs1 nibble 2 = 0xC
    state.regs.write(Reg::A0, 0x0000_0C00);
    // All 8 nibbles of rs2 are index 2
    state.regs.write(Reg::A1, 0x2222_2222);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xCCCC_CCCC);
}

#[test]
fn test_xperm4_max_in_bounds_index() {
    let mut state = initialize_state([Rv32ZbkxInstruction::Xperm4 {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    // rs1 nibble 7 (highest in-bounds for RV32) = 0xA
    state.regs.write(Reg::A0, 0xA000_0000);
    // All indices are 7 - the last valid index
    state.regs.write(Reg::A1, 0x7777_7777);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xAAAA_AAAA);
}

#[test]
fn test_xperm4_zero_lut() {
    let mut state = initialize_state([Rv32ZbkxInstruction::Xperm4 {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0x0);
    state.regs.write(Reg::A1, 0x7654_3210);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0x0);
}

// xperm8 tests

#[test]
fn test_xperm8_basic() {
    let mut state = initialize_state([Rv32ZbkxInstruction::Xperm8 {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    // rs1 bytes: 0->0x10, 1->0x20, 2->0x30, 3->0x40
    state.regs.write(Reg::A0, 0x4030_2010);
    // Select bytes 0,1,2,3 in order
    state.regs.write(Reg::A1, 0x0302_0100);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0x4030_2010);
}

#[test]
fn test_xperm8_out_of_bounds_zeroed() {
    let mut state = initialize_state([Rv32ZbkxInstruction::Xperm8 {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0x0403_0201);
    // Indices: 0 (in), 4 (out), 255 (out), 0 (in)
    state.regs.write(Reg::A1, 0x00FF_0400);

    execute(&mut state).unwrap();

    // byte 0: index 0 -> 0x01, byte 1: index 4 -> 0x00, byte 2: index 255 -> 0x00, byte 3: index 0
    // -> 0x01
    assert_eq!(state.regs.read(Reg::A2), 0x0100_0001);
}

#[test]
fn test_xperm8_identity() {
    let mut state = initialize_state([Rv32ZbkxInstruction::Xperm8 {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    let lut = 0xDEAD_BEEFu32;
    state.regs.write(Reg::A0, lut);
    state.regs.write(Reg::A1, 0x0302_0100);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), lut);
}

#[test]
fn test_xperm8_reverse() {
    let mut state = initialize_state([Rv32ZbkxInstruction::Xperm8 {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0x0403_0201);
    state.regs.write(Reg::A1, 0x0001_0203);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0x0102_0304);
}
