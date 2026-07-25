use crate::rv32::test_utils::{TEST_BASE_ADDR, execute, initialize_state};
use crate::{RegisterFile, VirtualMemory};
use ab_riscv_primitives::prelude::*;

#[test]
fn test_lb_aq_sign_extends() {
    let mut state = initialize_state([Rv32ZalasrInstruction::LbAq {
        rd: Reg::A1,
        rs1: Reg::A0,
        rl: false,
        rs2: Reg::Zero,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u8>(u64::from(addr), 0x80).unwrap();
    state.regs.write(Reg::A0, addr);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A1), 0xFFFF_FF80);
}

#[test]
fn test_lh_aq_sign_extends() {
    let mut state = initialize_state([Rv32ZalasrInstruction::LhAq {
        rd: Reg::A1,
        rs1: Reg::A0,
        rl: false,
        rs2: Reg::Zero,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u16>(u64::from(addr), 0x8000).unwrap();
    state.regs.write(Reg::A0, addr);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A1), 0xFFFF_8000);
}

#[test]
fn test_lw_aq() {
    let mut state = initialize_state([Rv32ZalasrInstruction::LwAq {
        rd: Reg::A1,
        rs1: Reg::A0,
        rl: false,
        rs2: Reg::Zero,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state
        .memory
        .write::<u32>(u64::from(addr), 0xDEAD_BEEF)
        .unwrap();
    state.regs.write(Reg::A0, addr);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A1), 0xDEAD_BEEF);
}

#[test]
fn test_sb_rl() {
    let mut state = initialize_state([Rv32ZalasrInstruction::SbRl {
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 0x42);

    execute(&mut state).unwrap();

    assert_eq!(state.memory.read::<u8>(u64::from(addr)).unwrap(), 0x42);
}

#[test]
fn test_sh_rl() {
    let mut state = initialize_state([Rv32ZalasrInstruction::ShRl {
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 0x1234);

    execute(&mut state).unwrap();

    assert_eq!(state.memory.read::<u16>(u64::from(addr)).unwrap(), 0x1234);
}

#[test]
fn test_sw_rl() {
    let mut state = initialize_state([Rv32ZalasrInstruction::SwRl {
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 0xDEAD_BEEF);

    execute(&mut state).unwrap();

    assert_eq!(
        state.memory.read::<u32>(u64::from(addr)).unwrap(),
        0xDEAD_BEEF
    );
}

#[test]
fn test_lw_aq_supports_misaligned_access() {
    let mut state = initialize_state([Rv32ZalasrInstruction::LwAq {
        rd: Reg::A1,
        rs1: Reg::A0,
        rl: false,
        rs2: Reg::Zero,
    }]);
    // Not 4-byte aligned
    let addr = TEST_BASE_ADDR + 0x101;
    state
        .memory
        .write::<u32>(u64::from(addr), 0xDEAD_BEEF)
        .unwrap();
    state.regs.write(Reg::A0, addr);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A1), 0xDEAD_BEEF);
}
