use crate::rv64::test_utils::{execute, initialize_state};
use ab_riscv_primitives::instructions::rv64::zk::zkn::zknh::Rv64ZknhInstruction;
use ab_riscv_primitives::registers::EReg;

#[test]
fn test_sha256_sig0_simple() {
    let mut state = initialize_state([Rv64ZknhInstruction::Sha256Sig0 {
        rd: EReg::A2,
        rs1: EReg::A0,
    }]);

    state.regs.write(EReg::A0, 0x1234_5678u64);

    execute(&mut state).unwrap();

    // sha256sig0 on lower 32 bits 0x12345678 → 0xe7fce6ee (bit 31 set → sign-extended)
    assert_eq!(state.regs.read(EReg::A2), 0xffff_ffff_e7fc_e6eeu64);
}

#[test]
fn test_sha256_sig0_zero() {
    let mut state = initialize_state([Rv64ZknhInstruction::Sha256Sig0 {
        rd: EReg::A2,
        rs1: EReg::A0,
    }]);

    state.regs.write(EReg::A0, 0u64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 0u64);
}

#[test]
fn test_sha256_sig0_high_bits_ignored() {
    let mut state = initialize_state([Rv64ZknhInstruction::Sha256Sig0 {
        rd: EReg::A2,
        rs1: EReg::A0,
    }]);

    state.regs.write(EReg::A0, 0xabcdef01_12345678u64);

    execute(&mut state).unwrap();

    // Same result as previous test (high 32 bits ignored)
    assert_eq!(state.regs.read(EReg::A2), 0xffff_ffff_e7fc_e6eeu64);
}

#[test]
fn test_sha256_sig1_sign_extend() {
    let mut state = initialize_state([Rv64ZknhInstruction::Sha256Sig1 {
        rd: EReg::A2,
        rs1: EReg::A0,
    }]);

    state.regs.write(EReg::A0, 0x1234_5678u64);

    execute(&mut state).unwrap();

    // sha256sig1 on 0x12345678 → 0xa1f78649 (bit 31 set → sign-extended)
    assert_eq!(state.regs.read(EReg::A2), 0xffff_ffff_a1f7_8649u64);
}

#[test]
fn test_sha512_sig0_simple() {
    let mut state = initialize_state([Rv64ZknhInstruction::Sha512Sig0 {
        rd: EReg::A2,
        rs1: EReg::A0,
    }]);

    state.regs.write(EReg::A0, 0x1234_5678_90ab_cdefu64);

    execute(&mut state).unwrap();

    // sha512sig0(0x1234567890abcdef) → 0x662c77c6c1e41aa1
    assert_eq!(state.regs.read(EReg::A2), 0x662c_77c6_c1e4_1aa1u64);
}

#[test]
fn test_sha512_sig1_zero() {
    let mut state = initialize_state([Rv64ZknhInstruction::Sha512Sig1 {
        rd: EReg::A2,
        rs1: EReg::A0,
    }]);

    state.regs.write(EReg::A0, 0u64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 0u64);
}

#[test]
fn test_zknh_combination() {
    let mut state = initialize_state([
        Rv64ZknhInstruction::Sha256Sum0 {
            rd: EReg::A2,
            rs1: EReg::A0,
        },
        Rv64ZknhInstruction::Sha512Sum1 {
            rd: EReg::A3,
            rs1: EReg::A1,
        },
    ]);

    state.regs.write(EReg::A0, 0x1234_5678u64);
    state.regs.write(EReg::A1, 0x1234_5678_90ab_cdefu64);

    execute(&mut state).unwrap();

    // Sanity check: results are non-zero
    let sha256_res = state.regs.read(EReg::A2);
    let sha512_res = state.regs.read(EReg::A3);
    assert_ne!(sha256_res, 0);
    assert_ne!(sha512_res, 0);
}
