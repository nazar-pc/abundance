use crate::rv64::test_utils::{TEST_BASE_ADDR, execute, initialize_state};
use crate::{ExecutionError, RegisterFile, VirtualMemory};
use ab_riscv_primitives::prelude::*;
use core::assert_matches;

#[test]
fn test_amocas_w_succeeds_and_sign_extends() {
    let mut state = initialize_state([Rv64ZacasInstruction::AmocasW {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u32>(addr, 0x8000_0000).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A2, 0xFFFF_FFFF_8000_0000);
    state.regs.write(Reg::A1, 222);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xFFFF_FFFF_8000_0000);
    assert_eq!(state.memory.read::<u32>(addr).unwrap(), 222);
}

#[test]
fn test_amocas_w_ignores_upper_bits_of_rd() {
    // Upper 32 bits of the compare value in `rd` must be ignored
    let mut state = initialize_state([Rv64ZacasInstruction::AmocasW {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u32>(addr, 111).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A2, 0xDEAD_BEEF_0000_006F);
    state.regs.write(Reg::A1, 222);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 111);
    assert_eq!(state.memory.read::<u32>(addr).unwrap(), 222);
}

#[test]
fn test_amocas_w_fails_on_mismatch() {
    let mut state = initialize_state([Rv64ZacasInstruction::AmocasW {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u32>(addr, 111).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A2, 999);
    state.regs.write(Reg::A1, 222);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 111);
    assert_eq!(state.memory.read::<u32>(addr).unwrap(), 111);
}

#[test]
fn test_amocas_d_succeeds() {
    let mut state = initialize_state([Rv64ZacasInstruction::AmocasD {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u64>(addr, 111).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A2, 111);
    state.regs.write(Reg::A1, 0xABCD_EF01_2345_6789);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 111);
    assert_eq!(
        state.memory.read::<u64>(addr).unwrap(),
        0xABCD_EF01_2345_6789
    );
}

#[test]
fn test_amocas_d_fails_on_mismatch() {
    let mut state = initialize_state([Rv64ZacasInstruction::AmocasD {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u64>(addr, 111).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A2, 999);
    state.regs.write(Reg::A1, 222);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 111);
    assert_eq!(state.memory.read::<u64>(addr).unwrap(), 111);
}

#[test]
fn test_amocas_q_register_pair_succeeds() {
    let mut state = initialize_state([Rv64ZacasInstruction::AmocasQ {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A4,
        rd_hi: Reg::A3,
        rs2_hi: Reg::A5,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u64>(addr, 1).unwrap();
    state.memory.write::<u64>(addr + 8, 2).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A2, 1);
    state.regs.write(Reg::A3, 2);
    state.regs.write(Reg::A4, 10);
    state.regs.write(Reg::A5, 20);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 1);
    assert_eq!(state.regs.read(Reg::A3), 2);
    assert_eq!(state.memory.read::<u64>(addr).unwrap(), 10);
    assert_eq!(state.memory.read::<u64>(addr + 8).unwrap(), 20);
}

#[test]
fn test_amocas_q_register_pair_fails_on_high_word_mismatch() {
    let mut state = initialize_state([Rv64ZacasInstruction::AmocasQ {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A4,
        rd_hi: Reg::A3,
        rs2_hi: Reg::A5,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u64>(addr, 1).unwrap();
    state.memory.write::<u64>(addr + 8, 2).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A2, 1);
    // Mismatched high half
    state.regs.write(Reg::A3, 999);
    state.regs.write(Reg::A4, 10);
    state.regs.write(Reg::A5, 20);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 1);
    assert_eq!(state.regs.read(Reg::A3), 2);
    assert_eq!(state.memory.read::<u64>(addr).unwrap(), 1);
    assert_eq!(state.memory.read::<u64>(addr + 8).unwrap(), 2);
}

#[test]
fn test_amocas_q_rd_zero_skips_whole_pair_write() {
    // Per spec: when the first register of the *destination* pair is `x0`, the whole pair write
    // is skipped (not just the `x0` half) - `rd_hi` must retain its original value even though
    // the compare succeeds and the swap happens.
    let mut state = initialize_state([Rv64ZacasInstruction::AmocasQ {
        rd: Reg::Zero,
        rs1: Reg::A0,
        rs2: Reg::A4,
        rd_hi: Reg::Ra,
        rs2_hi: Reg::A5,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    // Both compare halves are forced to 0 (rd == x0), so memory must hold 0/0 for the compare
    // to succeed and the swap to actually happen.
    state.memory.write::<u64>(addr, 0).unwrap();
    state.memory.write::<u64>(addr + 8, 0).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::Ra, 777);
    state.regs.write(Reg::A4, 10);
    state.regs.write(Reg::A5, 20);

    execute(&mut state).unwrap();

    // The swap happened (memory now holds the new value), yet rd_hi (Ra) must be untouched.
    assert_eq!(state.memory.read::<u64>(addr).unwrap(), 10);
    assert_eq!(state.memory.read::<u64>(addr + 8).unwrap(), 20);
    assert_eq!(state.regs.read(Reg::Ra), 777);
}

#[test]
fn test_amocas_q_rs2_zero_forces_swap_high_to_zero() {
    // Per spec: when the first register of the *source* pair is `x0`, BOTH halves of the swap
    // value read as zero - `rs2_hi`'s actual register value must be ignored.
    let mut state = initialize_state([Rv64ZacasInstruction::AmocasQ {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::Zero,
        rd_hi: Reg::A3,
        rs2_hi: Reg::Ra,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u64>(addr, 1).unwrap();
    state.memory.write::<u64>(addr + 8, 2).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A2, 1);
    state.regs.write(Reg::A3, 2);
    // rs2_hi holds a nonzero value, but must be ignored since rs2 (first of pair) is x0
    state.regs.write(Reg::Ra, 999);

    execute(&mut state).unwrap();

    assert_eq!(state.memory.read::<u64>(addr).unwrap(), 0);
    assert_eq!(state.memory.read::<u64>(addr + 8).unwrap(), 0);
}

#[test]
fn test_amoaddd_inherited_from_zaamo() {
    let mut state = initialize_state([Rv64ZacasInstruction::AmoaddD {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u64>(addr, 10).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 10);
    assert_eq!(state.memory.read::<u64>(addr).unwrap(), 42);
}

#[test]
fn test_amocasw_rejects_misaligned_atomicity_granule_crossing() {
    let mut state = initialize_state([Rv64ZacasInstruction::AmocasW {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    // 3 bytes before a 4096-byte misaligned atomicity granule boundary: the 4-byte access
    // straddles it
    let addr = TEST_BASE_ADDR + 0xffd;
    state.regs.write(Reg::A0, addr);

    assert_matches!(
        execute(&mut state),
        Err(ExecutionError::MisalignedAtomic { .. })
    );
}

#[test]
fn test_amocasd_rejects_misaligned_atomicity_granule_crossing() {
    let mut state = initialize_state([Rv64ZacasInstruction::AmocasD {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    // 6 bytes before a 4096-byte misaligned atomicity granule boundary: the 8-byte access
    // straddles it
    let addr = TEST_BASE_ADDR + 0xffa;
    state.regs.write(Reg::A0, addr);

    assert_matches!(
        execute(&mut state),
        Err(ExecutionError::MisalignedAtomic { .. })
    );
}

#[test]
fn test_amocasq_rejects_misaligned_atomicity_granule_crossing() {
    let mut state = initialize_state([Rv64ZacasInstruction::AmocasQ {
        rd: Reg::A2,
        rd_hi: Reg::A3,
        rs1: Reg::A0,
        rs2: Reg::A4,
        rs2_hi: Reg::A5,
        aq: false,
        rl: false,
    }]);
    // 15 bytes before a 4096-byte misaligned atomicity granule boundary: the 16-byte access
    // straddles it
    let addr = TEST_BASE_ADDR + 0xff1;
    state.regs.write(Reg::A0, addr);

    assert_matches!(
        execute(&mut state),
        Err(ExecutionError::MisalignedAtomic { .. })
    );
}
