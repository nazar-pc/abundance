use crate::RegisterFile;
use crate::rv32::test_utils::{execute, initialize_state};
use ab_riscv_primitives::prelude::*;

#[test]
fn test_bset_basic() {
    let mut state = initialize_state([Rv32ZbsInstruction::Bset {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0b1000u32);
    state.regs.write(Reg::A1, 2u32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0b1100);
}

#[test]
fn test_bseti_basic() {
    let mut state = initialize_state([Rv32ZbsInstruction::Bseti {
        rd: Reg::A2,
        rs1: Reg::A0,
        shamt: 0,
        rs2: Reg::Zero,
    }]);

    state.regs.write(Reg::A0, 0b1000u32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0b1001);
}

#[test]
fn test_bset_high_bit() {
    let mut state = initialize_state([Rv32ZbsInstruction::Bset {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0u32);
    // Set bit 31 (highest in RV32)
    state.regs.write(Reg::A1, 31u32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0x8000_0000u32);
}

#[test]
fn test_bclr_basic() {
    let mut state = initialize_state([Rv32ZbsInstruction::Bclr {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0b1111u32);
    state.regs.write(Reg::A1, 2u32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0b1011);
}

#[test]
fn test_bclri_basic() {
    let mut state = initialize_state([Rv32ZbsInstruction::Bclri {
        rd: Reg::A2,
        rs1: Reg::A0,
        shamt: 0,
        rs2: Reg::Zero,
    }]);

    state.regs.write(Reg::A0, 0xFFFF_FFFFu32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xFFFF_FFFEu32);
}

#[test]
fn test_bclr_high_bit() {
    let mut state = initialize_state([Rv32ZbsInstruction::Bclr {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0xFFFF_FFFFu32);
    state.regs.write(Reg::A1, 31u32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0x7FFF_FFFFu32);
}

#[test]
fn test_binv_basic() {
    let mut state = initialize_state([Rv32ZbsInstruction::Binv {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0b1010u32);
    state.regs.write(Reg::A1, 1u32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0b1000);
}

#[test]
fn test_binvi_basic() {
    let mut state = initialize_state([Rv32ZbsInstruction::Binvi {
        rd: Reg::A2,
        rs1: Reg::A0,
        shamt: 0,
        rs2: Reg::Zero,
    }]);

    state.regs.write(Reg::A0, 0b1010u32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0b1011);
}

#[test]
fn test_binv_twice() {
    let mut state = initialize_state([
        Rv32ZbsInstruction::Binv {
            rd: Reg::A2,
            rs1: Reg::A0,
            rs2: Reg::A1,
        },
        Rv32ZbsInstruction::Binv {
            rd: Reg::A3,
            rs1: Reg::A2,
            rs2: Reg::A1,
        },
    ]);

    state.regs.write(Reg::A0, 0b1010u32);
    state.regs.write(Reg::A1, 2u32);

    execute(&mut state).unwrap();

    // Inverting twice should give the original value
    assert_eq!(state.regs.read(Reg::A3), 0b1010);
}

#[test]
fn test_bext_basic() {
    let mut state = initialize_state([Rv32ZbsInstruction::Bext {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0b1010u32);
    state.regs.write(Reg::A1, 1u32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 1);
}

#[test]
fn test_bexti_basic() {
    let mut state = initialize_state([Rv32ZbsInstruction::Bexti {
        rd: Reg::A2,
        rs1: Reg::A0,
        shamt: 2,
        rs2: Reg::Zero,
    }]);

    state.regs.write(Reg::A0, 0b1010u32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0);
}

#[test]
fn test_bext_high_bit() {
    let mut state = initialize_state([Rv32ZbsInstruction::Bext {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0x8000_0000u32);
    state.regs.write(Reg::A1, 31u32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 1);
}

#[test]
fn test_bext_zero_bit() {
    let mut state = initialize_state([Rv32ZbsInstruction::Bext {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0x7FFF_FFFFu32);
    state.regs.write(Reg::A1, 31u32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0);
}

#[test]
fn test_combination() {
    let mut state = initialize_state([
        // Set bit 5
        Rv32ZbsInstruction::Bset {
            rd: Reg::A2,
            rs1: Reg::A0,
            rs2: Reg::A1,
        },
        // Set bit 10
        Rv32ZbsInstruction::Bseti {
            rd: Reg::A3,
            rs1: Reg::A2,
            shamt: 10,
            rs2: Reg::Zero,
        },
        // Extract bit 5
        Rv32ZbsInstruction::Bext {
            rd: Reg::A4,
            rs1: Reg::A3,
            rs2: Reg::A1,
        },
        // Clear bit 5
        Rv32ZbsInstruction::Bclr {
            rd: Reg::A5,
            rs1: Reg::A3,
            rs2: Reg::A1,
        },
    ]);

    state.regs.write(Reg::A0, 0u32);
    state.regs.write(Reg::A1, 5u32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0b10_0000u32);
    assert_eq!(state.regs.read(Reg::A3), 0b100_0010_0000u32);
    assert_eq!(state.regs.read(Reg::A4), 1u32);
    assert_eq!(state.regs.read(Reg::A5), 0b100_0000_0000u32);
}
