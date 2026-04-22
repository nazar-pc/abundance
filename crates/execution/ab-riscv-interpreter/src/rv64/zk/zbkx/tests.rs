use crate::RegisterFile;
use crate::rv64::test_utils::{execute, initialize_state};
use ab_riscv_primitives::prelude::*;
// xperm4 tests

#[test]
fn test_xperm4_basic() {
    let mut state = initialize_state([Rv64ZbkxInstruction::Xperm4 {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    // Nibbles 0–15 of rs1: nibble i = i (nibble 0 = 0x0, nibble 15 = 0xF)
    state.regs.write(Reg::A0, 0xFEDCBA9876543210u64);
    // Identity: each nibble of rs2 selects nibble i
    state.regs.write(Reg::A1, 0xFEDCBA9876543210u64);

    execute(&mut state).unwrap();

    // xperm4 of a value with itself: nibble i of rs2 is i, so we look up nibble i of rs1
    // which is also i - identity maps through identity lut back to the lut itself
    assert_eq!(state.regs.read(Reg::A2), 0xFEDCBA9876543210u64);
}

#[test]
fn test_xperm4_constant_index() {
    let mut state = initialize_state([Rv64ZbkxInstruction::Xperm4 {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    // rs1 nibble 3 = 0xA
    state.regs.write(Reg::A0, 0x000000000000A000u64);
    // All 16 nibbles of rs2 are index 3
    state.regs.write(Reg::A1, 0x3333333333333333u64);

    execute(&mut state).unwrap();

    // Every output nibble should be 0xA
    assert_eq!(state.regs.read(Reg::A2), 0xAAAAAAAAAAAAAAAAu64);
}

#[test]
fn test_xperm4_no_out_of_bounds() {
    let mut state = initialize_state([Rv64ZbkxInstruction::Xperm4 {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0xFEDCBA9876543210u64);
    // Maximum nibble index is 0xF = 15, which is the last nibble - always in bounds
    state.regs.write(Reg::A1, 0xFFFFFFFFFFFFFFFFu64);

    execute(&mut state).unwrap();

    // Every output nibble = nibble 15 of rs1 = 0xF
    assert_eq!(state.regs.read(Reg::A2), 0xFFFFFFFFFFFFFFFFu64);
}

#[test]
fn test_xperm4_zero_lut() {
    let mut state = initialize_state([Rv64ZbkxInstruction::Xperm4 {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0x0u64);
    state.regs.write(Reg::A1, 0xFEDCBA9876543210u64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0x0u64);
}

// xperm8 tests

#[test]
fn test_xperm8_basic() {
    let mut state = initialize_state([Rv64ZbkxInstruction::Xperm8 {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    // rs1 bytes (index -> value): 0->0x10, 1->0x20, 2->0x30, 3->0x40, ...
    state.regs.write(Reg::A0, 0x80_70_60_50_40_30_20_10u64);
    // rs2 indices: select bytes 0,1,2,3 in order
    state.regs.write(Reg::A1, 0x03_02_01_00_03_02_01_00u64);

    execute(&mut state).unwrap();

    // Each index picks a byte from rs1
    assert_eq!(state.regs.read(Reg::A2), 0x40_30_20_10_40_30_20_10u64);
}

#[test]
fn test_xperm8_out_of_bounds_zeroed() {
    let mut state = initialize_state([Rv64ZbkxInstruction::Xperm8 {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0x08_07_06_05_04_03_02_01u64);
    // Indices 8–255 are out of bounds and must produce 0
    state.regs.write(Reg::A1, 0xFF_10_09_08_00_00_00_00u64);

    execute(&mut state).unwrap();

    // First four lanes: index 0->0x01, 0->0x01, 0->0x01, 0->0x01
    // Upper three lanes: indices 8,9,16 -> all out of bounds -> 0x00
    // Highest lane: index 0xFF -> out of bounds -> 0x00
    assert_eq!(state.regs.read(Reg::A2), 0x00_00_00_00_01_01_01_01u64);
}

#[test]
fn test_xperm8_identity() {
    let mut state = initialize_state([Rv64ZbkxInstruction::Xperm8 {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    let lut = 0xDE_AD_BE_EF_CA_FE_BA_BEu64;
    state.regs.write(Reg::A0, lut);
    // Identity permutation: index i selects byte i
    state.regs.write(Reg::A1, 0x07_06_05_04_03_02_01_00u64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), lut);
}

#[test]
fn test_xperm8_reverse() {
    let mut state = initialize_state([Rv64ZbkxInstruction::Xperm8 {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0x08_07_06_05_04_03_02_01u64);
    // Reverse permutation
    state.regs.write(Reg::A1, 0x00_01_02_03_04_05_06_07u64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0x01_02_03_04_05_06_07_08u64);
}
