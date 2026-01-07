extern crate alloc;

use crate::m_64_ext::execute_m_64_ext;
use ab_riscv_primitives::instruction::m_64_ext::M64ExtInstruction;
use ab_riscv_primitives::registers::{EReg, Registers};
use alloc::vec;

// Multiplication Instructions

#[test]
fn test_mul() {
    let mut regs = Registers::<EReg<u64>>::default();

    regs.write(EReg::A0, 7);
    regs.write(EReg::A1, 8);

    let instructions = vec![M64ExtInstruction::Mul {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg::A2), 56);
}

#[test]
fn test_mulh() {
    let mut regs = Registers::<EReg<u64>>::default();

    regs.write(EReg::A0, i64::MAX as u64);
    regs.write(EReg::A1, 2);

    let instructions = vec![M64ExtInstruction::Mulh {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    let (_, hi) = i64::MAX.widening_mul(2);
    assert_eq!(regs.read(EReg::A2), hi.cast_unsigned());
}

#[test]
fn test_mulhu() {
    let mut regs = Registers::<EReg<u64>>::default();

    regs.write(EReg::A0, u64::MAX);
    regs.write(EReg::A1, u64::MAX);

    let instructions = vec![M64ExtInstruction::Mulhu {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    let prod = (u64::MAX as u128) * (u64::MAX as u128);
    assert_eq!(regs.read(EReg::A2), (prod >> 64) as u64);
}

#[test]
fn test_mulhsu() {
    let mut regs = Registers::<EReg<u64>>::default();

    regs.write(EReg::A0, (-2i64).cast_unsigned());
    regs.write(EReg::A1, 3);

    let instructions = vec![M64ExtInstruction::Mulhsu {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    let prod = (-2i64 as i128) * (3i128);
    assert_eq!(regs.read(EReg::A2), (prod >> 64).cast_unsigned() as u64);
}

// Division Instructions

#[test]
fn test_div() {
    let mut regs = Registers::<EReg<u64>>::default();

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, 3);

    let instructions = vec![M64ExtInstruction::Div {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg::A2).cast_signed(), 6);
}

#[test]
fn test_div_by_zero() {
    let mut regs = Registers::<EReg<u64>>::default();

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, 0);

    let instructions = vec![M64ExtInstruction::Div {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg::A2), (-1i64).cast_unsigned());
}

#[test]
fn test_div_overflow() {
    let mut regs = Registers::<EReg<u64>>::default();

    regs.write(EReg::A0, i64::MIN.cast_unsigned());
    regs.write(EReg::A1, (-1i64).cast_unsigned());

    let instructions = vec![M64ExtInstruction::Div {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg::A2), i64::MIN.cast_unsigned());
}

#[test]
fn test_divu() {
    let mut regs = Registers::<EReg<u64>>::default();

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, 3);

    let instructions = vec![M64ExtInstruction::Divu {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg::A2), 6);
}

#[test]
fn test_divu_by_zero() {
    let mut regs = Registers::<EReg<u64>>::default();

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, 0);

    let instructions = vec![M64ExtInstruction::Divu {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg::A2), u64::MAX);
}

#[test]
fn test_rem() {
    let mut regs = Registers::<EReg<u64>>::default();

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, 3);

    let instructions = vec![M64ExtInstruction::Rem {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg::A2).cast_signed(), 2);
}

#[test]
fn test_rem_by_zero() {
    let mut regs = Registers::<EReg<u64>>::default();

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, 0);

    let instructions = vec![M64ExtInstruction::Rem {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg::A2), 20);
}

#[test]
fn test_rem_overflow() {
    let mut regs = Registers::<EReg<u64>>::default();

    regs.write(EReg::A0, i64::MIN.cast_unsigned());
    regs.write(EReg::A1, (-1i64).cast_unsigned());

    let instructions = vec![M64ExtInstruction::Rem {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg::A2), 0);
}

