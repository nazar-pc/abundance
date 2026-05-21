use crate::RegisterFile;
use crate::rv32::test_utils::{execute, initialize_state};
use ab_riscv_primitives::prelude::*;

#[test]
fn test_clmul_simple() {
    let mut state = initialize_state([Rv32ZbcInstruction::Clmul {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0b1010);
    state.regs.write(Reg::A1, 0b1100);

    execute(&mut state).unwrap();

    // Same carryless multiply as RV64 but with 32-bit operands
    // 1010 clmul 1100 = 111_1000
    assert_eq!(state.regs.read(Reg::A2), 0b111_1000);
}

#[test]
fn test_clmul_zero() {
    let mut state = initialize_state([Rv32ZbcInstruction::Clmul {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0xFFFF_FFFF);
    state.regs.write(Reg::A1, 0);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0);
}

#[test]
fn test_clmul_identity() {
    let mut state = initialize_state([Rv32ZbcInstruction::Clmul {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0x1234_5678);
    state.regs.write(Reg::A1, 1);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0x1234_5678);
}

#[test]
fn test_clmulh_zero() {
    let mut state = initialize_state([Rv32ZbcInstruction::Clmulh {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0xFFFF_FFFF);
    state.regs.write(Reg::A1, 0);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0);
}

#[test]
fn test_clmulh_all_ones() {
    let mut state = initialize_state([Rv32ZbcInstruction::Clmulh {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0xFFFF_FFFF);
    state.regs.write(Reg::A1, 0xFFFF_FFFF);

    execute(&mut state).unwrap();

    // clmul(2^32-1, 2^32-1) full 64-bit = 0x5555_5555_AAAA_AAAA
    // clmulh = high 32 bits = 0x5555_5555
    assert_eq!(state.regs.read(Reg::A2), 0x5555_5555);
}

#[test]
fn test_clmulr_simple() {
    let mut state = initialize_state([Rv32ZbcInstruction::Clmulr {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0b1010);
    state.regs.write(Reg::A1, 0b1100);

    execute(&mut state).unwrap();

    // clmul(0b1010, 0b1100) = 0b1111000 (7 bits)
    // clmulr = bits [62:31] = (0b1111000 >> 31) = 0
    assert_eq!(state.regs.read(Reg::A2), 0);
}

#[test]
fn test_clmulr_with_high_bits() {
    let mut state = initialize_state([Rv32ZbcInstruction::Clmulr {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0x8000_0000);
    state.regs.write(Reg::A1, 0x8000_0000);

    execute(&mut state).unwrap();

    // clmul(0x8000_0000, 0x8000_0000) = 1 << 62
    // clmulr = (1 << 62) >> 31 = 1 << 31 = 0x8000_0000
    assert_eq!(state.regs.read(Reg::A2), 0x8000_0000);
}

#[test]
fn test_clmul_combination() {
    let mut state = initialize_state([
        Rv32ZbcInstruction::Clmul {
            rd: Reg::A2,
            rs1: Reg::A0,
            rs2: Reg::A1,
        },
        Rv32ZbcInstruction::Clmulh {
            rd: Reg::A3,
            rs1: Reg::A0,
            rs2: Reg::A1,
        },
    ]);

    state.regs.write(Reg::A0, 0x1234_5678);
    state.regs.write(Reg::A1, 0xABCD_EF01);

    execute(&mut state).unwrap();

    let low = state.regs.read(Reg::A2);
    let high = state.regs.read(Reg::A3);

    // Basic sanity: not both zero unless one operand was zero
    assert!(low != 0 || high != 0);
}
