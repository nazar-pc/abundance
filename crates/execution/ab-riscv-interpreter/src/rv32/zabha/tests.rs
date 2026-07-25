use crate::rv32::test_utils::{TEST_BASE_ADDR, execute, initialize_state};
use crate::{RegisterFile, VirtualMemory};
use ab_riscv_primitives::prelude::*;

#[test]
fn test_amoswap_b_sign_extends() {
    let mut state = initialize_state([Rv32ZabhaInstruction::AmoswapB {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u8>(u64::from(addr), 0x80).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 0x42);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xFFFF_FF80);
    assert_eq!(state.memory.read::<u8>(u64::from(addr)).unwrap(), 0x42);
}

#[test]
fn test_amoadd_h() {
    let mut state = initialize_state([Rv32ZabhaInstruction::AmoaddH {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u16>(u64::from(addr), 10).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 10);
    assert_eq!(state.memory.read::<u16>(u64::from(addr)).unwrap(), 42);
}

#[test]
fn test_amoadd_h_wraps_at_16_bits() {
    let mut state = initialize_state([Rv32ZabhaInstruction::AmoaddH {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state
        .memory
        .write::<u16>(u64::from(addr), u16::MAX)
        .unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 1);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xFFFF_FFFF);
    assert_eq!(state.memory.read::<u16>(u64::from(addr)).unwrap(), 0);
}

#[test]
fn test_amoxor_b() {
    let mut state = initialize_state([Rv32ZabhaInstruction::AmoxorB {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u8>(u64::from(addr), 0b1010).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 0b0110);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0b1010);
    assert_eq!(state.memory.read::<u8>(u64::from(addr)).unwrap(), 0b1100);
}

#[test]
fn test_amomin_b_signed_comparison() {
    let mut state = initialize_state([Rv32ZabhaInstruction::AmominB {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state
        .memory
        .write::<u8>(u64::from(addr), (-5i8).cast_unsigned())
        .unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 3);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xFFFF_FFFB);
    assert_eq!(
        state.memory.read::<u8>(u64::from(addr)).unwrap(),
        (-5i8).cast_unsigned()
    );
}

#[test]
fn test_amominu_h_unsigned_comparison() {
    let mut state = initialize_state([Rv32ZabhaInstruction::AmominuH {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state
        .memory
        .write::<u16>(u64::from(addr), (-5i16).cast_unsigned())
        .unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 3);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), i32::from(-5i16).cast_unsigned());
    assert_eq!(state.memory.read::<u16>(u64::from(addr)).unwrap(), 3);
}

#[test]
fn test_amocas_b_succeeds_on_match() {
    let mut state = initialize_state([Rv32ZabhaInstruction::AmocasB {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u8>(u64::from(addr), 111).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A2, 111);
    state.regs.write(Reg::A1, 42);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 111);
    assert_eq!(state.memory.read::<u8>(u64::from(addr)).unwrap(), 42);
}

#[test]
fn test_amocas_h_fails_on_mismatch() {
    let mut state = initialize_state([Rv32ZabhaInstruction::AmocasH {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u16>(u64::from(addr), 111).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A2, 999);
    state.regs.write(Reg::A1, 42);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 111);
    assert_eq!(state.memory.read::<u16>(u64::from(addr)).unwrap(), 111);
}

#[test]
fn test_amoadd_w_inherited_from_zaamo() {
    let mut state = initialize_state([Rv32ZabhaInstruction::Amoadd {
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