#[test]
fn test_remu() {
    let mut regs = Registers::<EReg<u64>>::default();

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, 3);

    let instructions = vec![M64ExtInstruction::Remu {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg::A2), 2);
}

#[test]
fn test_remu_by_zero() {
    let mut regs = Registers::<EReg<u64>>::default();

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, 0);

    let instructions = vec![M64ExtInstruction::Remu {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg::A2), 20);
}

// RV64M instructions - operate on lower 32 bits and sign-extend

#[test]
fn test_mulw_basic() {
    let mut regs = Registers::<EReg<u64>>::default();

    regs.write(EReg::A0, 7);
    regs.write(EReg::A1, 8);

    let instructions = vec![M64ExtInstruction::Mulw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg::A2), 56);
}

#[test]
fn test_mulw_overflow() {
    let mut regs = Registers::<EReg<u64>>::default();

    // 0x7FFFFFFF * 2 = 0xFFFFFFFE (as i32) = -2
    regs.write(EReg::A0, 0x7FFF_FFFF);
    regs.write(EReg::A1, 2);

    let instructions = vec![M64ExtInstruction::Mulw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    // Result should be sign-extended: 0xFFFFFFFFFFFFFFFE
    assert_eq!(regs.read(EReg::A2), 0xFFFF_FFFF_FFFF_FFFE);
}

#[test]
fn test_mulw_negative() {
    let mut regs = Registers::<EReg<u64>>::default();

    regs.write(EReg::A0, (-3i32).cast_unsigned() as u64);
    regs.write(EReg::A1, 4);

    let instructions = vec![M64ExtInstruction::Mulw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg::A2), (-12i64).cast_unsigned());
}

#[test]
fn test_mulw_ignores_upper_bits() {
    let mut regs = Registers::<EReg<u64>>::default();

    // Upper 32 bits should be ignored
    regs.write(EReg::A0, 0xDEAD_BEEF_0000_0007);
    regs.write(EReg::A1, 0xCAFE_BABE_0000_0008);

    let instructions = vec![M64ExtInstruction::Mulw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    // 7 * 8 = 56
    assert_eq!(regs.read(EReg::A2), 56);
}

#[test]
fn test_divw_basic() {
    let mut regs = Registers::<EReg<u64>>::default();

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, 3);

    let instructions = vec![M64ExtInstruction::Divw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg::A2), 6);
}

#[test]
fn test_divw_negative() {
    let mut regs = Registers::<EReg<u64>>::default();

    regs.write(EReg::A0, (-20i32).cast_unsigned() as u64);
    regs.write(EReg::A1, 3);

    let instructions = vec![M64ExtInstruction::Divw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg::A2), (-6i64).cast_unsigned());
}

#[test]
fn test_divw_by_zero() {
    let mut regs = Registers::<EReg<u64>>::default();

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, 0);

    let instructions = vec![M64ExtInstruction::Divw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    // Division by zero returns -1 (sign-extended)
    assert_eq!(regs.read(EReg::A2), (-1i64).cast_unsigned());
}

#[test]
fn test_divw_overflow() {
    let mut regs = Registers::<EReg<u64>>::default();

    regs.write(EReg::A0, i32::MIN.cast_unsigned() as u64);
    regs.write(EReg::A1, (-1i32).cast_unsigned() as u64);

    let instructions = vec![M64ExtInstruction::Divw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    // Overflow case: returns i32::MIN sign-extended
    assert_eq!(regs.read(EReg::A2), (i32::MIN as i64).cast_unsigned());
}

#[test]
fn test_divw_ignores_upper_bits() {
    let mut regs = Registers::<EReg<u64>>::default();

    // Upper 32 bits should be ignored
    regs.write(EReg::A0, 0xDEAD_BEEF_0000_0014); // 20 in lower 32 bits
    regs.write(EReg::A1, 0xCAFE_BABE_0000_0003); // 3 in lower 32 bits

    let instructions = vec![M64ExtInstruction::Divw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg::A2), 6);
}

