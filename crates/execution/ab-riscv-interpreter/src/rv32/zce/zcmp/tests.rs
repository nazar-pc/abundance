use crate::rv32::test_utils::{TEST_BASE_ADDR, execute, initialize_state};
use crate::{ExecutionError, ProgramCounter, RegisterFile, VirtualMemory};
use ab_riscv_primitives::prelude::*;

// CM.PUSH

#[test]
fn test_cm_push_ra_only_decrements_sp() {
    // urlist=4 ({ra}), stack_adj=16 -> base=16
    let mut state = initialize_state([Rv32ZcmpInstruction::CmPush {
        urlist: ZcmpUrlist::try_from_raw(4).unwrap(),
        stack_adj: 16,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    let sp_start = TEST_BASE_ADDR + 0x400;
    state.regs.write(Reg::Sp, sp_start);
    state.regs.write(Reg::Ra, 0xDEAD_0001);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::Sp), sp_start - 16);
    // ra stored at sp_start-4 (4 bytes per slot in RV32)
    assert_eq!(
        state.memory.read::<u32>(u64::from(sp_start - 4)).unwrap(),
        0xDEAD_0001
    );
}

#[test]
fn test_cm_push_stack_adj_adds_extra() {
    // urlist=4 ({ra}), stack_adj=32 -> base=16 + 16 extra
    let mut state = initialize_state([Rv32ZcmpInstruction::CmPush {
        urlist: ZcmpUrlist::try_from_raw(4).unwrap(),
        stack_adj: 32,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    let sp_start = TEST_BASE_ADDR + 0x400;
    state.regs.write(Reg::Sp, sp_start);
    state.regs.write(Reg::Ra, 0xABCD);
    execute(&mut state).unwrap();
    // sp decremented by full stack_adj, not just base
    assert_eq!(state.regs.read(Reg::Sp), sp_start - 32);
    // registers still stored relative to original sp, not stack_adj
    assert_eq!(
        state.memory.read::<u32>(u64::from(sp_start - 4)).unwrap(),
        0xABCD
    );
}

#[test]
fn test_cm_push_ra_s0() {
    // urlist=5 ({ra, s0}), stack_adj=16
    let mut state = initialize_state([Rv32ZcmpInstruction::CmPush {
        urlist: ZcmpUrlist::try_from_raw(5).unwrap(),
        stack_adj: 16,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    let sp_start = TEST_BASE_ADDR + 0x400;
    state.regs.write(Reg::Sp, sp_start);
    state.regs.write(Reg::Ra, 0x1111);
    state.regs.write(Reg::S0, 0x2222);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::Sp), sp_start - 16);
    // ra at sp_start-4, s0 at sp_start-8
    assert_eq!(
        state.memory.read::<u32>(u64::from(sp_start - 4)).unwrap(),
        0x1111
    );
    assert_eq!(
        state.memory.read::<u32>(u64::from(sp_start - 8)).unwrap(),
        0x2222
    );
}

#[test]
fn test_cm_push_ra_s0_s1() {
    // urlist=6 ({ra, s0, s1}), stack_adj=16
    let mut state = initialize_state([Rv32ZcmpInstruction::CmPush {
        urlist: ZcmpUrlist::try_from_raw(6).unwrap(),
        stack_adj: 16,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    let sp_start = TEST_BASE_ADDR + 0x500;
    state.regs.write(Reg::Sp, sp_start);
    state.regs.write(Reg::Ra, 0x1111);
    state.regs.write(Reg::S0, 0x2222);
    state.regs.write(Reg::S1, 0x3333);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::Sp), sp_start - 16);
    assert_eq!(
        state.memory.read::<u32>(u64::from(sp_start - 4)).unwrap(),
        0x1111
    );
    assert_eq!(
        state.memory.read::<u32>(u64::from(sp_start - 8)).unwrap(),
        0x2222
    );
    assert_eq!(
        state.memory.read::<u32>(u64::from(sp_start - 12)).unwrap(),
        0x3333
    );
}

#[test]
fn test_cm_push_max_urlist() {
    // urlist=15 ({ra, s0-s11}), stack_adj=64
    let mut state = initialize_state([Rv32ZcmpInstruction::CmPush {
        urlist: ZcmpUrlist::try_from_raw(15).unwrap(),
        stack_adj: 64,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    let sp_start = TEST_BASE_ADDR + 0x800;
    state.regs.write(Reg::Sp, sp_start);
    state.regs.write(Reg::Ra, 0x1);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::Sp), sp_start - 64);
}

// CM.POP

#[test]
fn test_cm_pop_restores_and_increments_sp() {
    // urlist=4 ({ra}), stack_adj=16
    let mut state = initialize_state([Rv32ZcmpInstruction::CmPop {
        urlist: ZcmpUrlist::try_from_raw(4).unwrap(),
        stack_adj: 16,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    let sp_start = TEST_BASE_ADDR + 0x300;
    state.regs.write(Reg::Sp, sp_start);
    let new_sp = sp_start + 16;
    state
        .memory
        .write::<u32>(u64::from(new_sp - 4), 0xABCD_1234)
        .unwrap();
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::Ra), 0xABCD_1234);
    assert_eq!(state.regs.read(Reg::Sp), new_sp);
}

#[test]
fn test_cm_pop_ra_s0() {
    // urlist=5, stack_adj=16
    let mut state = initialize_state([Rv32ZcmpInstruction::CmPop {
        urlist: ZcmpUrlist::try_from_raw(5).unwrap(),
        stack_adj: 16,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    let sp_start = TEST_BASE_ADDR + 0x300;
    state.regs.write(Reg::Sp, sp_start);
    let new_sp = sp_start + 16;
    state
        .memory
        .write::<u32>(u64::from(new_sp - 4), 0x1111)
        .unwrap();
    state
        .memory
        .write::<u32>(u64::from(new_sp - 8), 0x2222)
        .unwrap();
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::Ra), 0x1111);
    assert_eq!(state.regs.read(Reg::S0), 0x2222);
    assert_eq!(state.regs.read(Reg::Sp), new_sp);
}

#[test]
fn test_cm_pop_stack_adj_extra() {
    // urlist=4, stack_adj=32 -> base=16 + 16 extra
    let mut state = initialize_state([Rv32ZcmpInstruction::CmPop {
        urlist: ZcmpUrlist::try_from_raw(4).unwrap(),
        stack_adj: 32,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    let sp_start = TEST_BASE_ADDR + 0x300;
    state.regs.write(Reg::Sp, sp_start);
    let new_sp = sp_start + 32;
    state
        .memory
        .write::<u32>(u64::from(new_sp - 4), 0xABCD)
        .unwrap();
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::Sp), new_sp);
    assert_eq!(state.regs.read(Reg::Ra), 0xABCD);
}

// CM.POPRETZ

#[test]
fn test_cm_popretz_zeros_a0_and_jumps() {
    let mut state = initialize_state([Rv32ZcmpInstruction::CmPopretz {
        urlist: ZcmpUrlist::try_from_raw(4).unwrap(),
        stack_adj: 16,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    let sp_start = TEST_BASE_ADDR + 0x300;
    state.regs.write(Reg::Sp, sp_start);
    state.regs.write(Reg::A0, 0xFFFF_FFFF);
    let new_sp = sp_start + 16;
    let return_addr = TEST_BASE_ADDR + 0x600;
    state
        .memory
        .write::<u32>(u64::from(new_sp - 4), return_addr)
        .unwrap();
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0);
    assert_eq!(state.instruction_fetcher.get_pc(), return_addr);
    assert_eq!(state.regs.read(Reg::Sp), new_sp);
}

#[test]
fn test_cm_popretz_clears_lsb_of_return_addr() {
    let mut state = initialize_state([Rv32ZcmpInstruction::CmPopretz {
        urlist: ZcmpUrlist::try_from_raw(4).unwrap(),
        stack_adj: 16,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    let sp_start = TEST_BASE_ADDR + 0x300;
    state.regs.write(Reg::Sp, sp_start);
    let new_sp = sp_start + 16;
    // Return address with LSB set (mode bit, must be cleared)
    state
        .memory
        .write::<u32>(u64::from(new_sp - 4), TEST_BASE_ADDR + 0x101)
        .unwrap();
    execute(&mut state).unwrap();
    assert_eq!(state.instruction_fetcher.get_pc(), TEST_BASE_ADDR + 0x100);
}

// CM.POPRET

#[test]
fn test_cm_popret_restores_and_jumps() {
    let mut state = initialize_state([Rv32ZcmpInstruction::CmPopret {
        urlist: ZcmpUrlist::try_from_raw(5).unwrap(),
        stack_adj: 16,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    let sp_start = TEST_BASE_ADDR + 0x300;
    state.regs.write(Reg::Sp, sp_start);
    let new_sp = sp_start + 16;
    let return_addr = TEST_BASE_ADDR + 0x500;
    state
        .memory
        .write::<u32>(u64::from(new_sp - 4), return_addr)
        .unwrap();
    state
        .memory
        .write::<u32>(u64::from(new_sp - 8), 0x9999)
        .unwrap();
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::Ra), return_addr);
    assert_eq!(state.regs.read(Reg::S0), 0x9999);
    assert_eq!(state.regs.read(Reg::Sp), new_sp);
    assert_eq!(state.instruction_fetcher.get_pc(), return_addr);
}

#[test]
fn test_cm_popret_clears_lsb_of_return_addr() {
    let mut state = initialize_state([Rv32ZcmpInstruction::CmPopret {
        urlist: ZcmpUrlist::try_from_raw(4).unwrap(),
        stack_adj: 16,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    let sp_start = TEST_BASE_ADDR + 0x300;
    state.regs.write(Reg::Sp, sp_start);
    let new_sp = sp_start + 16;
    // Return address with LSB set (mode bit, must be cleared)
    state
        .memory
        .write::<u32>(u64::from(new_sp - 4), TEST_BASE_ADDR + 0x101)
        .unwrap();
    execute(&mut state).unwrap();
    assert_eq!(state.instruction_fetcher.get_pc(), TEST_BASE_ADDR + 0x100);
}

// CM.MVA01S

#[test]
fn test_cm_mva01s_copies_to_a0_a1() {
    let mut state = initialize_state([Rv32ZcmpInstruction::CmMva01s {
        rs1: Reg::S2,
        rs2: Reg::S3,
    }]);
    state.regs.write(Reg::S2, 0x1111);
    state.regs.write(Reg::S3, 0x2222);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0x1111);
    assert_eq!(state.regs.read(Reg::A1), 0x2222);
    // Source registers must be unchanged
    assert_eq!(state.regs.read(Reg::S2), 0x1111);
    assert_eq!(state.regs.read(Reg::S3), 0x2222);
}

#[test]
fn test_cm_mva01s_reads_before_write() {
    // If rs1 or rs2 alias a0/a1, reads must occur before writes
    let mut state = initialize_state([Rv32ZcmpInstruction::CmMva01s {
        rs1: Reg::S0,
        rs2: Reg::S1,
    }]);
    state.regs.write(Reg::S0, 0xAAAA);
    state.regs.write(Reg::S1, 0xBBBB);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0xAAAA);
    assert_eq!(state.regs.read(Reg::A1), 0xBBBB);
}

// CM.MVSA01

#[test]
fn test_cm_mvsa01_copies_a0_a1_to_s_regs() {
    let mut state = initialize_state([Rv32ZcmpInstruction::CmMvsa01 {
        rs1: Reg::S4,
        rs2: Reg::S5,
    }]);
    state.regs.write(Reg::A0, 0x3333);
    state.regs.write(Reg::A1, 0x4444);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::S4), 0x3333);
    assert_eq!(state.regs.read(Reg::S5), 0x4444);
    // a0/a1 must be unchanged
    assert_eq!(state.regs.read(Reg::A0), 0x3333);
    assert_eq!(state.regs.read(Reg::A1), 0x4444);
}

// Push/Pop round-trip

#[test]
fn test_push_pop_round_trip_ra_s0_s1() {
    let sp_start = TEST_BASE_ADDR + 0x800;

    // PUSH {ra, s0, s1}
    let mut state = initialize_state([Rv32ZcmpInstruction::CmPush {
        urlist: ZcmpUrlist::try_from_raw(6).unwrap(),
        stack_adj: 16,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::Sp, sp_start);
    state.regs.write(Reg::Ra, 0xAAAA);
    state.regs.write(Reg::S0, 0xBBBB);
    state.regs.write(Reg::S1, 0xCCCC);
    execute(&mut state).unwrap();
    let sp_after_push = state.regs.read(Reg::Sp);
    assert_eq!(sp_after_push, sp_start - 16);

    // POP from the same memory
    let mut state2 = initialize_state([Rv32ZcmpInstruction::CmPop {
        urlist: ZcmpUrlist::try_from_raw(6).unwrap(),
        stack_adj: 16,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state2.regs.write(Reg::Sp, sp_after_push);
    for offset in 0u32..64 {
        let addr = sp_after_push + offset;
        if let Ok(v) = state.memory.read::<u32>(u64::from(addr)) {
            state2.memory.write::<u32>(u64::from(addr), v).unwrap();
        }
    }
    execute(&mut state2).unwrap();
    assert_eq!(state2.regs.read(Reg::Ra), 0xAAAA);
    assert_eq!(state2.regs.read(Reg::S0), 0xBBBB);
    assert_eq!(state2.regs.read(Reg::S1), 0xCCCC);
    assert_eq!(state2.regs.read(Reg::Sp), sp_start);
}

// Memory errors

#[test]
fn test_cm_push_oob_memory() {
    // sp very low -> sp-4 underflows into unmapped memory
    let mut state = initialize_state([Rv32ZcmpInstruction::CmPush {
        urlist: ZcmpUrlist::try_from_raw(4).unwrap(),
        stack_adj: 16,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::Sp, 2);
    let result = execute(&mut state);
    assert!(matches!(result, Err(ExecutionError::MemoryAccess(_))));
}
