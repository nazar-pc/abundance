use crate::rv32::test_utils::{execute, initialize_state};
use ab_riscv_primitives::prelude::*;

// SHA-256 (single-register)

#[test]
fn test_sha256_sig0_simple() {
    let mut state = initialize_state([Rv32ZknhInstruction::Sha256Sig0 {
        rd: Reg::A2,
        rs1: Reg::A0,
    }]);
    state.regs.write(Reg::A0, 0x1234_5678u32);
    execute(&mut state).unwrap();
    let x = 0x1234_5678u32;
    assert_eq!(
        state.regs.read(Reg::A2),
        x.rotate_right(7) ^ x.rotate_right(18) ^ (x >> 3)
    );
}

#[test]
fn test_sha256_sig0_zero() {
    let mut state = initialize_state([Rv32ZknhInstruction::Sha256Sig0 {
        rd: Reg::A2,
        rs1: Reg::A0,
    }]);
    state.regs.write(Reg::A0, 0u32);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), 0u32);
}

#[test]
fn test_sha256_sig0_all_ones() {
    let mut state = initialize_state([Rv32ZknhInstruction::Sha256Sig0 {
        rd: Reg::A2,
        rs1: Reg::A0,
    }]);
    state.regs.write(Reg::A0, u32::MAX);
    execute(&mut state).unwrap();
    let x = u32::MAX;
    assert_eq!(
        state.regs.read(Reg::A2),
        x.rotate_right(7) ^ x.rotate_right(18) ^ (x >> 3)
    );
}

#[test]
fn test_sha256_sig1_simple() {
    let mut state = initialize_state([Rv32ZknhInstruction::Sha256Sig1 {
        rd: Reg::A2,
        rs1: Reg::A0,
    }]);
    state.regs.write(Reg::A0, 0x1234_5678u32);
    execute(&mut state).unwrap();
    let x = 0x1234_5678u32;
    assert_eq!(
        state.regs.read(Reg::A2),
        x.rotate_right(17) ^ x.rotate_right(19) ^ (x >> 10)
    );
}

#[test]
fn test_sha256_sum0_simple() {
    let mut state = initialize_state([Rv32ZknhInstruction::Sha256Sum0 {
        rd: Reg::A2,
        rs1: Reg::A0,
    }]);
    state.regs.write(Reg::A0, 0xdead_beefu32);
    execute(&mut state).unwrap();
    let x = 0xdead_beefu32;
    assert_eq!(
        state.regs.read(Reg::A2),
        x.rotate_right(2) ^ x.rotate_right(13) ^ x.rotate_right(22)
    );
}

#[test]
fn test_sha256_sum1_simple() {
    let mut state = initialize_state([Rv32ZknhInstruction::Sha256Sum1 {
        rd: Reg::A2,
        rs1: Reg::A0,
    }]);
    state.regs.write(Reg::A0, 0xdead_beefu32);
    execute(&mut state).unwrap();
    let x = 0xdead_beefu32;
    assert_eq!(
        state.regs.read(Reg::A2),
        x.rotate_right(6) ^ x.rotate_right(11) ^ x.rotate_right(25)
    );
}

// Reference functions for SHA-512 tests.
//
// Register conventions (from the RISC-V scalar crypto Sail model):
//
//   sha512sig0l, sha512sig1l: rs1 = LOW word, rs2 = HIGH word
//   sha512sig0h, sha512sig1h: rs1 = HIGH word, rs2 = LOW word
//   sha512sum0r, sha512sum1r: rs1 = LOW word, rs2 = HIGH word
//
// For sum0r and sum1r the Sail pseudocode builds the 64-bit operand as
//   x[63:32] = X(rs2),  x[31:0] = X(rs1)
// and writes x[31:0] of the result to rd.
//
// Each function takes (rs1_value, rs2_value) exactly as the executor reads them.

fn ref_sha512sig0h(rs1: u32, rs2: u32) -> u32 {
    // rs1=HIGH, rs2=LOW
    (rs1 >> 1) ^ (rs2 << 31) ^ (rs1 >> 8) ^ (rs2 << 24) ^ (rs1 >> 7)
}

fn ref_sha512sig0l(rs1: u32, rs2: u32) -> u32 {
    // rs1=LOW, rs2=HIGH; extra (rs2<<25) term from SHR64(x,7).lo cross-boundary bits
    (rs1 >> 1) ^ (rs2 << 31) ^ (rs1 >> 8) ^ (rs2 << 24) ^ (rs1 >> 7) ^ (rs2 << 25)
}

