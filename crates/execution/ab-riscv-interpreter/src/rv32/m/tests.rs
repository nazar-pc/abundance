use crate::RegisterFile;
use crate::rv32::test_utils::{execute, initialize_state};
use ab_riscv_primitives::prelude::*;
// Multiplication Instructions

#[test]
fn test_mul() {
    let mut state = initialize_state([Rv32MInstruction::Mul {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 7);
    state.regs.write(Reg::A1, 8);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 56);
}

#[test]
fn test_mul_overflow() {
    let mut state = initialize_state([Rv32MInstruction::Mul {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    // Lower 32 bits of 0x7FFFFFFF * 2 = 0xFFFFFFFE
    state.regs.write(Reg::A0, 0x7FFF_FFFF);
    state.regs.write(Reg::A1, 2);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xFFFF_FFFE);
}

#[test]
fn test_mulh() {
    let mut state = initialize_state([Rv32MInstruction::Mulh {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, i32::MAX as u32);
    state.regs.write(Reg::A1, 2);

    execute(&mut state).unwrap();

    let prod = i64::from(i32::MAX) * 2i64;
    assert_eq!(
        state.regs.read(Reg::A2),
        ((prod >> 32u8) as i32).cast_unsigned()
    );
}

#[test]
fn test_mulh_negative() {
    let mut state = initialize_state([Rv32MInstruction::Mulh {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, (-1i32).cast_unsigned());
    state.regs.write(Reg::A1, (-1i32).cast_unsigned());

    execute(&mut state).unwrap();

    // (-1) * (-1) = 1, upper 32 bits = 0
    assert_eq!(state.regs.read(Reg::A2), 0);
}

#[test]
fn test_mulhu() {
    let mut state = initialize_state([Rv32MInstruction::Mulhu {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, u32::MAX);
    state.regs.write(Reg::A1, u32::MAX);

    execute(&mut state).unwrap();

    let prod = u64::from(u32::MAX) * u64::from(u32::MAX);
    assert_eq!(state.regs.read(Reg::A2), (prod >> 32u8) as u32);
}

#[test]
fn test_mulhsu() {
    let mut state = initialize_state([Rv32MInstruction::Mulhsu {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, (-2i32).cast_unsigned());
    state.regs.write(Reg::A1, 3);

    execute(&mut state).unwrap();

    let prod = i64::from(-2i32) * (3i64);
    assert_eq!(
        state.regs.read(Reg::A2),
        ((prod >> 32u8) as i32).cast_unsigned()
    );
}

// Division Instructions

#[test]
fn test_div() {
    let mut state = initialize_state([Rv32MInstruction::Div {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 20);
    state.regs.write(Reg::A1, 3);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2).cast_signed(), 6);
}

#[test]
fn test_div_negative() {
    let mut state = initialize_state([Rv32MInstruction::Div {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, (-20i32).cast_unsigned());
    state.regs.write(Reg::A1, 3);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2).cast_signed(), -6);
}

#[test]
fn test_div_by_zero() {
    let mut state = initialize_state([Rv32MInstruction::Div {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 20);
    state.regs.write(Reg::A1, 0);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), (-1i32).cast_unsigned());
}

#[test]
fn test_div_overflow() {
    let mut state = initialize_state([Rv32MInstruction::Div {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, i32::MIN.cast_unsigned());
    state.regs.write(Reg::A1, (-1i32).cast_unsigned());

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), i32::MIN.cast_unsigned());
}

#[test]
fn test_divu() {
    let mut state = initialize_state([Rv32MInstruction::Divu {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 20);
    state.regs.write(Reg::A1, 3);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 6);
}

#[test]
fn test_divu_by_zero() {
    let mut state = initialize_state([Rv32MInstruction::Divu {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 20);
    state.regs.write(Reg::A1, 0);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), u32::MAX);
}

#[test]
fn test_rem() {
    let mut state = initialize_state([Rv32MInstruction::Rem {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 20);
    state.regs.write(Reg::A1, 3);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2).cast_signed(), 2);
}

#[test]
fn test_rem_negative_dividend() {
    let mut state = initialize_state([Rv32MInstruction::Rem {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, (-20i32).cast_unsigned());
    state.regs.write(Reg::A1, 3);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2).cast_signed(), -2);
}

#[test]
fn test_rem_by_zero() {
    let mut state = initialize_state([Rv32MInstruction::Rem {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 20);
    state.regs.write(Reg::A1, 0);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 20);
}

#[test]
fn test_rem_overflow() {
    let mut state = initialize_state([Rv32MInstruction::Rem {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, i32::MIN.cast_unsigned());
    state.regs.write(Reg::A1, (-1i32).cast_unsigned());

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0);
}

#[test]
fn test_remu() {
    let mut state = initialize_state([Rv32MInstruction::Remu {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 20);
    state.regs.write(Reg::A1, 3);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 2);
}

#[test]
fn test_remu_large_unsigned() {
    let mut state = initialize_state([Rv32MInstruction::Remu {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, u32::MAX);
    state.regs.write(Reg::A1, 10);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), u32::MAX % 10);
}

#[test]
fn test_remu_by_zero() {
    let mut state = initialize_state([Rv32MInstruction::Remu {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 20);
    state.regs.write(Reg::A1, 0);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 20);
}

// Combined Tests

#[test]
fn test_div_rem_combination() {
    let mut state = initialize_state([
        // A2 = 23 / 5 = 4
        Rv32MInstruction::Div {
            rd: Reg::A2,
            rs1: Reg::A0,
            rs2: Reg::A1,
        },
        // A3 = 23 % 5 = 3
        Rv32MInstruction::Rem {
            rd: Reg::A3,
            rs1: Reg::A0,
            rs2: Reg::A1,
        },
    ]);

    state.regs.write(Reg::A0, 23);
    state.regs.write(Reg::A1, 5);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 4);
    assert_eq!(state.regs.read(Reg::A3), 3);
}

#[test]
fn test_zero_register() {
    let mut state = initialize_state([
        // Should not modify zero register
        Rv32MInstruction::Mul {
            rd: Reg::Zero,
            rs1: Reg::A0,
            rs2: Reg::A0,
        },
        // Reading from zero should give 0
        Rv32MInstruction::Mul {
            rd: Reg::A1,
            rs1: Reg::Zero,
            rs2: Reg::A0,
        },
    ]);

    state.regs.write(Reg::A0, 42);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::Zero), 0);
    assert_eq!(state.regs.read(Reg::A1), 0);
}
