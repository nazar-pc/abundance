use crate::rv64::test_utils::{TEST_BASE_ADDR, execute, initialize_state};
use crate::{RegisterFile, VirtualMemory};
use ab_riscv_primitives::prelude::*;

#[test]
fn test_lb_aq_sign_extends() {
    let mut state = initialize_state([Rv64ZalasrInstruction::LbAq {
        rd: Reg::A1,
        rs1: Reg::A0,
        rl: false,
        rs2: Reg::Zero,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u8>(addr, 0x80).unwrap();
    state.regs.write(Reg::A0, addr);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A1), 0xFFFF_FFFF_FFFF_FF80);
}

#[test]
fn test_lw_aq_sign_extends() {
    let mut state = initialize_state([Rv64ZalasrInstruction::LwAq {
        rd: Reg::A1,
        rs1: Reg::A0,
        rl: false,
        rs2: Reg::Zero,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u32>(addr, 0x8000_0000).unwrap();
    state.regs.write(Reg::A0, addr);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A1), 0xFFFF_FFFF_8000_0000);
}

#[test]
fn test_ld_aq() {
    let mut state = initialize_state([Rv64ZalasrInstruction::LdAq {
        rd: Reg::A1,
        rs1: Reg::A0,
        rl: false,
        rs2: Reg::Zero,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state
        .memory
        .write::<u64>(addr, 0xDEAD_BEEF_1234_5678)
        .unwrap();
    state.regs.write(Reg::A0, addr);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A1), 0xDEAD_BEEF_1234_5678);
}

#[test]
fn test_sb_rl() {
    let mut state = initialize_state([Rv64ZalasrInstruction::SbRl {
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 0x42);

    execute(&mut state).unwrap();

    assert_eq!(state.memory.read::<u8>(addr).unwrap(), 0x42);
}

#[test]
fn test_sd_rl() {
    let mut state = initialize_state([Rv64ZalasrInstruction::SdRl {
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 0xDEAD_BEEF_1234_5678);

    execute(&mut state).unwrap();

    assert_eq!(
        state.memory.read::<u64>(addr).unwrap(),
        0xDEAD_BEEF_1234_5678
    );
}

#[test]
fn test_ld_aq_supports_misaligned_access() {
    let mut state = initialize_state([Rv64ZalasrInstruction::LdAq {
        rd: Reg::A1,
        rs1: Reg::A0,
        rl: false,
        rs2: Reg::Zero,
    }]);
    // Not 8-byte aligned
    let addr = TEST_BASE_ADDR + 0x101;
    state
        .memory
        .write::<u64>(addr, 0xDEAD_BEEF_1234_5678)
        .unwrap();
    state.regs.write(Reg::A0, addr);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A1), 0xDEAD_BEEF_1234_5678);
}
