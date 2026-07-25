use crate::rv32::test_utils::{TEST_BASE_ADDR, execute, initialize_state};
use crate::{RegisterFile, VirtualMemory};
use ab_riscv_primitives::prelude::*;

#[test]
fn test_lr_reads_value() {
    let mut state = initialize_state([Rv32ZalrscInstruction::Lr {
        rd: Reg::A1,
        rs1: Reg::A0,
        aq: false,
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
fn test_sc_succeeds_after_matching_lr() {
    let mut state = initialize_state([
        Rv32ZalrscInstruction::Lr {
            rd: Reg::A1,
            rs1: Reg::A0,
            aq: false,
            rl: false,
            rs2: Reg::Zero,
        },
        Rv32ZalrscInstruction::Sc {
            rd: Reg::A2,
            rs1: Reg::A0,
            rs2: Reg::A3,
            aq: false,
            rl: false,
        },
    ]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u32>(u64::from(addr), 1).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A3, 42);

    execute(&mut state).unwrap();

    // `sc` succeeds, so it returns 0 and stores the new value
    assert_eq!(state.regs.read(Reg::A2), 0);
    assert_eq!(state.memory.read::<u32>(u64::from(addr)).unwrap(), 42);
}

#[test]
fn test_sc_fails_without_prior_lr() {
    let mut state = initialize_state([Rv32ZalrscInstruction::Sc {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A3,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u32>(u64::from(addr), 1).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A3, 42);

    execute(&mut state).unwrap();

    // `sc` fails, so it returns a nonzero value and does not store
    assert_eq!(state.regs.read(Reg::A2), 1);
    assert_eq!(state.memory.read::<u32>(u64::from(addr)).unwrap(), 1);
}

#[test]
fn test_sc_fails_for_different_address() {
    let mut state = initialize_state([
        Rv32ZalrscInstruction::Lr {
            rd: Reg::A1,
            rs1: Reg::A0,
            aq: false,
            rl: false,
            rs2: Reg::Zero,
        },
        Rv32ZalrscInstruction::Sc {
            rd: Reg::A2,
            rs1: Reg::A4,
            rs2: Reg::A3,
            aq: false,
            rl: false,
        },
    ]);
    let addr = TEST_BASE_ADDR + 0x100;
    let other_addr = TEST_BASE_ADDR + 0x200;
    state.memory.write::<u32>(u64::from(addr), 1).unwrap();
    state.memory.write::<u32>(u64::from(other_addr), 2).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A4, other_addr);
    state.regs.write(Reg::A3, 42);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 1);
    assert_eq!(state.memory.read::<u32>(u64::from(other_addr)).unwrap(), 2);
}

#[test]
fn test_sc_fails_after_second_sc() {
    let mut state = initialize_state([
        Rv32ZalrscInstruction::Lr {
            rd: Reg::A1,
            rs1: Reg::A0,
            aq: false,
            rl: false,
            rs2: Reg::Zero,
        },
        Rv32ZalrscInstruction::Sc {
            rd: Reg::A2,
            rs1: Reg::A0,
            rs2: Reg::A3,
            aq: false,
            rl: false,
        },
        Rv32ZalrscInstruction::Sc {
            rd: Reg::A5,
            rs1: Reg::A0,
            rs2: Reg::A3,
            aq: false,
            rl: false,
        },
    ]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u32>(u64::from(addr), 1).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A3, 42);

    execute(&mut state).unwrap();

    // First `sc` succeeds and invalidates the reservation, so the second `sc` fails
    assert_eq!(state.regs.read(Reg::A2), 0);
    assert_eq!(state.regs.read(Reg::A5), 1);
}

#[test]
fn test_lr_sc_supports_misaligned_access() {
    let mut state = initialize_state([
        Rv32ZalrscInstruction::Lr {
            rd: Reg::A1,
            rs1: Reg::A0,
            aq: false,
            rl: false,
            rs2: Reg::Zero,
        },
        Rv32ZalrscInstruction::Sc {
            rd: Reg::A2,
            rs1: Reg::A0,
            rs2: Reg::A3,
            aq: false,
            rl: false,
        },
    ]);
    // Not 4-byte aligned
    let addr = TEST_BASE_ADDR + 0x101;
    state.memory.write::<u32>(u64::from(addr), 1).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A3, 42);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0);
    assert_eq!(state.memory.read::<u32>(u64::from(addr)).unwrap(), 42);
}
