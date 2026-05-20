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
    state.regs.write(Reg::A0, 0xFEDC_BA98_7654_3210);
    // Identity: each nibble of rs2 selects nibble i
    state.regs.write(Reg::A1, 0xFEDC_BA98_7654_3210);

    execute(&mut state).unwrap();

    // xperm4 of a value with itself: nibble i of rs2 is i, so we look up nibble i of rs1
    // which is also i - identity maps through identity lut back to the lut itself
    assert_eq!(state.regs.read(Reg::A2), 0xFEDC_BA98_7654_3210);
}

#[test]
fn test_xperm4_constant_index() {
    let mut state = initialize_state([Rv64ZbkxInstruction::Xperm4 {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    // rs1 nibble 3 = 0xA
    state.regs.write(Reg::A0, 0x0000_0000_0000_A000);
    // All 16 nibbles of rs2 are index 3
    state.regs.write(Reg::A1, 0x3333_3333_3333_3333);

    execute(&mut state).unwrap();

    // Every output nibble should be 0xA
    assert_eq!(state.regs.read(Reg::A2), 0xAAAA_AAAA_AAAA_AAAA);
}

#[test]
fn test_xperm4_no_out_of_bounds() {
    let mut state = initialize_state([Rv64ZbkxInstruction::Xperm4 {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0xFEDC_BA98_7654_3210);
    // Maximum nibble index is 0xF = 15, which is the last nibble - always in bounds
    state.regs.write(Reg::A1, 0xFFFF_FFFF_FFFF_FFFF);

    execute(&mut state).unwrap();

    // Every output nibble = nibble 15 of rs1 = 0xF
    assert_eq!(state.regs.read(Reg::A2), 0xFFFF_FFFF_FFFF_FFFF);
}

#[test]
fn test_xperm4_zero_lut() {
    let mut state = initialize_state([Rv64ZbkxInstruction::Xperm4 {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0x0);
    state.regs.write(Reg::A1, 0xFEDC_BA98_7654_3210);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0x0);
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
    state.regs.write(Reg::A0, 0x8070_6050_4030_2010);
    // rs2 indices: select bytes 0,1,2,3 in order
    state.regs.write(Reg::A1, 0x0302_0100_0302_0100);

    execute(&mut state).unwrap();

    // Each index picks a byte from rs1
    assert_eq!(state.regs.read(Reg::A2), 0x4030_2010_4030_2010);
}

#[test]
fn test_xperm8_out_of_bounds_zeroed() {
    let mut state = initialize_state([Rv64ZbkxInstruction::Xperm8 {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0x0807_0605_0403_0201);
    // Indices 8–255 are out of bounds and must produce 0
    state.regs.write(Reg::A1, 0xFF10_0908_0000_0000);

    execute(&mut state).unwrap();

    // First four lanes: index 0->0x01, 0->0x01, 0->0x01, 0->0x01
    // Upper three lanes: indices 8,9,16 -> all out of bounds -> 0x00
    // Highest lane: index 0xFF -> out of bounds -> 0x00
    assert_eq!(state.regs.read(Reg::A2), 0x0000_0000_0101_0101);
}

#[test]
fn test_xperm8_identity() {
    let mut state = initialize_state([Rv64ZbkxInstruction::Xperm8 {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    let lut = 0xDEAD_BEEF_CAFE_BABEu64;
    state.regs.write(Reg::A0, lut);
    // Identity permutation: index i selects byte i
    state.regs.write(Reg::A1, 0x0706_0504_0302_0100);

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

    state.regs.write(Reg::A0, 0x0807_0605_0403_0201);
    // Reverse permutation
    state.regs.write(Reg::A1, 0x0001_0203_0405_0607);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0x0102_0304_0506_0708);
}
