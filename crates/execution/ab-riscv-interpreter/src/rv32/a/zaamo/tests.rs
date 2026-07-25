use crate::rv32::test_utils::{TEST_BASE_ADDR, execute, initialize_state};
use crate::{RegisterFile, VirtualMemory};
use ab_riscv_primitives::prelude::*;

#[test]
fn test_amoswap() {
    let mut state = initialize_state([Rv32ZaamoInstruction::Amoswap {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u32>(u64::from(addr), 111).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 222);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 111);
    assert_eq!(state.memory.read::<u32>(u64::from(addr)).unwrap(), 222);
}

#[test]
fn test_amoadd() {
    let mut state = initialize_state([Rv32ZaamoInstruction::Amoadd {
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

#[test]
fn test_amoadd_wraps() {
    let mut state = initialize_state([Rv32ZaamoInstruction::Amoadd {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state
        .memory
        .write::<u32>(u64::from(addr), u32::MAX)
        .unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 1);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), u32::MAX);
    assert_eq!(state.memory.read::<u32>(u64::from(addr)).unwrap(), 0);
}

#[test]
fn test_amoxor() {
    let mut state = initialize_state([Rv32ZaamoInstruction::Amoxor {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u32>(u64::from(addr), 0b1010).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 0b0110);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0b1010);
    assert_eq!(state.memory.read::<u32>(u64::from(addr)).unwrap(), 0b1100);
}

#[test]
fn test_amoand() {
    let mut state = initialize_state([Rv32ZaamoInstruction::Amoand {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u32>(u64::from(addr), 0b1010).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 0b0110);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0b1010);
    assert_eq!(state.memory.read::<u32>(u64::from(addr)).unwrap(), 0b0010);
}

#[test]
fn test_amoor() {
    let mut state = initialize_state([Rv32ZaamoInstruction::Amoor {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u32>(u64::from(addr), 0b1010).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 0b0110);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0b1010);
    assert_eq!(state.memory.read::<u32>(u64::from(addr)).unwrap(), 0b1110);
}

#[test]
fn test_amomin() {
    let mut state = initialize_state([Rv32ZaamoInstruction::Amomin {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state
        .memory
        .write::<u32>(u64::from(addr), (-5i32).cast_unsigned())
        .unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 3);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), (-5i32).cast_unsigned());
    assert_eq!(
        state.memory.read::<u32>(u64::from(addr)).unwrap(),
        (-5i32).cast_unsigned()
    );
}

#[test]
fn test_amomax() {
    let mut state = initialize_state([Rv32ZaamoInstruction::Amomax {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state
        .memory
        .write::<u32>(u64::from(addr), (-5i32).cast_unsigned())
        .unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 3);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), (-5i32).cast_unsigned());
    assert_eq!(state.memory.read::<u32>(u64::from(addr)).unwrap(), 3);
}

#[test]
fn test_amominu_treats_operands_as_unsigned() {
    let mut state = initialize_state([Rv32ZaamoInstruction::Amominu {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    // As unsigned, this is a very large number, larger than `3`
    state
        .memory
        .write::<u32>(u64::from(addr), (-5i32).cast_unsigned())
        .unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 3);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), (-5i32).cast_unsigned());
    assert_eq!(state.memory.read::<u32>(u64::from(addr)).unwrap(), 3);
}

#[test]
fn test_amomaxu_treats_operands_as_unsigned() {
    let mut state = initialize_state([Rv32ZaamoInstruction::Amomaxu {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state
        .memory
        .write::<u32>(u64::from(addr), (-5i32).cast_unsigned())
        .unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 3);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), (-5i32).cast_unsigned());
    assert_eq!(
        state.memory.read::<u32>(u64::from(addr)).unwrap(),
        (-5i32).cast_unsigned()
    );
}

#[test]
fn test_amoswap_supports_misaligned_access() {
    let mut state = initialize_state([Rv32ZaamoInstruction::Amoswap {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
        aq: false,
        rl: false,
    }]);
    // Not 4-byte aligned
    let addr = TEST_BASE_ADDR + 0x101;
    state.memory.write::<u32>(u64::from(addr), 111).unwrap();
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 222);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 111);
    assert_eq!(state.memory.read::<u32>(u64::from(addr)).unwrap(), 222);
}