fn ref_sha512sig1h(rs1: u32, rs2: u32) -> u32 {
    // rs1=HIGH, rs2=LOW
    (rs1 >> 19) ^ (rs2 << 13) ^ (rs2 >> 29) ^ (rs1 << 3) ^ (rs1 >> 6)
}

fn ref_sha512sig1l(rs1: u32, rs2: u32) -> u32 {
    // rs1=LOW, rs2=HIGH; extra (rs2<<26) term from SHR64(x,6).lo cross-boundary bits
    (rs1 >> 19) ^ (rs2 << 13) ^ (rs2 >> 29) ^ (rs1 << 3) ^ (rs1 >> 6) ^ (rs2 << 26)
}

fn ref_sha512sum0r(rs1: u32, rs2: u32) -> u32 {
    // rs1=LOW, rs2=HIGH: input = {rs2, rs1}
    // ROR64(x,28).lo = (rs1>>28)|(rs2<<4)
    // ROR64(x,34).lo = (rs2>>2) |(rs1<<30)
    // ROR64(x,39).lo = (rs2>>7) |(rs1<<25)
    (rs1 >> 28) ^ (rs2 << 4) ^ (rs2 >> 2) ^ (rs1 << 30) ^ (rs2 >> 7) ^ (rs1 << 25)
}

fn ref_sha512sum1r(rs1: u32, rs2: u32) -> u32 {
    // rs1=LOW, rs2=HIGH: input = {rs2, rs1}
    // ROR64(x,14).lo = (rs1>>14)|(rs2<<18)
    // ROR64(x,18).lo = (rs1>>18)|(rs2<<14)
    // ROR64(x,41).lo = (rs2>>9) |(rs1<<23)
    (rs1 >> 14) ^ (rs2 << 18) ^ (rs1 >> 18) ^ (rs2 << 14) ^ (rs2 >> 9) ^ (rs1 << 23)
}

// sha512sig0h  (rs1=HIGH, rs2=LOW)

#[test]
fn test_sha512sig0h_simple() {
    // 64-bit value 0x1234567890abcdef: hi=0x12345678  lo=0x90abcdef
    let hi = 0x1234_5678u32;
    let lo = 0x90ab_cdefu32;
    let mut state = initialize_state([Rv32ZknhInstruction::Sha512Sig0h {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A0, hi);
    state.regs.write(Reg::A1, lo);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), ref_sha512sig0h(hi, lo));
    // Concrete value cross-checked against u64 formula
    assert_eq!(state.regs.read(Reg::A2), 0x662c_77c6u32);
}

#[test]
fn test_sha512sig0h_rs2_zero() {
    let hi = 0x5bbc_8872u32;
    let lo = 0u32;
    let mut state = initialize_state([Rv32ZknhInstruction::Sha512Sig0h {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A0, hi);
    state.regs.write(Reg::A1, lo);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), ref_sha512sig0h(hi, lo));
}

#[test]
fn test_sha512sig0h_rs1_zero() {
    let hi = 0u32;
    let lo = 0xffff_ffffu32;
    let mut state = initialize_state([Rv32ZknhInstruction::Sha512Sig0h {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A0, hi);
    state.regs.write(Reg::A1, lo);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), ref_sha512sig0h(hi, lo));
}

#[test]
fn test_sha512sig0h_both_zero() {
    let mut state = initialize_state([Rv32ZknhInstruction::Sha512Sig0h {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A0, 0u32);
    state.regs.write(Reg::A1, 0u32);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), 0u32);
}

// sha512sig0l  (rs1=LOW, rs2=HIGH)

#[test]
fn test_sha512sig0l_simple() {
    // 64-bit value 0x1234567890abcdef: hi=0x12345678  lo=0x90abcdef
    let lo = 0x90ab_cdefu32;
    let hi = 0x1234_5678u32;
    let mut state = initialize_state([Rv32ZknhInstruction::Sha512Sig0l {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A0, lo);
    state.regs.write(Reg::A1, hi);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), ref_sha512sig0l(lo, hi));
    // Concrete value cross-checked against u64 formula
    assert_eq!(state.regs.read(Reg::A2), 0xc1e4_1aa1u32);
}

#[test]
fn test_sha512sig0l_rs1_zero() {
    let lo = 0u32;
    let hi = 0x5bbc_8872u32;
    let mut state = initialize_state([Rv32ZknhInstruction::Sha512Sig0l {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A0, lo);
    state.regs.write(Reg::A1, hi);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), ref_sha512sig0l(lo, hi));
}

#[test]
fn test_sha512sig0l_both_zero() {
    let mut state = initialize_state([Rv32ZknhInstruction::Sha512Sig0l {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A0, 0u32);
    state.regs.write(Reg::A1, 0u32);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), 0u32);
}