#[test]
fn test_divuw_basic() {
    let mut regs = Registers::<EReg<u64>>::default();

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, 3);

    let instructions = vec![M64ExtInstruction::Divuw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg::A2), 6);
}

#[test]
fn test_divuw_large_unsigned() {
    let mut regs = Registers::<EReg<u64>>::default();

    regs.write(EReg::A0, 0xFFFF_FFFF); // u32::MAX
    regs.write(EReg::A1, 2);

    let instructions = vec![M64ExtInstruction::Divuw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    // 0xFFFFFFFF / 2 = 0x7FFFFFFF (sign-extended to 64-bit)
    assert_eq!(regs.read(EReg::A2), 0x0000_0000_7FFF_FFFF);
}

#[test]
fn test_divuw_by_zero() {
    let mut regs = Registers::<EReg<u64>>::default();

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, 0);

    let instructions = vec![M64ExtInstruction::Divuw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    // Division by zero returns u32::MAX sign-extended
    assert_eq!(regs.read(EReg::A2), 0xFFFF_FFFF_FFFF_FFFF);
}

#[test]
fn test_divuw_ignores_upper_bits() {
    let mut regs = Registers::<EReg<u64>>::default();

    // Upper 32 bits should be ignored
    regs.write(EReg::A0, 0xDEAD_BEEF_0000_0064); // 100 in lower 32 bits
    regs.write(EReg::A1, 0xCAFE_BABE_0000_0005); // 5 in lower 32 bits

    let instructions = vec![M64ExtInstruction::Divuw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg::A2), 20);
}

#[test]
fn test_remw_basic() {
    let mut regs = Registers::<EReg<u64>>::default();

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, 3);

    let instructions = vec![M64ExtInstruction::Remw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg::A2), 2);
}

