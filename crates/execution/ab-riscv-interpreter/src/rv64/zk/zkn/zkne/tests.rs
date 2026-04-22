use crate::RegisterFile;
use crate::rv64::test_utils::{execute, initialize_state};
use ab_riscv_primitives::prelude::*;
// aes64es / aes64esm - verify non-trivial, distinct, asymmetric results

// TODO: `llvm.aarch64.crypto.aes*` is not supported in Miri yet:
//  https://github.com/rust-lang/miri/issues/3172#issuecomment-3730602707
#[cfg(not(all(miri, target_arch = "aarch64")))]
#[test]
fn test_aes64es_nonzero() {
    let mut state = initialize_state([Rv64ZkneInstruction::Aes64Es {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A0, 0x0011223344556677u64);
    state.regs.write(Reg::A1, 0x8899aabbccddeeffu64);
    execute(&mut state).unwrap();
    assert_ne!(state.regs.read(Reg::A2), 0u64);
}

// TODO: `llvm.aarch64.crypto.aes*` is not supported in Miri yet:
//  https://github.com/rust-lang/miri/issues/3172#issuecomment-3730602707
#[cfg(not(all(miri, target_arch = "aarch64")))]
#[test]
fn test_aes64esm_nonzero() {
    let mut state = initialize_state([Rv64ZkneInstruction::Aes64Esm {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A0, 0x0011223344556677u64);
    state.regs.write(Reg::A1, 0x8899aabbccddeeffu64);
    execute(&mut state).unwrap();
    assert_ne!(state.regs.read(Reg::A2), 0u64);
}

// TODO: `llvm.aarch64.crypto.aes*` is not supported in Miri yet:
//  https://github.com/rust-lang/miri/issues/3172#issuecomment-3730602707
#[cfg(not(all(miri, target_arch = "aarch64")))]
#[test]
fn test_aes64es_differs_from_aes64esm() {
    // Both instructions use the same inputs but produce different results because aes64esm applies
    // MixColumns and aes64es does not
    let mut state_es = initialize_state([Rv64ZkneInstruction::Aes64Es {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    let mut state_esm = initialize_state([Rv64ZkneInstruction::Aes64Esm {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    let v0 = 0x0011223344556677u64;
    let v1 = 0x8899aabbccddeeffu64;
    state_es.regs.write(Reg::A0, v0);
    state_es.regs.write(Reg::A1, v1);
    state_esm.regs.write(Reg::A0, v0);
    state_esm.regs.write(Reg::A1, v1);
    execute(&mut state_es).unwrap();
    execute(&mut state_esm).unwrap();
    assert_ne!(state_es.regs.read(Reg::A2), state_esm.regs.read(Reg::A2));
}

// TODO: `llvm.aarch64.crypto.aes*` is not supported in Miri yet:
//  https://github.com/rust-lang/miri/issues/3172#issuecomment-3730602707
#[cfg(not(all(miri, target_arch = "aarch64")))]
#[test]
fn test_aes64es_arg_swap_differs() {
    // aes64es(rs1, rs2) != aes64es(rs2, rs1) in general - verifies the half-state split is
    // correctly asymmetric
    let mut state1 = initialize_state([Rv64ZkneInstruction::Aes64Es {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    let mut state2 = initialize_state([Rv64ZkneInstruction::Aes64Es {
        rd: Reg::A2,
        rs1: Reg::A1,
        rs2: Reg::A0,
    }]);
    let v0 = 0x0011223344556677u64;
    let v1 = 0x8899aabbccddeeffu64;
    state1.regs.write(Reg::A0, v0);
    state1.regs.write(Reg::A1, v1);
    state2.regs.write(Reg::A0, v0);
    state2.regs.write(Reg::A1, v1);
    execute(&mut state1).unwrap();
    execute(&mut state2).unwrap();
    assert_ne!(state1.regs.read(Reg::A2), state2.regs.read(Reg::A2));
}

// TODO: `llvm.aarch64.crypto.aes*` is not supported in Miri yet:
//  https://github.com/rust-lang/miri/issues/3172#issuecomment-3730602707
#[cfg(not(all(miri, target_arch = "aarch64")))]
#[test]
fn test_aes64es_zero_input_known() {
    let mut state = initialize_state([Rv64ZkneInstruction::Aes64Es {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    // All-zero state: ShiftRows is a no-op on uniform input, SubBytes maps 0x00 -> 0x63.
    // Every byte of every column becomes 0x63, so the low 64-bit half = 0x6363636363636363.
    state.regs.write(Reg::A0, 0u64);
    state.regs.write(Reg::A1, 0u64);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), 0x6363636363636363u64);
}

// TODO: `llvm.aarch64.crypto.aes*` is not supported in Miri yet:
//  https://github.com/rust-lang/miri/issues/3172#issuecomment-3730602707
#[cfg(not(all(miri, target_arch = "aarch64")))]
#[test]
fn test_aes64esm_zero_input_known() {
    let mut state = initialize_state([Rv64ZkneInstruction::Aes64Esm {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    // All-zero state: after SubBytes every byte = 0x63. MixColumns on [0x63, 0x63, 0x63, 0x63]:
    //   r0 = gmul(0x63,2)^gmul(0x63,3)^0x63^0x63
    //      = gmul(0x63,2)^gmul(0x63,3) (identical pairs cancel)
    //      = gmul(0x63, 2^3) = gmul(0x63, 1) = 0x63
    // All four rows are symmetric and identical, so every output byte = 0x63.
    state.regs.write(Reg::A0, 0u64);
    state.regs.write(Reg::A1, 0u64);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), 0x6363636363636363u64);
}