// Consistency: sig0h and sig0l together reconstruct the full u64 result
#[test]
fn test_sha512sig0_consistent_with_u64() {
    let val: u64 = 0x1234_5678_90ab_cdef;
    let lo = val as u32;
    let hi = (val >> 32) as u32;
    let u64_result = val.rotate_right(1) ^ val.rotate_right(8) ^ (val >> 7);
    assert_eq!(ref_sha512sig0h(hi, lo), (u64_result >> 32) as u32);
    assert_eq!(ref_sha512sig0l(lo, hi), u64_result as u32);
}

// sha512sig1h  (rs1=HIGH, rs2=LOW)

#[test]
fn test_sha512sig1h_simple() {
    // 64-bit value 0x1234567890abcdef: hi=0x12345678  lo=0x90abcdef
    let hi = 0x1234_5678u32;
    let lo = 0x90ab_cdefu32;
    let mut state = initialize_state([Rv32ZknhInstruction::Sha512Sig1h {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A0, hi);
    state.regs.write(Reg::A1, lo);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), ref_sha512sig1h(hi, lo));
    assert_eq!(state.regs.read(Reg::A2), 0xe857_80dbu32);
}

#[test]
fn test_sha512sig1h_rs2_zero() {
    let hi = 0xaaaa_aaaau32;
    let lo = 0u32;
    let mut state = initialize_state([Rv32ZknhInstruction::Sha512Sig1h {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A0, hi);
    state.regs.write(Reg::A1, lo);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), ref_sha512sig1h(hi, lo));
}

#[test]
fn test_sha512sig1h_both_zero() {
    let mut state = initialize_state([Rv32ZknhInstruction::Sha512Sig1h {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A0, 0u32);
    state.regs.write(Reg::A1, 0u32);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), 0u32);
}

// sha512sig1l  (rs1=LOW, rs2=HIGH)

#[test]
fn test_sha512sig1l_simple() {
    // 64-bit value 0x1234567890abcdef: hi=0x12345678  lo=0x90abcdef
    let lo = 0x90ab_cdefu32;
    let hi = 0x1234_5678u32;
    let mut state = initialize_state([Rv32ZknhInstruction::Sha512Sig1l {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A0, lo);
    state.regs.write(Reg::A1, hi);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), ref_sha512sig1l(lo, hi));
    assert_eq!(state.regs.read(Reg::A2), 0xedd3_d25au32);
}

#[test]
fn test_sha512sig1l_rs1_zero() {
    let lo = 0u32;
    let hi = 0x5555_5555u32;
    let mut state = initialize_state([Rv32ZknhInstruction::Sha512Sig1l {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A0, lo);
    state.regs.write(Reg::A1, hi);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), ref_sha512sig1l(lo, hi));
}

#[test]
fn test_sha512sig1l_both_zero() {
    let mut state = initialize_state([Rv32ZknhInstruction::Sha512Sig1l {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A0, 0u32);
    state.regs.write(Reg::A1, 0u32);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), 0u32);
}

// Consistency: sig1h and sig1l together reconstruct the full u64 result
#[test]
fn test_sha512sig1_consistent_with_u64() {
    let val: u64 = 0xfedc_ba98_7654_3210;
    let lo = val as u32;
    let hi = (val >> 32) as u32;
    let u64_result = val.rotate_right(19) ^ val.rotate_right(61) ^ (val >> 6);
    assert_eq!(ref_sha512sig1h(hi, lo), (u64_result >> 32) as u32);
    assert_eq!(ref_sha512sig1l(lo, hi), u64_result as u32);
}

// sha512sum0r (rs1=LOW, rs2=HIGH)
//
// The Sail pseudocode builds x[63:32]=rs2, x[31:0]=rs1 and writes x[31:0] of the
// ROR64(x,28)^ROR64(x,34)^ROR64(x,39) result to rd.

