use crate::RegisterFile;
use crate::rv32::test_utils::{execute, initialize_state};
use ab_riscv_primitives::prelude::*;

#[test]
fn test_sh1add() {
    let mut state = initialize_state([Rv32ZbaInstruction::Sh1add {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 10);
    state.regs.write(Reg::A1, 100);

    execute(&mut state).unwrap();

    // (10 << 1) + 100 = 20 + 100 = 120
    assert_eq!(state.regs.read(Reg::A2), 120);
}

#[test]
fn test_sh2add() {
    let mut state = initialize_state([Rv32ZbaInstruction::Sh2add {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 10);
    state.regs.write(Reg::A1, 100);

    execute(&mut state).unwrap();

    // (10 << 2) + 100 = 40 + 100 = 140
    assert_eq!(state.regs.read(Reg::A2), 140);
}

#[test]
fn test_sh3add() {
    let mut state = initialize_state([Rv32ZbaInstruction::Sh3add {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 10);
    state.regs.write(Reg::A1, 100);

    execute(&mut state).unwrap();

    // (10 << 3) + 100 = 80 + 100 = 180
    assert_eq!(state.regs.read(Reg::A2), 180);
}

#[test]
fn test_sh1add_overflow() {
    let mut state = initialize_state([Rv32ZbaInstruction::Sh1add {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0x8000_0000u32);
    state.regs.write(Reg::A1, 1);

    execute(&mut state).unwrap();

    // (0x8000_0000 << 1) wraps to 0, then + 1 = 1
    assert_eq!(state.regs.read(Reg::A2), 1);
}
