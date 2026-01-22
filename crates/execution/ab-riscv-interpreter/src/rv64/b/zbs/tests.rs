use crate::rv64::test_utils::{execute, initialize_state};
use ab_riscv_primitives::instruction::rv64::b::zbs::Rv64ZbsInstruction;
use ab_riscv_primitives::registers::EReg;

#[test]
fn test_bset_basic() {
    let mut state = initialize_state([Rv64ZbsInstruction::Bset {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 0b1000);
    // Set bit 2
    state.regs.write(EReg::A1, 2);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 0b1100);
}

#[test]
fn test_bseti_basic() {
    let mut state = initialize_state([Rv64ZbsInstruction::Bseti {
        rd: EReg::A2,
        rs1: EReg::A0,
        shamt: 0,
    }]);

    state.regs.write(EReg::A0, 0b1000);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 0b1001);
}

#[test]
fn test_bset_high_bit() {
    let mut state = initialize_state([Rv64ZbsInstruction::Bset {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 0);
    // Set bit 63
    state.regs.write(EReg::A1, 63);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 0x8000_0000_0000_0000u64);
}

#[test]
fn test_bclr_basic() {
    let mut state = initialize_state([Rv64ZbsInstruction::Bclr {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 0b1111);
    // Clear bit 2
    state.regs.write(EReg::A1, 2);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 0b1011);
}

#[test]
fn test_bclri_basic() {
    let mut state = initialize_state([Rv64ZbsInstruction::Bclri {
        rd: EReg::A2,
        rs1: EReg::A0,
        shamt: 0,
    }]);

    state.regs.write(EReg::A0, 0xFFFF_FFFF_FFFF_FFFFu64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 0xFFFF_FFFF_FFFF_FFFEu64);
}

#[test]
fn test_bclr_high_bit() {
    let mut state = initialize_state([Rv64ZbsInstruction::Bclr {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 0xFFFF_FFFF_FFFF_FFFFu64);
    // Clear bit 63
    state.regs.write(EReg::A1, 63);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 0x7FFF_FFFF_FFFF_FFFFu64);
}

#[test]
fn test_binv_basic() {
    let mut state = initialize_state([Rv64ZbsInstruction::Binv {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 0b1010);
    // Invert bit 1
    state.regs.write(EReg::A1, 1);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 0b1000);
}

#[test]
fn test_binvi_basic() {
    let mut state = initialize_state([Rv64ZbsInstruction::Binvi {
        rd: EReg::A2,
        rs1: EReg::A0,
        shamt: 0,
    }]);

    state.regs.write(EReg::A0, 0b1010);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 0b1011);
}

#[test]
fn test_binv_twice() {
    let mut state = initialize_state([
        Rv64ZbsInstruction::Binv {
            rd: EReg::A2,
            rs1: EReg::A0,
            rs2: EReg::A1,
        },
        Rv64ZbsInstruction::Binv {
            rd: EReg::A3,
            rs1: EReg::A2,
            rs2: EReg::A1,
        },
    ]);

    state.regs.write(EReg::A0, 0b1010);
    state.regs.write(EReg::A1, 2);

    execute(&mut state).unwrap();

    // Inverting twice should give the original value
    assert_eq!(state.regs.read(EReg::A3), 0b1010);
}

#[test]
fn test_bext_basic() {
    let mut state = initialize_state([Rv64ZbsInstruction::Bext {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 0b1010);
    // Extract bit 1
    state.regs.write(EReg::A1, 1);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 1);
}

#[test]
fn test_bexti_basic() {
    let mut state = initialize_state([Rv64ZbsInstruction::Bexti {
        rd: EReg::A2,
        rs1: EReg::A0,
        shamt: 2,
    }]);

    state.regs.write(EReg::A0, 0b1010);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 0);
}

#[test]
fn test_bext_high_bit() {
    let mut state = initialize_state([Rv64ZbsInstruction::Bext {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 0x8000_0000_0000_0000u64);
    // Extract bit 63
    state.regs.write(EReg::A1, 63);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 1);
}

#[test]
fn test_bext_zero_bit() {
    let mut state = initialize_state([Rv64ZbsInstruction::Bext {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 0x7FFF_FFFF_FFFF_FFFFu64);
    // Extract bit 63 (which is 0)
    state.regs.write(EReg::A1, 63);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 0);
}

#[test]
fn test_combination() {
    let mut state = initialize_state([
        // Set bit 5
        Rv64ZbsInstruction::Bset {
            rd: EReg::A2,
            rs1: EReg::A0,
            rs2: EReg::A1,
        },
        // Set bit 10
        Rv64ZbsInstruction::Bseti {
            rd: EReg::A3,
            rs1: EReg::A2,
            shamt: 10,
        },
        // Extract bit 5
        Rv64ZbsInstruction::Bext {
            rd: EReg::A4,
            rs1: EReg::A3,
            rs2: EReg::A1,
        },
        // Clear bit 5
        Rv64ZbsInstruction::Bclr {
            rd: EReg::A5,
            rs1: EReg::A3,
            rs2: EReg::A1,
        },
    ]);

    state.regs.write(EReg::A0, 0);
    state.regs.write(EReg::A1, 5);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 0b10_0000);
    assert_eq!(state.regs.read(EReg::A3), 0b100_0010_0000);
    // bit 5 was set
    assert_eq!(state.regs.read(EReg::A4), 1);
    // bit 5 cleared
    assert_eq!(state.regs.read(EReg::A5), 0b100_0000_0000);
}
