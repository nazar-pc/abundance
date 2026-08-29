use crate::rv64::test_utils::{TEST_BASE_ADDR, execute, initialize_state};
use crate::{ExecutionError, RegisterFile, VirtualMemory};
use ab_riscv_primitives::prelude::*;
use core::assert_matches;

#[test]
fn test_lr_w_sign_extends() {
    let mut state = initialize_state([Rv64ZalrscInstruction::Lr {
        rd: Reg::A1,
        rs1: Reg::A0,
        aq: false,
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
fn test_sc_w_succeeds_after_matching_lr() {
    let mut state = initialize_state([
        Rv64ZalrscInstruction::Lr {
            rd: Reg::A1,
            rs1: Reg::A0,
            aq: false,
            rl: false,
            rs2: Reg::Zero,
        },
        Rv64ZalrscInstruction::Sc {
            rd: Reg::A2,
            rs1: Reg::A0,
            rs2: Reg::A3,
            aq: false,
            rl: false,
        },
    ]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u32>(addr, 1).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A3, 42);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0);
    assert_eq!(state.memory.read::<u32>(addr).unwrap(), 42);
}

#[test]
fn test_sc_w_fails_without_prior_lr() {
    let mut state = initialize_state([Rv64ZalrscInstruction::Sc {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A3,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u32>(addr, 1).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A3, 42);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 1);
    assert_eq!(state.memory.read::<u32>(addr).unwrap(), 1);
}

#[test]
fn test_lr_d_reads_full_64_bits() {
    let mut state = initialize_state([Rv64ZalrscInstruction::LrD {
        rd: Reg::A1,
        rs1: Reg::A0,
        aq: false,
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
fn test_sc_d_succeeds_after_matching_lr() {
    let mut state = initialize_state([
        Rv64ZalrscInstruction::LrD {
            rd: Reg::A1,
            rs1: Reg::A0,
            aq: false,
            rl: false,
            rs2: Reg::Zero,
        },
        Rv64ZalrscInstruction::ScD {
            rd: Reg::A2,
            rs1: Reg::A0,
            rs2: Reg::A3,
            aq: false,
            rl: false,
        },
    ]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u64>(addr, 1).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A3, 0xABCD_EF01_2345_6789);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0);
    assert_eq!(
        state.memory.read::<u64>(addr).unwrap(),
        0xABCD_EF01_2345_6789
    );
}

#[test]
fn test_sc_d_fails_for_different_address() {
    let mut state = initialize_state([
        Rv64ZalrscInstruction::LrD {
            rd: Reg::A1,
            rs1: Reg::A0,
            aq: false,
            rl: false,
            rs2: Reg::Zero,
        },
        Rv64ZalrscInstruction::ScD {
            rd: Reg::A2,
            rs1: Reg::A4,
            rs2: Reg::A3,
            aq: false,
            rl: false,
        },
    ]);
    let addr = TEST_BASE_ADDR + 0x100;
    let other_addr = TEST_BASE_ADDR + 0x200;
    state.memory.write::<u64>(addr, 1).unwrap();
    state.memory.write::<u64>(other_addr, 2).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A4, other_addr);
    state.regs.write(Reg::A3, 42);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 1);
    assert_eq!(state.memory.read::<u64>(other_addr).unwrap(), 2);
}

#[test]
fn test_sc_raises_store_access_fault_on_failure_out_of_bounds() {
    let mut state = initialize_state([Rv64ZalrscInstruction::Sc {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A3,
        aq: false,
        rl: false,
    }]);
    // 4-byte aligned but outside `TestMemory`'s bounds
    let addr = TEST_BASE_ADDR + 0x0010_0000;
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A3, 42);

    // No prior `lr`, so this `sc` fails on the reservation check - but it must still probe its
    // target address for an access fault, and report it as Store/AMO (not Load)
    assert_matches!(
        execute(&mut state),
        Err(ExecutionError::OutOfBoundsWrite { .. })
    );
}

#[test]
fn test_sc_d_raises_store_access_fault_on_failure_out_of_bounds() {
    let mut state = initialize_state([Rv64ZalrscInstruction::ScD {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A3,
        aq: false,
        rl: false,
    }]);
    // 8-byte aligned but outside `TestMemory`'s bounds
    let addr = TEST_BASE_ADDR + 0x0010_0000;
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A3, 42);

    assert_matches!(
        execute(&mut state),
        Err(ExecutionError::OutOfBoundsWrite { .. })
    );
}

#[test]
fn test_lr_rejects_misaligned_access() {
    let mut state = initialize_state([Rv64ZalrscInstruction::Lr {
        rd: Reg::A1,
        rs1: Reg::A0,
        aq: false,
        rl: false,
        rs2: Reg::Zero,
    }]);
    // Not 4-byte aligned
    let addr = TEST_BASE_ADDR + 0x101;
    state.regs.write(Reg::A0, addr);

    assert_matches!(
        execute(&mut state),
        Err(ExecutionError::MisalignedRead { .. })
    );
}

#[test]
fn test_sc_rejects_misaligned_access() {
    let mut state = initialize_state([Rv64ZalrscInstruction::Sc {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A3,
        aq: false,
        rl: false,
    }]);
    // Not 4-byte aligned
    let addr = TEST_BASE_ADDR + 0x101;
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A3, 42);

    assert_matches!(
        execute(&mut state),
        Err(ExecutionError::MisalignedWrite { .. })
    );
}

#[test]
fn test_lr_d_rejects_misaligned_access() {
    let mut state = initialize_state([Rv64ZalrscInstruction::LrD {
        rd: Reg::A1,
        rs1: Reg::A0,
        aq: false,
        rl: false,
        rs2: Reg::Zero,
    }]);
    // Not 8-byte aligned
    let addr = TEST_BASE_ADDR + 0x101;
    state.regs.write(Reg::A0, addr);

    assert_matches!(
        execute(&mut state),
        Err(ExecutionError::MisalignedRead { .. })
    );
}

#[test]
fn test_sc_d_rejects_misaligned_access() {
    let mut state = initialize_state([Rv64ZalrscInstruction::ScD {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A3,
        aq: false,
        rl: false,
    }]);
    // Not 8-byte aligned
    let addr = TEST_BASE_ADDR + 0x101;
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A3, 42);

    assert_matches!(
        execute(&mut state),
        Err(ExecutionError::MisalignedWrite { .. })
    );
}
