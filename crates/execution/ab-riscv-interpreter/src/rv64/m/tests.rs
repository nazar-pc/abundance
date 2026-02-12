use crate::rv64::test_utils::{execute, initialize_state};
use ab_riscv_primitives::instructions::rv64::m::Rv64MInstruction;
use ab_riscv_primitives::registers::general_purpose::EReg;

// Multiplication Instructions

#[test]
fn test_mul() {
    let mut state = initialize_state([Rv64MInstruction::Mul {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 7);
    state.regs.write(EReg::A1, 8);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 56);
}

#[test]
fn test_mulh() {
    let mut state = initialize_state([Rv64MInstruction::Mulh {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, i64::MAX as u64);
    state.regs.write(EReg::A1, 2);

    execute(&mut state).unwrap();

    let (_, hi) = i64::MAX.widening_mul(2);
    assert_eq!(state.regs.read(EReg::A2), hi.cast_unsigned());
}

#[test]
fn test_mulhu() {
    let mut state = initialize_state([Rv64MInstruction::Mulhu {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, u64::MAX);
    state.regs.write(EReg::A1, u64::MAX);

    execute(&mut state).unwrap();

    let prod = (u64::MAX as u128) * (u64::MAX as u128);
    assert_eq!(state.regs.read(EReg::A2), (prod >> 64) as u64);
}

#[test]
fn test_mulhsu() {
    let mut state = initialize_state([Rv64MInstruction::Mulhsu {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, (-2i64).cast_unsigned());
    state.regs.write(EReg::A1, 3);

    execute(&mut state).unwrap();

    let prod = (-2i64 as i128) * (3i128);
    assert_eq!(
        state.regs.read(EReg::A2),
        (prod >> 64).cast_unsigned() as u64
    );
}

// Division Instructions

#[test]
fn test_div() {
    let mut state = initialize_state([Rv64MInstruction::Div {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 20);
    state.regs.write(EReg::A1, 3);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2).cast_signed(), 6);
}

#[test]
fn test_div_by_zero() {
    let mut state = initialize_state([Rv64MInstruction::Div {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 20);
    state.regs.write(EReg::A1, 0);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), (-1i64).cast_unsigned());
}

#[test]
fn test_div_overflow() {
    let mut state = initialize_state([Rv64MInstruction::Div {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, i64::MIN.cast_unsigned());
    state.regs.write(EReg::A1, (-1i64).cast_unsigned());

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), i64::MIN.cast_unsigned());
}

#[test]
fn test_divu() {
    let mut state = initialize_state([Rv64MInstruction::Divu {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 20);
    state.regs.write(EReg::A1, 3);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 6);
}

#[test]
fn test_divu_by_zero() {
    let mut state = initialize_state([Rv64MInstruction::Divu {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 20);
    state.regs.write(EReg::A1, 0);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), u64::MAX);
}

#[test]
fn test_rem() {
    let mut state = initialize_state([Rv64MInstruction::Rem {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 20);
    state.regs.write(EReg::A1, 3);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2).cast_signed(), 2);
}

#[test]
fn test_rem_by_zero() {
    let mut state = initialize_state([Rv64MInstruction::Rem {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 20);
    state.regs.write(EReg::A1, 0);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 20);
}

#[test]
fn test_rem_overflow() {
    let mut state = initialize_state([Rv64MInstruction::Rem {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, i64::MIN.cast_unsigned());
    state.regs.write(EReg::A1, (-1i64).cast_unsigned());

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 0);
}

#[test]
fn test_remu() {
    let mut state = initialize_state([Rv64MInstruction::Remu {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 20);
    state.regs.write(EReg::A1, 3);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 2);
}

#[test]
fn test_remu_by_zero() {
    let mut state = initialize_state([Rv64MInstruction::Remu {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 20);
    state.regs.write(EReg::A1, 0);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 20);
}

// RV64M instructions - operate on lower 32 bits and sign-extend

#[test]
fn test_mulw_basic() {
    let mut state = initialize_state([Rv64MInstruction::Mulw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 7);
    state.regs.write(EReg::A1, 8);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 56);
}

#[test]
fn test_mulw_overflow() {
    let mut state = initialize_state([Rv64MInstruction::Mulw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    // 0x7FFFFFFF * 2 = 0xFFFFFFFE (as i32) = -2
    state.regs.write(EReg::A0, 0x7FFF_FFFF);
    state.regs.write(EReg::A1, 2);

    execute(&mut state).unwrap();

    // Result should be sign-extended: 0xFFFFFFFFFFFFFFFE
    assert_eq!(state.regs.read(EReg::A2), 0xFFFF_FFFF_FFFF_FFFE);
}

#[test]
fn test_mulw_negative() {
    let mut state = initialize_state([Rv64MInstruction::Mulw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, (-3i32).cast_unsigned() as u64);
    state.regs.write(EReg::A1, 4);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), (-12i64).cast_unsigned());
}

#[test]
fn test_mulw_ignores_upper_bits() {
    let mut state = initialize_state([Rv64MInstruction::Mulw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    // Upper 32 bits should be ignored
    state.regs.write(EReg::A0, 0xDEAD_BEEF_0000_0007);
    state.regs.write(EReg::A1, 0xCAFE_BABE_0000_0008);

    execute(&mut state).unwrap();

    // 7 * 8 = 56
    assert_eq!(state.regs.read(EReg::A2), 56);
}

#[test]
fn test_divw_basic() {
    let mut state = initialize_state([Rv64MInstruction::Divw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 20);
    state.regs.write(EReg::A1, 3);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 6);
}

#[test]
fn test_divw_negative() {
    let mut state = initialize_state([Rv64MInstruction::Divw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, (-20i32).cast_unsigned() as u64);
    state.regs.write(EReg::A1, 3);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), (-6i64).cast_unsigned());
}

#[test]
fn test_divw_by_zero() {
    let mut state = initialize_state([Rv64MInstruction::Divw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 20);
    state.regs.write(EReg::A1, 0);

    execute(&mut state).unwrap();

    // Division by zero returns -1 (sign-extended)
    assert_eq!(state.regs.read(EReg::A2), (-1i64).cast_unsigned());
}

#[test]
fn test_divw_overflow() {
    let mut state = initialize_state([Rv64MInstruction::Divw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, i32::MIN.cast_unsigned() as u64);
    state.regs.write(EReg::A1, (-1i32).cast_unsigned() as u64);

    execute(&mut state).unwrap();

    // Overflow case: returns i32::MIN sign-extended
    assert_eq!(state.regs.read(EReg::A2), (i32::MIN as i64).cast_unsigned());
}

#[test]
fn test_divw_ignores_upper_bits() {
    let mut state = initialize_state([Rv64MInstruction::Divw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    // Upper 32 bits should be ignored
    state.regs.write(EReg::A0, 0xDEAD_BEEF_0000_0014); // 20 in lower 32 bits
    state.regs.write(EReg::A1, 0xCAFE_BABE_0000_0003); // 3 in lower 32 bits

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 6);
}

#[test]
fn test_divuw_basic() {
    let mut state = initialize_state([Rv64MInstruction::Divuw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 20);
    state.regs.write(EReg::A1, 3);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 6);
}

#[test]
fn test_divuw_large_unsigned() {
    let mut state = initialize_state([Rv64MInstruction::Divuw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 0xFFFF_FFFF); // u32::MAX
    state.regs.write(EReg::A1, 2);

    execute(&mut state).unwrap();

    // 0xFFFFFFFF / 2 = 0x7FFFFFFF (sign-extended to 64-bit)
    assert_eq!(state.regs.read(EReg::A2), 0x0000_0000_7FFF_FFFF);
}

#[test]
fn test_divuw_by_zero() {
    let mut state = initialize_state([Rv64MInstruction::Divuw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 20);
    state.regs.write(EReg::A1, 0);

    execute(&mut state).unwrap();

    // Division by zero returns u32::MAX sign-extended
    assert_eq!(state.regs.read(EReg::A2), 0xFFFF_FFFF_FFFF_FFFF);
}

#[test]
fn test_divuw_ignores_upper_bits() {
    let mut state = initialize_state([Rv64MInstruction::Divuw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    // Upper 32 bits should be ignored
    state.regs.write(EReg::A0, 0xDEAD_BEEF_0000_0064); // 100 in lower 32 bits
    state.regs.write(EReg::A1, 0xCAFE_BABE_0000_0005); // 5 in lower 32 bits

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 20);
}

#[test]
fn test_remw_basic() {
    let mut state = initialize_state([Rv64MInstruction::Remw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 20);
    state.regs.write(EReg::A1, 3);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 2);
}

#[test]
fn test_remw_negative_dividend() {
    let mut state = initialize_state([Rv64MInstruction::Remw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, (-20i32).cast_unsigned() as u64);
    state.regs.write(EReg::A1, 3);

    execute(&mut state).unwrap();

    // -20 % 3 = -2 (sign-extended)
    assert_eq!(state.regs.read(EReg::A2), (-2i64).cast_unsigned());
}

#[test]
fn test_remw_negative_divisor() {
    let mut state = initialize_state([Rv64MInstruction::Remw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 20);
    state.regs.write(EReg::A1, (-3i32).cast_unsigned() as u64);

    execute(&mut state).unwrap();

    // 20 % -3 = 2
    assert_eq!(state.regs.read(EReg::A2), 2);
}

#[test]
fn test_remw_by_zero() {
    let mut state = initialize_state([Rv64MInstruction::Remw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 20);
    state.regs.write(EReg::A1, 0);

    execute(&mut state).unwrap();

    // Remainder by zero returns dividend
    assert_eq!(state.regs.read(EReg::A2), 20);
}

#[test]
fn test_remw_overflow() {
    let mut state = initialize_state([Rv64MInstruction::Remw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, i32::MIN.cast_unsigned() as u64);
    state.regs.write(EReg::A1, (-1i32).cast_unsigned() as u64);

    execute(&mut state).unwrap();

    // Overflow case: returns 0
    assert_eq!(state.regs.read(EReg::A2), 0);
}

#[test]
fn test_remw_ignores_upper_bits() {
    let mut state = initialize_state([Rv64MInstruction::Remw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    // Upper 32 bits should be ignored
    state.regs.write(EReg::A0, 0xDEAD_BEEF_0000_0017); // 23 in lower 32 bits
    state.regs.write(EReg::A1, 0xCAFE_BABE_0000_0005); // 5 in lower 32 bits

    execute(&mut state).unwrap();

    // 23 % 5 = 3
    assert_eq!(state.regs.read(EReg::A2), 3);
}

#[test]
fn test_remuw_basic() {
    let mut state = initialize_state([Rv64MInstruction::Remuw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 20);
    state.regs.write(EReg::A1, 3);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 2);
}

#[test]
fn test_remuw_large_unsigned() {
    let mut state = initialize_state([Rv64MInstruction::Remuw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 0xFFFF_FFFF); // u32::MAX
    state.regs.write(EReg::A1, 10);

    execute(&mut state).unwrap();

    // 0xFFFFFFFF % 10 = 5 (sign-extended)
    assert_eq!(state.regs.read(EReg::A2), 5);
}

#[test]
fn test_remuw_by_zero() {
    let mut state = initialize_state([Rv64MInstruction::Remuw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 20);
    state.regs.write(EReg::A1, 0);

    execute(&mut state).unwrap();

    // Remainder by zero returns dividend
    assert_eq!(state.regs.read(EReg::A2), 20);
}

#[test]
fn test_remuw_ignores_upper_bits() {
    let mut state = initialize_state([Rv64MInstruction::Remuw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    // Upper 32 bits should be ignored
    state.regs.write(EReg::A0, 0xDEAD_BEEF_0000_0064); // 100 in lower 32 bits
    state.regs.write(EReg::A1, 0xCAFE_BABE_0000_0007); // 7 in lower 32 bits

    execute(&mut state).unwrap();

    // 100 % 7 = 2
    assert_eq!(state.regs.read(EReg::A2), 2);
}

#[test]
fn test_remuw_negative_as_unsigned() {
    let mut state = initialize_state([Rv64MInstruction::Remuw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    // Set value that is negative as i32 but positive as u32
    state.regs.write(EReg::A0, 0xFFFF_FFFF); // -1 as i32, u32::MAX as u32
    state.regs.write(EReg::A1, 2);

    execute(&mut state).unwrap();

    // 0xFFFFFFFF % 2 = 1 (sign-extended)
    assert_eq!(state.regs.read(EReg::A2), 1);
}

// Combined RV64M Tests

#[test]
fn test_mulw_divw_combination() {
    let mut state = initialize_state([
        // A2 = 7 * 8 = 56
        Rv64MInstruction::Mulw {
            rd: EReg::A2,
            rs1: EReg::A0,
            rs2: EReg::A1,
        },
        // A3 = 56 / 7 = 8
        Rv64MInstruction::Divw {
            rd: EReg::A3,
            rs1: EReg::A2,
            rs2: EReg::A0,
        },
    ]);

    state.regs.write(EReg::A0, 7);
    state.regs.write(EReg::A1, 8);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 56);
    assert_eq!(state.regs.read(EReg::A3), 8);
}

#[test]
fn test_divw_remw_combination() {
    let mut state = initialize_state([
        // A2 = 23 / 5 = 4
        Rv64MInstruction::Divw {
            rd: EReg::A2,
            rs1: EReg::A0,
            rs2: EReg::A1,
        },
        // A3 = 23 % 5 = 3
        Rv64MInstruction::Remw {
            rd: EReg::A3,
            rs1: EReg::A0,
            rs2: EReg::A1,
        },
    ]);

    state.regs.write(EReg::A0, 23);
    state.regs.write(EReg::A1, 5);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 4);
    assert_eq!(state.regs.read(EReg::A3), 3);
}

#[test]
fn test_rv64m_zero_register() {
    let mut state = initialize_state([
        // Should not modify zero register
        Rv64MInstruction::Mulw {
            rd: EReg::Zero,
            rs1: EReg::A0,
            rs2: EReg::A0,
        },
        // Reading from zero should give 0
        Rv64MInstruction::Mulw {
            rd: EReg::A1,
            rs1: EReg::Zero,
            rs2: EReg::A0,
        },
    ]);

    state.regs.write(EReg::A0, 42);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::Zero), 0);
    assert_eq!(state.regs.read(EReg::A1), 0);
}
