use crate::rv32::test_utils::{TEST_BASE_ADDR, execute, initialize_state};
use crate::{RegisterFile, VirtualMemory};
use ab_riscv_primitives::prelude::*;

#[test]
fn test_amocas_w_succeeds_on_match() {
    let mut state = initialize_state([Rv32ZacasInstruction::AmocasW {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u32>(u64::from(addr), 111).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A2, 111);
    state.regs.write(Reg::A1, 222);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 111);
    assert_eq!(state.memory.read::<u32>(u64::from(addr)).unwrap(), 222);
}

#[test]
fn test_amocas_w_fails_on_mismatch() {
    let mut state = initialize_state([Rv32ZacasInstruction::AmocasW {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u32>(u64::from(addr), 111).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A2, 999);
    state.regs.write(Reg::A1, 222);

    execute(&mut state).unwrap();

    // The old memory value is always returned into rd, whether the swap happens or not
    assert_eq!(state.regs.read(Reg::A2), 111);
    assert_eq!(state.memory.read::<u32>(u64::from(addr)).unwrap(), 111);
}

#[test]
fn test_amocas_d_register_pair_succeeds() {
    let mut state = initialize_state([Rv32ZacasInstruction::AmocasD {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A4,
        rd_hi: Reg::A3,
        rs2_hi: Reg::A5,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u32>(u64::from(addr), 1).unwrap();
    state.memory.write::<u32>(u64::from(addr + 4), 2).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A2, 1);
    state.regs.write(Reg::A3, 2);
    state.regs.write(Reg::A4, 10);
    state.regs.write(Reg::A5, 20);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 1);
    assert_eq!(state.regs.read(Reg::A3), 2);
    assert_eq!(state.memory.read::<u32>(u64::from(addr)).unwrap(), 10);
    assert_eq!(state.memory.read::<u32>(u64::from(addr + 4)).unwrap(), 20);
}

#[test]
fn test_amocas_d_register_pair_fails_on_high_word_mismatch() {
    let mut state = initialize_state([Rv32ZacasInstruction::AmocasD {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A4,
        rd_hi: Reg::A3,
        rs2_hi: Reg::A5,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u32>(u64::from(addr), 1).unwrap();
    state.memory.write::<u32>(u64::from(addr + 4), 2).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A2, 1);
    // Mismatched high word
    state.regs.write(Reg::A3, 999);
    state.regs.write(Reg::A4, 10);
    state.regs.write(Reg::A5, 20);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 1);
    assert_eq!(state.regs.read(Reg::A3), 2);
    assert_eq!(state.memory.read::<u32>(u64::from(addr)).unwrap(), 1);
    assert_eq!(state.memory.read::<u32>(u64::from(addr + 4)).unwrap(), 2);
}

#[test]
fn test_amocas_d_rd_zero_skips_whole_pair_write() {
    // Per spec: when the first register of the *destination* pair is `x0`, the whole pair write
    // is skipped (not just the `x0` half) - `rd_hi` must retain its original value even though
    // the compare succeeds and the swap happens.
    let mut state = initialize_state([Rv32ZacasInstruction::AmocasD {
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
    state.memory.write::<u32>(u64::from(addr), 0).unwrap();
    state.memory.write::<u32>(u64::from(addr + 4), 0).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::Ra, 777);
    state.regs.write(Reg::A4, 10);
    state.regs.write(Reg::A5, 20);

    execute(&mut state).unwrap();

    // The swap happened (memory now holds the new value), yet rd_hi (Ra) must be untouched.
    assert_eq!(state.memory.read::<u32>(u64::from(addr)).unwrap(), 10);
    assert_eq!(state.memory.read::<u32>(u64::from(addr + 4)).unwrap(), 20);
    assert_eq!(state.regs.read(Reg::Ra), 777);
}

#[test]
fn test_amocas_d_rs2_zero_forces_swap_high_to_zero() {
    // Per spec: when the first register of the *source* pair is `x0`, BOTH halves of the swap
    // value read as zero - `rs2_hi`'s actual register value must be ignored.
    let mut state = initialize_state([Rv32ZacasInstruction::AmocasD {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::Zero,
        rd_hi: Reg::A3,
        rs2_hi: Reg::Ra,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u32>(u64::from(addr), 1).unwrap();
    state.memory.write::<u32>(u64::from(addr + 4), 2).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A2, 1);
    state.regs.write(Reg::A3, 2);
    // rs2_hi holds a nonzero value, but must be ignored since rs2 (first of pair) is x0
    state.regs.write(Reg::Ra, 999);

    execute(&mut state).unwrap();

    assert_eq!(state.memory.read::<u32>(u64::from(addr)).unwrap(), 0);
    assert_eq!(state.memory.read::<u32>(u64::from(addr + 4)).unwrap(), 0);
}

#[test]
fn test_amoadd_inherited_from_zaamo() {
    let mut state = initialize_state([Rv32ZacasInstruction::Amoadd {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u32>(u64::from(addr), 10).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 10);
    assert_eq!(state.memory.read::<u32>(u64::from(addr)).unwrap(), 42);
}
