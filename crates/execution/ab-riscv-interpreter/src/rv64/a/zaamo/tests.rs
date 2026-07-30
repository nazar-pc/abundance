use crate::rv64::test_utils::{TEST_BASE_ADDR, execute, initialize_state};
use crate::{RegisterFile, VirtualMemory};
use ab_riscv_primitives::prelude::*;

#[test]
fn test_amoswap_w_sign_extends() {
    let mut state = initialize_state([Rv64ZaamoInstruction::Amoswap {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u32>(addr, 0x8000_0000).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 222);

    execute(&mut state).unwrap();

    // Old 32-bit value is sign-extended to 64 bits
    assert_eq!(state.regs.read(Reg::A2), 0xFFFF_FFFF_8000_0000);
    assert_eq!(state.memory.read::<u32>(addr).unwrap(), 222);
}

#[test]
fn test_amoadd_w_operates_on_32_bits() {
    let mut state = initialize_state([Rv64ZaamoInstruction::Amoadd {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u32>(addr, u32::MAX).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 1);

    execute(&mut state).unwrap();

    // Old value `0xFFFF_FFFF` is sign-extended to `0xFFFF_FFFF_FFFF_FFFF` (i.e. `-1i64`)
    assert_eq!(state.regs.read(Reg::A2), u64::MAX);
    // Result wraps at 32 bits, not 64
    assert_eq!(state.memory.read::<u32>(addr).unwrap(), 0);
}

#[test]
fn test_amomin_w_uses_32_bit_signed_comparison() {
    let mut state = initialize_state([Rv64ZaamoInstruction::Amomin {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state
        .memory
        .write::<u32>(addr, (-5i32).cast_unsigned())
        .unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 3);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xFFFF_FFFF_FFFF_FFFB);
    assert_eq!(
        state.memory.read::<u32>(addr).unwrap(),
        (-5i32).cast_unsigned()
    );
}

#[test]
fn test_amoswap_d() {
    let mut state = initialize_state([Rv64ZaamoInstruction::AmoswapD {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state
        .memory
        .write::<u64>(addr, 0x1111_1111_1111_1111)
        .unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 0x2222_2222_2222_2222);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0x1111_1111_1111_1111);
    assert_eq!(
        state.memory.read::<u64>(addr).unwrap(),
        0x2222_2222_2222_2222
    );
}

#[test]
fn test_amoadd_d() {
    let mut state = initialize_state([Rv64ZaamoInstruction::AmoaddD {
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
fn test_amoadd_d_wraps() {
    let mut state = initialize_state([Rv64ZaamoInstruction::AmoaddD {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u64>(addr, u64::MAX).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 1);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), u64::MAX);
    assert_eq!(state.memory.read::<u64>(addr).unwrap(), 0);
}

#[test]
fn test_amomaxu_d_unsigned_comparison() {
    let mut state = initialize_state([Rv64ZaamoInstruction::AmomaxuD {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state
        .memory
        .write::<u64>(addr, (-5i64).cast_unsigned())
        .unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 3);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), (-5i64).cast_unsigned());
    assert_eq!(
        state.memory.read::<u64>(addr).unwrap(),
        (-5i64).cast_unsigned()
    );
}

#[test]
fn test_amoxor_d_supports_misaligned_access() {
    let mut state = initialize_state([Rv64ZaamoInstruction::AmoxorD {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    // Not 8-byte aligned
    let addr = TEST_BASE_ADDR + 0x101;
    state.memory.write::<u64>(addr, 0b1010).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 0b0110);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0b1010);
    assert_eq!(state.memory.read::<u64>(addr).unwrap(), 0b1100);
}