#[test]
fn test_sha512sum0r_simple() {
    // 64-bit value 0x1234567890abcdef: hi=0x12345678 (HIGH=rs2), lo=0x90abcdef (LOW=rs1)
    let lo = 0x90ab_cdefu32;
    let hi = 0x1234_5678u32;
    let mut state = initialize_state([Rv32ZknhInstruction::Sha512Sum0r {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    // rs1 holds the LOW word, rs2 holds the HIGH word
    state.regs.write(Reg::A0, lo);
    state.regs.write(Reg::A1, hi);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), ref_sha512sum0r(lo, hi));
    // Concrete value cross-checked against u64 formula
    assert_eq!(state.regs.read(Reg::A2), 0x39ec_1abbu32);
}

#[test]
fn test_sha512sum0r_consistent_with_u64() {
    // With rs1=lo (LOW) and rs2=hi (HIGH), input={rs2,rs1}={hi,lo}=val exactly
    let val: u64 = 0x1234_5678_90ab_cdef;
    let lo = val as u32;
    let hi = (val >> 32) as u32;
    let expected_lo = (val.rotate_right(28) ^ val.rotate_right(34) ^ val.rotate_right(39)) as u32;
    let mut state = initialize_state([Rv32ZknhInstruction::Sha512Sum0r {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    // rs1=lo (LOW operand), rs2=hi (HIGH operand)
    state.regs.write(Reg::A0, lo);
    state.regs.write(Reg::A1, hi);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), expected_lo);
}

#[test]
fn test_sha512sum0r_both_zero() {
    let mut state = initialize_state([Rv32ZknhInstruction::Sha512Sum0r {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A0, 0u32);
    state.regs.write(Reg::A1, 0u32);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), 0u32);
}

// sha512sum1r (rs1=LOW, rs2=HIGH)
//
// The Sail pseudocode builds x[63:32]=rs2, x[31:0]=rs1 and writes x[31:0] of the
// ROR64(x,14)^ROR64(x,18)^ROR64(x,41) result to rd.

#[test]
fn test_sha512sum1r_simple() {
    // 64-bit value 0x1234567890abcdef: hi=0x12345678 (HIGH=rs2), lo=0x90abcdef (LOW=rs1)
    let lo = 0x90ab_cdefu32;
    let hi = 0x1234_5678u32;
    let mut state = initialize_state([Rv32ZknhInstruction::Sha512Sum1r {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    // rs1 holds the LOW word, rs2 holds the HIGH word
    state.regs.write(Reg::A0, lo);
    state.regs.write(Reg::A1, hi);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), ref_sha512sum1r(lo, hi));
    // Concrete value cross-checked against u64 formula
    assert_eq!(state.regs.read(Reg::A2), 0xbbf5_7caeu32);
}

#[test]
fn test_sha512sum1r_consistent_with_u64() {
    // With rs1=lo (LOW) and rs2=hi (HIGH), input={rs2,rs1}={hi,lo}=val exactly
    let val: u64 = 0x1234_5678_90ab_cdef;
    let lo = val as u32;
    let hi = (val >> 32) as u32;
    let expected_lo = (val.rotate_right(14) ^ val.rotate_right(18) ^ val.rotate_right(41)) as u32;
    let mut state = initialize_state([Rv32ZknhInstruction::Sha512Sum1r {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    // rs1=lo (LOW operand), rs2=hi (HIGH operand)
    state.regs.write(Reg::A0, lo);
    state.regs.write(Reg::A1, hi);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), expected_lo);
}

#[test]
fn test_sha512sum1r_both_zero() {
    let mut state = initialize_state([Rv32ZknhInstruction::Sha512Sum1r {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A0, 0u32);
    state.regs.write(Reg::A1, 0u32);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), 0u32);
}

// Edge-value matrix. rs1/rs2 loaded in the correct per-instruction order.

const EDGE_VALUES: &[u32] = &[
    0x0000_0000,
    0xffff_ffff,
    0x8000_0000,
    0x7fff_ffff,
    0x0000_0001,
    0x5bbc_8872,
    0xaaaa_aaaa,
    0x5555_5555,
];

#[test]
fn test_sha512sig0h_edge_matrix() {
    for &rs1 in EDGE_VALUES {
        for &rs2 in EDGE_VALUES {
            let mut state = initialize_state([Rv32ZknhInstruction::Sha512Sig0h {
                rd: Reg::A2,
                rs1: Reg::A0,
                rs2: Reg::A1,
            }]);
            state.regs.write(Reg::A0, rs1);
            state.regs.write(Reg::A1, rs2);
            execute(&mut state).unwrap();
            assert_eq!(
                state.regs.read(Reg::A2),
                ref_sha512sig0h(rs1, rs2),
                "sha512sig0h(rs1={rs1:#010x}, rs2={rs2:#010x})"
            );
        }
    }
}

#[test]
fn test_sha512sig0l_edge_matrix() {
    for &rs1 in EDGE_VALUES {
        for &rs2 in EDGE_VALUES {
            let mut state = initialize_state([Rv32ZknhInstruction::Sha512Sig0l {
                rd: Reg::A2,
                rs1: Reg::A0,
                rs2: Reg::A1,
            }]);
            state.regs.write(Reg::A0, rs1);
            state.regs.write(Reg::A1, rs2);
            execute(&mut state).unwrap();
            assert_eq!(
                state.regs.read(Reg::A2),
                ref_sha512sig0l(rs1, rs2),
                "sha512sig0l(rs1={rs1:#010x}, rs2={rs2:#010x})"
            );
        }
    }
}

#[test]
fn test_sha512sig1h_edge_matrix() {
    for &rs1 in EDGE_VALUES {
        for &rs2 in EDGE_VALUES {
            let mut state = initialize_state([Rv32ZknhInstruction::Sha512Sig1h {
                rd: Reg::A2,
                rs1: Reg::A0,
                rs2: Reg::A1,
            }]);
            state.regs.write(Reg::A0, rs1);
            state.regs.write(Reg::A1, rs2);
            execute(&mut state).unwrap();
            assert_eq!(
                state.regs.read(Reg::A2),
                ref_sha512sig1h(rs1, rs2),
                "sha512sig1h(rs1={rs1:#010x}, rs2={rs2:#010x})"
            );
        }
    }
}

#[test]
fn test_sha512sig1l_edge_matrix() {
    for &rs1 in EDGE_VALUES {
        for &rs2 in EDGE_VALUES {
            let mut state = initialize_state([Rv32ZknhInstruction::Sha512Sig1l {
                rd: Reg::A2,
                rs1: Reg::A0,
                rs2: Reg::A1,
            }]);
            state.regs.write(Reg::A0, rs1);
            state.regs.write(Reg::A1, rs2);
            execute(&mut state).unwrap();
            assert_eq!(
                state.regs.read(Reg::A2),
                ref_sha512sig1l(rs1, rs2),
                "sha512sig1l(rs1={rs1:#010x}, rs2={rs2:#010x})"
            );
        }
    }
}

#[test]
fn test_sha512sum0r_edge_matrix() {
    for &rs1 in EDGE_VALUES {
        for &rs2 in EDGE_VALUES {
            let mut state = initialize_state([Rv32ZknhInstruction::Sha512Sum0r {
                rd: Reg::A2,
                rs1: Reg::A0,
                rs2: Reg::A1,
            }]);
            state.regs.write(Reg::A0, rs1);
            state.regs.write(Reg::A1, rs2);
            execute(&mut state).unwrap();
            assert_eq!(
                state.regs.read(Reg::A2),
                ref_sha512sum0r(rs1, rs2),
                "sha512sum0r(rs1={rs1:#010x}, rs2={rs2:#010x})"
            );
        }
    }
}

#[test]
fn test_sha512sum1r_edge_matrix() {
    for &rs1 in EDGE_VALUES {
        for &rs2 in EDGE_VALUES {
            let mut state = initialize_state([Rv32ZknhInstruction::Sha512Sum1r {
                rd: Reg::A2,
                rs1: Reg::A0,
                rs2: Reg::A1,
            }]);
            state.regs.write(Reg::A0, rs1);
            state.regs.write(Reg::A1, rs2);
            execute(&mut state).unwrap();
            assert_eq!(
                state.regs.read(Reg::A2),
                ref_sha512sum1r(rs1, rs2),
                "sha512sum1r(rs1={rs1:#010x}, rs2={rs2:#010x})"
            );
        }
    }
}

// Combination test

#[test]
fn test_zknh_combination() {
    let sha256_input = 0x1234_5678u32;
    // 64-bit value 0x1234567890abcdef: hi=0x12345678 (HIGH), lo=0x90abcdef (LOW)
    let sha512_lo = 0x90ab_cdefu32;
    let sha512_hi = 0x1234_5678u32;

    let mut state = initialize_state([
        Rv32ZknhInstruction::Sha256Sum0 {
            rd: Reg::A2,
            rs1: Reg::A0,
        },
        // sum1r: rs1=LOW, rs2=HIGH
        Rv32ZknhInstruction::Sha512Sum1r {
            rd: Reg::A3,
            rs1: Reg::A4,
            rs2: Reg::A5,
        },
    ]);

    state.regs.write(Reg::A0, sha256_input);
    // A4=rs1=LOW word, A5=rs2=HIGH word
    state.regs.write(Reg::A4, sha512_lo);
    state.regs.write(Reg::A5, sha512_hi);

    execute(&mut state).unwrap();

    let expected_sha256 = sha256_input.rotate_right(2)
        ^ sha256_input.rotate_right(13)
        ^ sha256_input.rotate_right(22);
    let expected_sha512 = ref_sha512sum1r(sha512_lo, sha512_hi);

    assert_eq!(state.regs.read(Reg::A2), expected_sha256);
    assert_eq!(state.regs.read(Reg::A3), expected_sha512);
}
