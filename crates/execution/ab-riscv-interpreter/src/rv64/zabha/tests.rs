use crate::rv64::test_utils::{TEST_BASE_ADDR, execute, initialize_state};
use crate::{ExecutionError, RegisterFile, VirtualMemory};
use ab_riscv_primitives::prelude::*;
use core::assert_matches;

#[test]
fn test_amoswap_b_sign_extends_to_64_bits() {
    let mut state = initialize_state([Rv64ZabhaInstruction::AmoswapB {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u8>(addr, 0x80).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 0x42);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xFFFF_FFFF_FFFF_FF80);
    assert_eq!(state.memory.read::<u8>(addr).unwrap(), 0x42);
}

#[test]
fn test_amoadd_h() {
    let mut state = initialize_state([Rv64ZabhaInstruction::AmoaddH {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u16>(addr, 10).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 10);
    assert_eq!(state.memory.read::<u16>(addr).unwrap(), 42);
}

#[test]
fn test_amominu_b_unsigned_comparison() {
    let mut state = initialize_state([Rv64ZabhaInstruction::AmominuB {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state
        .memory
        .write::<u8>(addr, (-5i8).cast_unsigned())
        .unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 3);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), i64::from(-5i8).cast_unsigned());
    assert_eq!(state.memory.read::<u8>(addr).unwrap(), 3);
}

#[test]
fn test_amocas_h_succeeds_on_match() {
    let mut state = initialize_state([Rv64ZabhaInstruction::AmocasH {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u16>(addr, 111).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A2, 111);
    state.regs.write(Reg::A1, 42);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 111);
    assert_eq!(state.memory.read::<u16>(addr).unwrap(), 42);
}

#[test]
fn test_amocas_b_fails_on_mismatch() {
    let mut state = initialize_state([Rv64ZabhaInstruction::AmocasB {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u8>(addr, 111).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A2, 999);
    state.regs.write(Reg::A1, 42);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 111);
    assert_eq!(state.memory.read::<u8>(addr).unwrap(), 111);
}

#[test]
fn test_amoadd_d_inherited_from_zaamo() {
    let mut state = initialize_state([Rv64ZabhaInstruction::AmoaddD {
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
fn test_amoadd_h_rejects_misaligned_atomicity_granule_crossing() {
    let mut state = initialize_state([Rv64ZabhaInstruction::AmoaddH {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    // 1 byte before a 4096-byte misaligned atomicity granule boundary: the 2-byte access
    // straddles it
    let addr = TEST_BASE_ADDR + 0xfff;
    state.regs.write(Reg::A0, addr);

    assert_matches!(
        execute(&mut state),
        Err(ExecutionError::MisalignedAtomic { .. })
    );
}
