use crate::RegisterFile;
use crate::rv64::test_utils::{execute, initialize_state};
use ab_riscv_primitives::prelude::*;
// aes64im - self-contained, no cross-instruction dependency

// TODO: `llvm.aarch64.crypto.aes*` is not supported in Miri yet:
//  https://github.com/rust-lang/miri/issues/3172#issuecomment-3730602707
#[cfg(not(all(miri, target_arch = "aarch64")))]
#[test]
fn test_aes64im_zero() {
    let mut state = initialize_state([Rv64ZkndInstruction::Aes64Im {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, 0);
    execute(&mut state).unwrap();
    // InvMixColumns(0) = 0
    assert_eq!(state.regs.read(Reg::A2), 0);
}

// TODO: `llvm.aarch64.crypto.aes*` is not supported in Miri yet:
//  https://github.com/rust-lang/miri/issues/3172#issuecomment-3730602707
#[cfg(not(all(miri, target_arch = "aarch64")))]
#[test]
fn test_aes64im_unit_basis() {
    let mut state = initialize_state([Rv64ZkndInstruction::Aes64Im {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::Zero,
    }]);
    // InvMixColumns([0x01, 0x00, 0x00, 0x00]):
    //   r0 = 0x0e*1 = 0x0e
    //   r1 = 0x09*1 = 0x09
    //   r2 = 0x0d*1 = 0x0d
    //   r3 = 0x0b*1 = 0x0b
    // packed u32 little-endian = 0x0b0d_090e
    let col: u32 = 0x0000_0001;
    let input = u64::from(col) | (u64::from(col) << 32u8);
    state.regs.write(Reg::A0, input);
    execute(&mut state).unwrap();
    let expected_col: u32 = 0x0b0d_090e;
    let expected = u64::from(expected_col) | (u64::from(expected_col) << 32u8);
    assert_eq!(state.regs.read(Reg::A2), expected);
}

// aes64ds / aes64dsm - verify they produce non-trivial, distinct results

// TODO: `llvm.aarch64.crypto.aes*` is not supported in Miri yet:
//  https://github.com/rust-lang/miri/issues/3172#issuecomment-3730602707
#[cfg(not(all(miri, target_arch = "aarch64")))]
#[test]
fn test_aes64ds_nonzero() {
    let mut state = initialize_state([Rv64ZkndInstruction::Aes64Ds {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A0, 0x0011_2233_4455_6677);
    state.regs.write(Reg::A1, 0x8899_aabb_ccdd_eeff);
    execute(&mut state).unwrap();
    assert_ne!(state.regs.read(Reg::A2), 0);
}

// TODO: `llvm.aarch64.crypto.aes*` is not supported in Miri yet:
//  https://github.com/rust-lang/miri/issues/3172#issuecomment-3730602707
#[cfg(not(all(miri, target_arch = "aarch64")))]
#[test]
fn test_aes64dsm_nonzero() {
    let mut state = initialize_state([Rv64ZkndInstruction::Aes64Dsm {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A0, 0x0011_2233_4455_6677);
    state.regs.write(Reg::A1, 0x8899_aabb_ccdd_eeff);
    execute(&mut state).unwrap();
    assert_ne!(state.regs.read(Reg::A2), 0);
}

// TODO: `llvm.aarch64.crypto.aes*` is not supported in Miri yet:
//  https://github.com/rust-lang/miri/issues/3172#issuecomment-3730602707
#[cfg(not(all(miri, target_arch = "aarch64")))]
#[test]
fn test_aes64ds_differs_from_aes64dsm() {
    // Both instructions use the same inputs but produce different results because aes64dsm applies
    // InvMixColumns and aes64ds does not
    let mut state_ds = initialize_state([Rv64ZkndInstruction::Aes64Ds {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    let mut state_dsm = initialize_state([Rv64ZkndInstruction::Aes64Dsm {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    let v0 = 0x0011_2233_4455_6677_u64;
    let v1 = 0x8899_aabb_ccdd_eeff_u64;
    state_ds.regs.write(Reg::A0, v0);
    state_ds.regs.write(Reg::A1, v1);
    state_dsm.regs.write(Reg::A0, v0);
    state_dsm.regs.write(Reg::A1, v1);
    execute(&mut state_ds).unwrap();
    execute(&mut state_dsm).unwrap();
    assert_ne!(state_ds.regs.read(Reg::A2), state_dsm.regs.read(Reg::A2),);
}

// TODO: `llvm.aarch64.crypto.aes*` is not supported in Miri yet:
//  https://github.com/rust-lang/miri/issues/3172#issuecomment-3730602707
#[cfg(not(all(miri, target_arch = "aarch64")))]
#[test]
fn test_aes64ds_arg_swap_differs() {
    // aes64ds(rs1, rs2) != aes64ds(rs2, rs1) in general - this verifies the half-state split is
    // correctly asymmetric
    let mut state1 = initialize_state([Rv64ZkndInstruction::Aes64Ds {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    let mut state2 = initialize_state([Rv64ZkndInstruction::Aes64Ds {
        rd: Reg::A2,
        rs1: Reg::A1,
        rs2: Reg::A0,
    }]);
    let v0 = 0x0011_2233_4455_6677_u64;
    let v1 = 0x8899_aabb_ccdd_eeff_u64;
    state1.regs.write(Reg::A0, v0);
    state1.regs.write(Reg::A1, v1);
    state2.regs.write(Reg::A0, v0);
    state2.regs.write(Reg::A1, v1);
    execute(&mut state1).unwrap();
    execute(&mut state2).unwrap();
    assert_ne!(state1.regs.read(Reg::A2), state2.regs.read(Reg::A2),);
}

// aes64ks1i

#[test]
fn test_aes64ks1i_rnum_1() {
    let mut state = initialize_state([Rv64ZkndInstruction::Aes64Ks1i {
        rd: Reg::A2,
        rs1: Reg::A0,
        rnum: Rv64ZkndKsRnum::R1,
        rs2: Reg::Zero,
    }]);
    // rs1[63:32] = 0x0000_0000
    // RotWord([0x00,0x00,0x00,0x00]) = [0x00,0x00,0x00,0x00] (rotation of zeros is zeros)
    // SubWord = [SBOX[0],SBOX[0],SBOX[0],SBOX[0]] = [0x63,0x63,0x63,0x63]
    // packed LE u32 = 0x6363_6363
    // XOR RCON[1]=0x02 into low byte -> 0x6363_6363 ^ 0x0000_0002 = 0x6363_6361
    // rd = result | (result << 32) = 0x6363_6361_6363_6361
    state.regs.write(Reg::A0, 0);
    execute(&mut state).unwrap();
    let result = 0x6363_6363_u32 ^ 0x0000_0002_u32;
    let expected = u64::from(result) | (u64::from(result) << 32u8);
    assert_eq!(state.regs.read(Reg::A2), expected);
}

#[test]
fn test_aes64ks1i_rnum_1_nonzero_input() {
    let mut state = initialize_state([Rv64ZkndInstruction::Aes64Ks1i {
        rd: Reg::A2,
        rs1: Reg::A0,
        rnum: Rv64ZkndKsRnum::R1,
        rs2: Reg::Zero,
    }]);
    // rs1[63:32] = 0xAABB_CCDD (bytes LE: [0xDD, 0xCC, 0xBB, 0xAA])
    // RotWord via rotate_right(8): 0xAABB_CCDD.rotate_right(8) = 0xDDAA_BBCC
    // (bytes LE: [0xCC, 0xBB, 0xAA, 0xDD])
    // SubWord([0xCC,0xBB,0xAA,0xDD]):
    //   SBOX[0xCC]=0x4B, SBOX[0xBB]=0xEA, SBOX[0xAA]=0xAC, SBOX[0xDD]=0xC1
    // packed = 0x4B | (0xEA<<8) | (0xAC<<16) | (0xC1<<24) = 0xC1AC_EA4B
    // XOR RCON[1]=0x02 -> 0xC1AC_EA4B ^ 0x0000_0002 = 0xC1AC_EA49
    state.regs.write(Reg::A0, 0xAABB_CCDD_0000_0000);
    execute(&mut state).unwrap();
    let b0 = 0x4Bu32; // SBOX[rotated byte 0 = 0xCC]
    let b1 = 0xEAu32; // SBOX[rotated byte 1 = 0xBB]
    let b2 = 0xACu32; // SBOX[rotated byte 2 = 0xAA]
    let b3 = 0xC1u32; // SBOX[rotated byte 3 = 0xDD]
    let subbed = b0 | (b1 << 8u8) | (b2 << 16u8) | (b3 << 24u8);
    let result = subbed ^ 0x02u32;
    let expected = u64::from(result) | (u64::from(result) << 32u8);
    assert_eq!(state.regs.read(Reg::A2), expected);
}

#[test]
fn test_aes64ks1i_rnum_10_no_rot_no_rcon() {
    let mut state = initialize_state([Rv64ZkndInstruction::Aes64Ks1i {
        rd: Reg::A2,
        rs1: Reg::A0,
        rnum: Rv64ZkndKsRnum::Final,
        rs2: Reg::Zero,
    }]);
    // rnum=10: no RotWord, no RCON - just SubWord(rs1[63:32])
    state.regs.write(Reg::A0, 0);
    execute(&mut state).unwrap();
    // SubWord(0x0000_0000) = 0x6363_6363
    assert_eq!(state.regs.read(Reg::A2), 0x6363_6363_6363_6363);
}

#[test]
fn test_aes64ks1i_rnum_10_nonzero_input() {
    let mut state = initialize_state([Rv64ZkndInstruction::Aes64Ks1i {
        rd: Reg::A2,
        rs1: Reg::A0,
        rnum: Rv64ZkndKsRnum::Final,
        rs2: Reg::Zero,
    }]);
    // rnum=10: no RotWord, no RCON - SubWord(rs1[63:32]) only
    // rs1[63:32] = 0x0001_0203 (bytes LE: [0x03,0x02,0x01,0x00])
    // SubWord: SBOX[0x03]=0x7B, SBOX[0x02]=0x77, SBOX[0x01]=0x7C, SBOX[0x00]=0x63
    // packed = 0x7B | (0x77<<8) | (0x7C<<16) | (0x63<<24) = 0x637C_777B
    state.regs.write(Reg::A0, 0x0001_0203_0000_0000);
    execute(&mut state).unwrap();
    let result = 0x637C_777Bu32;
    let expected = u64::from(result) | (u64::from(result) << 32u8);
    assert_eq!(state.regs.read(Reg::A2), expected);
}

#[test]
fn test_aes64ks1i_rnum_0() {
    let mut state = initialize_state([Rv64ZkndInstruction::Aes64Ks1i {
        rd: Reg::A2,
        rs1: Reg::A0,
        rnum: Rv64ZkndKsRnum::R0,
        rs2: Reg::Zero,
    }]);
    // rs1[63:32] = 0x0000_0000
    // RotWord(0x0000_0000) = 0x0000_0000 (all zeros)
    // SubWord: SBOX[0]=0x63 for all bytes -> 0x6363_6363
    // XOR RCON[0]=0x01 into low byte -> 0x63636363 ^ 0x0000_0001 = 0x6363_6362
    state.regs.write(Reg::A0, 0);
    execute(&mut state).unwrap();
    let result = 0x6363_6363_u32 ^ 0x0000_0001_u32;
    let expected = u64::from(result) | (u64::from(result) << 32u8);
    assert_eq!(state.regs.read(Reg::A2), expected);
}

// aes64ks2

#[test]
fn test_aes64ks2_zero() {
    let mut state = initialize_state([Rv64ZkndInstruction::Aes64Ks2 {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A0, 0);
    state.regs.write(Reg::A1, 0);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), 0);
}

#[test]
fn test_aes64ks2_known() {
    let mut state = initialize_state([Rv64ZkndInstruction::Aes64Ks2 {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    // w0 = rs1[63:32] ^ rs2[31:0]
    // w1 = w0 ^ rs2[63:32]
    let rs1: u64 = 0xAABB_CCDD_0000_0000;
    let rs2: u64 = 0x1122_3344_5566_7788;
    state.regs.write(Reg::A0, rs1);
    state.regs.write(Reg::A1, rs2);
    execute(&mut state).unwrap();
    let w0 = 0xAABB_CCDD_u32 ^ 0x5566_7788_u32;
    let w1 = w0 ^ 0x1122_3344_u32;
    let expected = u64::from(w0) | (u64::from(w1) << 32u8);
    assert_eq!(state.regs.read(Reg::A2), expected);
}
