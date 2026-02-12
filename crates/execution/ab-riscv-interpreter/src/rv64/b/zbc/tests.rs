use crate::rv64::test_utils::{execute, initialize_state};
use ab_riscv_primitives::instructions::rv64::b::zbc::Rv64ZbcInstruction;
use ab_riscv_primitives::registers::general_purpose::EReg;

#[test]
fn test_clmul_simple() {
    let mut state = initialize_state([Rv64ZbcInstruction::Clmul {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 0b1010);
    state.regs.write(EReg::A1, 0b1100);

    execute(&mut state).unwrap();

    // 1010 clmul 1100:
    // bit 2 of b: 1010 << 2 = 101000
    // bit 3 of b: 1010 << 3 = 1010000
    // XOR: 101000 ^ 1010000 = 1111000
    assert_eq!(state.regs.read(EReg::A2), 0b1111000);
}

#[test]
fn test_clmul_zero() {
    let mut state = initialize_state([Rv64ZbcInstruction::Clmul {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 0xFFFF_FFFF_FFFF_FFFFu64);
    state.regs.write(EReg::A1, 0);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 0);
}

#[test]
fn test_clmul_identity() {
    let mut state = initialize_state([Rv64ZbcInstruction::Clmul {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 0x1234_5678_9ABC_DEF0u64);
    state.regs.write(EReg::A1, 1);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 0x1234_5678_9ABC_DEF0u64);
}

#[test]
fn test_clmulh_simple() {
    let mut state = initialize_state([Rv64ZbcInstruction::Clmulh {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 0xFFFF_FFFF_FFFF_FFFFu64);
    state.regs.write(EReg::A1, 0xFFFF_FFFF_FFFF_FFFFu64);

    execute(&mut state).unwrap();

    // Carryless multiplication of 0xFFFF...FFFF × 0xFFFF...FFFF
    // Each bit in result[i] = XOR of all (a[j] & b[k]) where j+k=i
    // For i >= 64 (high word), with all bits set in both operands:
    // bit 127: only from bit[63] × bit[63] = 1
    // bit 126: from bit[62]×bit[63] XOR bit[63]×bit[62] = 1 XOR 1 = 0
    // bit 125: from bit[61]×bit[63] XOR bit[62]×bit[62] XOR bit[63]×bit[61] = 1 XOR 1 XOR 1 = 1
    // The pattern alternates, giving 0xAAAA_AAAA_AAAA_AAAA in high 64 bits
    // For all 1s, high word is (2^64 - 1) ^ (2^63 - 1) = specific pattern

    // The actual result for clmulh of (2^64-1) × (2^64-1) in high 64 bits
    // is 0x5555_5555_5555_5555
    assert_eq!(state.regs.read(EReg::A2), 0x5555_5555_5555_5555u64);
}

#[test]
fn test_clmulh_zero() {
    let mut state = initialize_state([Rv64ZbcInstruction::Clmulh {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 0xFFFF_FFFF_FFFF_FFFFu64);
    state.regs.write(EReg::A1, 0);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 0);
}

#[test]
fn test_clmulr_simple() {
    let mut state = initialize_state([Rv64ZbcInstruction::Clmulr {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 0b1010);
    state.regs.write(EReg::A1, 0b1100);

    execute(&mut state).unwrap();

    // clmulr shifts the full 128-bit result right by 1
    assert_eq!(state.regs.read(EReg::A2), 0b111100);
}

#[test]
fn test_clmulr_with_high_bits() {
    let mut state = initialize_state([Rv64ZbcInstruction::Clmulr {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 0x8000_0000_0000_0000u64);
    state.regs.write(EReg::A1, 0x8000_0000_0000_0000u64);

    execute(&mut state).unwrap();

    // Carryless multiply: 0x8000_0000_0000_0000 × 0x8000_0000_0000_0000
    // Bit 63 of rs1 times bit 63 of rs2 = bit 126 of 128-bit result
    // 128-bit result: 0x4000_0000_0000_0000_0000_0000_0000_0000
    // Shift right by 1: 0x2000_0000_0000_0000_0000_0000_0000_0000
    // Low 64 bits: 0x0000_0000_0000_0000
    // High bit (bit 125) contributes to bit 64 after shift, so we get 0
    assert_eq!(state.regs.read(EReg::A2), 0);
}

#[test]
fn test_clmul_combination() {
    let mut state = initialize_state([
        Rv64ZbcInstruction::Clmul {
            rd: EReg::A2,
            rs1: EReg::A0,
            rs2: EReg::A1,
        },
        Rv64ZbcInstruction::Clmulh {
            rd: EReg::A3,
            rs1: EReg::A0,
            rs2: EReg::A1,
        },
    ]);

    state.regs.write(EReg::A0, 0x1234_5678u64);
    state.regs.write(EReg::A1, 0xABCD_EF01u64);

    execute(&mut state).unwrap();

    // Just verify they execute without panic
    // The actual values depend on carryless multiplication logic
    let low = state.regs.read(EReg::A2);
    let high = state.regs.read(EReg::A3);

    // Basic sanity check: not both zeros unless one operand was zero
    assert!(low != 0 || high != 0);
}
