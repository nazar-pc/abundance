use crate::rv32::test_utils::{execute, initialize_state};
use ab_riscv_primitives::instructions::rv32::zk::zbkx::Rv32ZbkxInstruction;
use ab_riscv_primitives::registers::general_purpose::Reg;

// xperm4 tests

#[test]
fn test_xperm4_basic() {
    let mut state = initialize_state([Rv32ZbkxInstruction::Xperm4 {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    // Nibbles 0–7 of rs1: nibble i = i
    state.regs.write(Reg::A0, 0x76543210u32);
    // Identity permutation
    state.regs.write(Reg::A1, 0x76543210u32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0x76543210u32);
}

#[test]
fn test_xperm4_out_of_bounds_zeroed() {
    let mut state = initialize_state([Rv32ZbkxInstruction::Xperm4 {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0x76543210u32);
    // Nibble indices: 0,1,2,3 (in), 8,9,10,11 (out of bounds for RV32)
    state.regs.write(Reg::A1, 0xBA983210u32);

    execute(&mut state).unwrap();

    // Lower nibbles: indices 0–3 -> values 0–3; upper nibbles: indices 8–11 -> 0
    assert_eq!(state.regs.read(Reg::A2), 0x00003210u32);
}

#[test]
fn test_xperm4_constant_index() {
    let mut state = initialize_state([Rv32ZbkxInstruction::Xperm4 {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    // rs1 nibble 2 = 0xC
    state.regs.write(Reg::A0, 0x0000_0C00u32);
    // All 8 nibbles of rs2 are index 2
    state.regs.write(Reg::A1, 0x22222222u32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xCCCCCCCCu32);
}

#[test]
fn test_xperm4_max_in_bounds_index() {
    let mut state = initialize_state([Rv32ZbkxInstruction::Xperm4 {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    // rs1 nibble 7 (highest in-bounds for RV32) = 0xA
    state.regs.write(Reg::A0, 0xA0000000u32);
    // All indices are 7 - the last valid index
    state.regs.write(Reg::A1, 0x77777777u32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xAAAAAAAAu32);
}

#[test]
fn test_xperm4_zero_lut() {
    let mut state = initialize_state([Rv32ZbkxInstruction::Xperm4 {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0x0u32);
    state.regs.write(Reg::A1, 0x76543210u32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0x0u32);
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
    state.regs.write(Reg::A0, 0x40_30_20_10u32);
    // Select bytes 0,1,2,3 in order
    state.regs.write(Reg::A1, 0x03_02_01_00u32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0x40_30_20_10u32);
}

#[test]
fn test_xperm8_out_of_bounds_zeroed() {
    let mut state = initialize_state([Rv32ZbkxInstruction::Xperm8 {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0x04_03_02_01u32);
    // Indices: 0 (in), 4 (out), 255 (out), 0 (in)
    state.regs.write(Reg::A1, 0x00_FF_04_00u32);

    execute(&mut state).unwrap();

    // byte 0: index 0 -> 0x01, byte 1: index 4 -> 0x00, byte 2: index 255 -> 0x00, byte 3: index 0
    // -> 0x01
    assert_eq!(state.regs.read(Reg::A2), 0x01_00_00_01u32);
}

#[test]
fn test_xperm8_identity() {
    let mut state = initialize_state([Rv32ZbkxInstruction::Xperm8 {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    let lut = 0xDE_AD_BE_EFu32;
    state.regs.write(Reg::A0, lut);
    state.regs.write(Reg::A1, 0x03_02_01_00u32);

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

    state.regs.write(Reg::A0, 0x04_03_02_01u32);
    state.regs.write(Reg::A1, 0x00_01_02_03u32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0x01_02_03_04u32);
}