#[test]
fn test_remw_negative_dividend() {
    let mut regs = Registers::<EReg<u64>>::default();

    regs.write(EReg::A0, (-20i32).cast_unsigned() as u64);
    regs.write(EReg::A1, 3);

    let instructions = vec![M64ExtInstruction::Remw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    // -20 % 3 = -2 (sign-extended)
    assert_eq!(regs.read(EReg::A2), (-2i64).cast_unsigned());
}

#[test]
fn test_remw_negative_divisor() {
    let mut regs = Registers::<EReg<u64>>::default();

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, (-3i32).cast_unsigned() as u64);

    let instructions = vec![M64ExtInstruction::Remw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    // 20 % -3 = 2
    assert_eq!(regs.read(EReg::A2), 2);
}

#[test]
fn test_remw_by_zero() {
    let mut regs = Registers::<EReg<u64>>::default();

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, 0);

    let instructions = vec![M64ExtInstruction::Remw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    // Remainder by zero returns dividend
    assert_eq!(regs.read(EReg::A2), 20);
}

#[test]
fn test_remw_overflow() {
    let mut regs = Registers::<EReg<u64>>::default();

    regs.write(EReg::A0, i32::MIN.cast_unsigned() as u64);
    regs.write(EReg::A1, (-1i32).cast_unsigned() as u64);

    let instructions = vec![M64ExtInstruction::Remw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    // Overflow case: returns 0
    assert_eq!(regs.read(EReg::A2), 0);
}

#[test]
fn test_remw_ignores_upper_bits() {
    let mut regs = Registers::<EReg<u64>>::default();

    // Upper 32 bits should be ignored
    regs.write(EReg::A0, 0xDEAD_BEEF_0000_0017); // 23 in lower 32 bits
    regs.write(EReg::A1, 0xCAFE_BABE_0000_0005); // 5 in lower 32 bits

    let instructions = vec![M64ExtInstruction::Remw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    // 23 % 5 = 3
    assert_eq!(regs.read(EReg::A2), 3);
}

#[test]
fn test_remuw_basic() {
    let mut regs = Registers::<EReg<u64>>::default();

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, 3);

    let instructions = vec![M64ExtInstruction::Remuw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg::A2), 2);
}

#[test]
fn test_remuw_large_unsigned() {
    let mut regs = Registers::<EReg<u64>>::default();

    regs.write(EReg::A0, 0xFFFF_FFFF); // u32::MAX
    regs.write(EReg::A1, 10);

    let instructions = vec![M64ExtInstruction::Remuw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    // 0xFFFFFFFF % 10 = 5 (sign-extended)
    assert_eq!(regs.read(EReg::A2), 5);
}

#[test]
fn test_remuw_by_zero() {
    let mut regs = Registers::<EReg<u64>>::default();

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, 0);

    let instructions = vec![M64ExtInstruction::Remuw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    // Remainder by zero returns dividend
    assert_eq!(regs.read(EReg::A2), 20);
}

#[test]
fn test_remuw_ignores_upper_bits() {
    let mut regs = Registers::<EReg<u64>>::default();

    // Upper 32 bits should be ignored
    regs.write(EReg::A0, 0xDEAD_BEEF_0000_0064); // 100 in lower 32 bits
    regs.write(EReg::A1, 0xCAFE_BABE_0000_0007); // 7 in lower 32 bits

    let instructions = vec![M64ExtInstruction::Remuw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    // 100 % 7 = 2
    assert_eq!(regs.read(EReg::A2), 2);
}

#[test]
fn test_remuw_negative_as_unsigned() {
    let mut regs = Registers::<EReg<u64>>::default();

    // Set value that is negative as i32 but positive as u32
    regs.write(EReg::A0, 0xFFFF_FFFF); // -1 as i32, u32::MAX as u32
    regs.write(EReg::A1, 2);

    let instructions = vec![M64ExtInstruction::Remuw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    // 0xFFFFFFFF % 2 = 1 (sign-extended)
    assert_eq!(regs.read(EReg::A2), 1);
}

// Combined RV64M Tests

#[test]
fn test_mulw_divw_combination() {
    let mut regs = Registers::<EReg<u64>>::default();

    regs.write(EReg::A0, 7);
    regs.write(EReg::A1, 8);

    let instructions = vec![
        // A2 = 7 * 8 = 56
        M64ExtInstruction::Mulw {
            rd: EReg::A2,
            rs1: EReg::A0,
            rs2: EReg::A1,
        },
        // A3 = 56 / 7 = 8
        M64ExtInstruction::Divw {
            rd: EReg::A3,
            rs1: EReg::A2,
            rs2: EReg::A0,
        },
    ];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg::A2), 56);
    assert_eq!(regs.read(EReg::A3), 8);
}

#[test]
fn test_divw_remw_combination() {
    let mut regs = Registers::<EReg<u64>>::default();

    regs.write(EReg::A0, 23);
    regs.write(EReg::A1, 5);

    let instructions = vec![
        // A2 = 23 / 5 = 4
        M64ExtInstruction::Divw {
            rd: EReg::A2,
            rs1: EReg::A0,
            rs2: EReg::A1,
        },
        // A3 = 23 % 5 = 3
        M64ExtInstruction::Remw {
            rd: EReg::A3,
            rs1: EReg::A0,
            rs2: EReg::A1,
        },
    ];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg::A2), 4);
    assert_eq!(regs.read(EReg::A3), 3);
}

#[test]
fn test_rv64m_zero_register() {
    let mut regs = Registers::<EReg<u64>>::default();

    regs.write(EReg::A0, 42);

    let instructions = vec![
        // Should not modify zero register
        M64ExtInstruction::Mulw {
            rd: EReg::Zero,
            rs1: EReg::A0,
            rs2: EReg::A0,
        },
        // Reading from zero should give 0
        M64ExtInstruction::Mulw {
            rd: EReg::A1,
            rs1: EReg::Zero,
            rs2: EReg::A0,
        },
    ];

    for instruction in instructions {
        execute_m_64_ext(&mut regs, instruction);
    }

    assert_eq!(regs.read(EReg::Zero), 0);
    assert_eq!(regs.read(EReg::A1), 0);
}
