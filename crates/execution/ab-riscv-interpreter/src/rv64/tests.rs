extern crate alloc;

use crate::rv64::test_utils::{TEST_BASE_ADDR, execute, initialize_state};
use crate::{ExecutionError, ProgramCounter, VirtualMemory};
use ab_riscv_primitives::instruction::rv64::Rv64Instruction;
use ab_riscv_primitives::registers::EReg;
use alloc::vec;

// Arithmetic Instructions

#[test]
fn test_add() {
    let mut state = initialize_state(vec![Rv64Instruction::Add {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 10);
    state.regs.write(EReg::A1, 20);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 30);
}

#[test]
fn test_add_overflow() {
    let mut state = initialize_state(vec![Rv64Instruction::Add {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, u64::MAX);
    state.regs.write(EReg::A1, 1);

    execute(&mut state).unwrap();

    // Wrapping behavior
    assert_eq!(state.regs.read(EReg::A2), 0);
}

#[test]
fn test_sub() {
    let mut state = initialize_state(vec![Rv64Instruction::Sub {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 50);
    state.regs.write(EReg::A1, 20);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 30);
}

#[test]
fn test_sub_underflow() {
    let mut state = initialize_state(vec![Rv64Instruction::Sub {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 0);
    state.regs.write(EReg::A1, 1);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), u64::MAX);
}

// Logical Instructions

#[test]
fn test_and() {
    let mut state = initialize_state(vec![Rv64Instruction::And {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 0b1111_0000);
    state.regs.write(EReg::A1, 0b1010_1010);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 0b1010_0000);
}

#[test]
fn test_or() {
    let mut state = initialize_state(vec![Rv64Instruction::Or {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 0b1111_0000);
    state.regs.write(EReg::A1, 0b0000_1111);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 0b1111_1111);
}

#[test]
fn test_xor() {
    let mut state = initialize_state(vec![Rv64Instruction::Xor {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 0b1111_0000);
    state.regs.write(EReg::A1, 0b1010_1010);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 0b0101_1010);
}

// Shift Instructions

#[test]
fn test_sll() {
    let mut state = initialize_state(vec![Rv64Instruction::Sll {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 1);
    state.regs.write(EReg::A1, 4);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 16);
}

#[test]
fn test_sll_mask() {
    let mut state = initialize_state(vec![Rv64Instruction::Sll {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 1);
    // High bits should be masked
    state.regs.write(EReg::A1, 0x100);

    execute(&mut state).unwrap();

    // Only lower 6 bits used
    assert_eq!(state.regs.read(EReg::A2), 1);
}

#[test]
fn test_srl() {
    let mut state = initialize_state(vec![Rv64Instruction::Srl {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 16);
    state.regs.write(EReg::A1, 2);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 4);
}

#[test]
fn test_sra() {
    let mut state = initialize_state(vec![Rv64Instruction::Sra {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, (-16i64).cast_unsigned());
    state.regs.write(EReg::A1, 2);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), (-4i64).cast_unsigned());
}

// Comparison Instructions

#[test]
fn test_slt_less() {
    let mut state = initialize_state(vec![Rv64Instruction::Slt {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, (-5i64).cast_unsigned());
    state.regs.write(EReg::A1, 10);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 1);
}

#[test]
fn test_slt_greater() {
    let mut state = initialize_state(vec![Rv64Instruction::Slt {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 10);
    state.regs.write(EReg::A1, (-5i64).cast_unsigned());

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 0);
}

#[test]
fn test_sltu() {
    let mut state = initialize_state(vec![Rv64Instruction::Sltu {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 5);
    state.regs.write(EReg::A1, (-1i64).cast_unsigned());

    execute(&mut state).unwrap();

    // 5 < MAX unsigned
    assert_eq!(state.regs.read(EReg::A2), 1);
}

// RV64 32-bit Instructions

#[test]
fn test_addw() {
    let mut state = initialize_state(vec![Rv64Instruction::Addw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 0x0000_0001_8000_0000);
    state.regs.write(EReg::A1, 0x0000_0000_8000_0000);

    execute(&mut state).unwrap();

    // Sign-extended result
    assert_eq!(state.regs.read(EReg::A2), 0);
}

#[test]
fn test_subw() {
    let mut state = initialize_state(vec![Rv64Instruction::Subw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 0x0000_0001_0000_0000);
    state.regs.write(EReg::A1, 1);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), (-1i64).cast_unsigned());
}

#[test]
fn test_sllw() {
    let mut state = initialize_state(vec![Rv64Instruction::Sllw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 1);
    state.regs.write(EReg::A1, 31);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 0xFFFF_FFFF_8000_0000);
}

#[test]
fn test_srlw() {
    let mut state = initialize_state(vec![Rv64Instruction::Srlw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 0xFFFF_FFFF_8000_0000);
    state.regs.write(EReg::A1, 1);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 0x0000_0000_4000_0000);
}

#[test]
fn test_sraw() {
    let mut state = initialize_state(vec![Rv64Instruction::Sraw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 0xFFFF_FFFF_8000_0000);
    state.regs.write(EReg::A1, 1);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 0xFFFF_FFFF_C000_0000);
}

// Immediate Instructions

#[test]
fn test_addi() {
    let mut state = initialize_state(vec![Rv64Instruction::Addi {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 5,
    }]);

    state.regs.write(EReg::A0, 10);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A1), 15);
}

#[test]
fn test_addi_negative() {
    let mut state = initialize_state(vec![Rv64Instruction::Addi {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: -5,
    }]);

    state.regs.write(EReg::A0, 10);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A1), 5);
}

#[test]
fn test_slti() {
    let mut state = initialize_state(vec![Rv64Instruction::Slti {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 10,
    }]);

    state.regs.write(EReg::A0, (-5i64).cast_unsigned());

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A1), 1);
}

#[test]
fn test_sltiu() {
    let mut state = initialize_state(vec![Rv64Instruction::Sltiu {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: -1,
    }]);

    state.regs.write(EReg::A0, 5);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A1), 1);
}

#[test]
fn test_xori() {
    let mut state = initialize_state(vec![Rv64Instruction::Xori {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 0xAA,
    }]);

    state.regs.write(EReg::A0, 0xFF);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A1), 0x55);
}

#[test]
fn test_ori() {
    let mut state = initialize_state(vec![Rv64Instruction::Ori {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 0x0F,
    }]);

    state.regs.write(EReg::A0, 0xF0);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A1), 0xFF);
}

#[test]
fn test_andi() {
    let mut state = initialize_state(vec![Rv64Instruction::Andi {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 0x0F,
    }]);

    state.regs.write(EReg::A0, 0xFF);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A1), 0x0F);
}

#[test]
fn test_slli() {
    let mut state = initialize_state(vec![Rv64Instruction::Slli {
        rd: EReg::A1,
        rs1: EReg::A0,
        shamt: 4,
    }]);

    state.regs.write(EReg::A0, 1);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A1), 16);
}

#[test]
fn test_srli() {
    let mut state = initialize_state(vec![Rv64Instruction::Srli {
        rd: EReg::A1,
        rs1: EReg::A0,
        shamt: 2,
    }]);

    state.regs.write(EReg::A0, 16);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A1), 4);
}

#[test]
fn test_srai() {
    let mut state = initialize_state(vec![Rv64Instruction::Srai {
        rd: EReg::A1,
        rs1: EReg::A0,
        shamt: 2,
    }]);

    state.regs.write(EReg::A0, (-16i64).cast_unsigned());

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A1), (-4i64).cast_unsigned());
}

#[test]
fn test_addiw() {
    let mut state = initialize_state(vec![Rv64Instruction::Addiw {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 5,
    }]);

    // -5 sign-extended
    state.regs.write(EReg::A0, 0xFFFF_FFFF_FFFF_FFFB);

    execute(&mut state).unwrap();

    // -5 + 5 = 0
    assert_eq!(state.regs.read(EReg::A1), 0);
}

#[test]
fn test_slliw() {
    let mut state = initialize_state(vec![Rv64Instruction::Slliw {
        rd: EReg::A1,
        rs1: EReg::A0,
        shamt: 31,
    }]);

    state.regs.write(EReg::A0, 1);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A1), 0xFFFF_FFFF_8000_0000);
}

#[test]
fn test_srliw() {
    let mut state = initialize_state(vec![Rv64Instruction::Srliw {
        rd: EReg::A1,
        rs1: EReg::A0,
        shamt: 1,
    }]);

    state.regs.write(EReg::A0, 0xFFFF_FFFF_8000_0000);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A1), 0x0000_0000_4000_0000);
}

#[test]
fn test_sraiw() {
    let mut state = initialize_state(vec![Rv64Instruction::Sraiw {
        rd: EReg::A1,
        rs1: EReg::A0,
        shamt: 1,
    }]);

    state.regs.write(EReg::A0, 0xFFFF_FFFF_8000_0000);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A1), 0xFFFF_FFFF_C000_0000);
}

// Load Instructions

#[test]
fn test_lb() {
    let mut state = initialize_state(vec![Rv64Instruction::Lb {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 10,
    }]);

    let data_addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<i8>(data_addr + 10, -5).unwrap();
    state.regs.write(EReg::A0, data_addr);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A1), (-5i64).cast_unsigned());
}

#[test]
fn test_lh() {
    let mut state = initialize_state(vec![Rv64Instruction::Lh {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 0,
    }]);

    let data_addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<i16>(data_addr, -300).unwrap();
    state.regs.write(EReg::A0, data_addr);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A1), (-300i64).cast_unsigned());
}

#[test]
fn test_lw() {
    let mut state = initialize_state(vec![Rv64Instruction::Lw {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 0,
    }]);

    let data_addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<i32>(data_addr, -100000).unwrap();
    state.regs.write(EReg::A0, data_addr);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A1), (-100000i64).cast_unsigned());
}

#[test]
fn test_ld() {
    let mut state = initialize_state(vec![Rv64Instruction::Ld {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 0,
    }]);

    let data_addr = TEST_BASE_ADDR + 0x100;
    state
        .memory
        .write::<u64>(data_addr, 0x1234_5678_9ABC_DEF0)
        .unwrap();
    state.regs.write(EReg::A0, data_addr);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A1), 0x1234_5678_9ABC_DEF0);
}

#[test]
fn test_lbu() {
    let mut state = initialize_state(vec![Rv64Instruction::Lbu {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 0,
    }]);

    let data_addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u8>(data_addr, 0xFF).unwrap();
    state.regs.write(EReg::A0, data_addr);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A1), 0xFF);
}

#[test]
fn test_lhu() {
    let mut state = initialize_state(vec![Rv64Instruction::Lhu {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 0,
    }]);

    let data_addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u16>(data_addr, 0xFFFF).unwrap();
    state.regs.write(EReg::A0, data_addr);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A1), 0xFFFF);
}

#[test]
fn test_lwu() {
    let mut state = initialize_state(vec![Rv64Instruction::Lwu {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 0,
    }]);

    let data_addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u32>(data_addr, 0xFFFF_FFFF).unwrap();
    state.regs.write(EReg::A0, data_addr);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A1), 0xFFFF_FFFF);
}

// Store Instructions

#[test]
fn test_sb() {
    let mut state = initialize_state(vec![Rv64Instruction::Sb {
        rs1: EReg::A0,
        rs2: EReg::A1,
        imm: 0,
    }]);

    let data_addr = TEST_BASE_ADDR + 0x100;
    state.regs.write(EReg::A0, data_addr);
    state.regs.write(EReg::A1, 0x12);

    execute(&mut state).unwrap();

    assert_eq!(state.memory.read::<u8>(data_addr).unwrap(), 0x12);
}

#[test]
fn test_sh() {
    let mut state = initialize_state(vec![Rv64Instruction::Sh {
        rs1: EReg::A0,
        rs2: EReg::A1,
        imm: 0,
    }]);

    let data_addr = TEST_BASE_ADDR + 0x100;
    state.regs.write(EReg::A0, data_addr);
    state.regs.write(EReg::A1, 0x1234);

    execute(&mut state).unwrap();

    assert_eq!(state.memory.read::<u16>(data_addr).unwrap(), 0x1234);
}

#[test]
fn test_sw() {
    let mut state = initialize_state(vec![Rv64Instruction::Sw {
        rs1: EReg::A0,
        rs2: EReg::A1,
        imm: 0,
    }]);

    let data_addr = TEST_BASE_ADDR + 0x100;
    state.regs.write(EReg::A0, data_addr);
    state.regs.write(EReg::A1, 0x1234_5678);

    execute(&mut state).unwrap();

    assert_eq!(state.memory.read::<u32>(data_addr).unwrap(), 0x1234_5678);
}

#[test]
fn test_sd() {
    let mut state = initialize_state(vec![Rv64Instruction::Sd {
        rs1: EReg::A0,
        rs2: EReg::A1,
        imm: 0,
    }]);

    let data_addr = TEST_BASE_ADDR + 0x100;
    state.regs.write(EReg::A0, data_addr);
    state.regs.write(EReg::A1, 0x1234_5678_9ABC_DEF0);

    execute(&mut state).unwrap();

    assert_eq!(
        state.memory.read::<u64>(data_addr).unwrap(),
        0x1234_5678_9ABC_DEF0
    );
}

// Branch Instructions

#[test]
fn test_beq_taken() {
    let mut state = initialize_state(vec![Rv64Instruction::Beq {
        rs1: EReg::A0,
        rs2: EReg::A1,
        // Branch offset from PC before increment
        imm: 8,
    }]);

    state.regs.write(EReg::A0, 10);
    state.regs.write(EReg::A1, 10);

    let initial_pc = state.instruction_fetcher.get_pc();
    execute(&mut state).unwrap();

    // Branch calculates: old_pc (stored before fetch incremented it) + offset
    // The implementation stores old_pc before PC is incremented
    // So: initial_pc + 8 = 0x1000 + 8 = 0x1008
    assert_eq!(
        state.instruction_fetcher.get_pc(),
        initial_pc.wrapping_add(8)
    );
}

#[test]
fn test_beq_not_taken() {
    let mut state = initialize_state(vec![
        Rv64Instruction::Beq {
            rs1: EReg::A0,
            rs2: EReg::A1,
            imm: 8,
        },
        Rv64Instruction::Addi {
            rd: EReg::A2,
            rs1: EReg::Zero,
            // This should execute
            imm: 99,
        },
    ]);

    state.regs.write(EReg::A0, 10);
    state.regs.write(EReg::A1, 20);

    execute(&mut state).unwrap();

    // Verify the branch was NOT taken - next instruction executed
    assert_eq!(state.regs.read(EReg::A2), 99);
}

#[test]
fn test_bne_taken() {
    let mut state = initialize_state(vec![Rv64Instruction::Bne {
        rs1: EReg::A0,
        rs2: EReg::A1,
        imm: 8,
    }]);

    state.regs.write(EReg::A0, 10);
    state.regs.write(EReg::A1, 20);

    let initial_pc = state.instruction_fetcher.get_pc();
    execute(&mut state).unwrap();

    assert_eq!(
        state.instruction_fetcher.get_pc(),
        initial_pc.wrapping_add(8)
    );
}

#[test]
fn test_blt_taken() {
    let mut state = initialize_state(vec![Rv64Instruction::Blt {
        rs1: EReg::A0,
        rs2: EReg::A1,
        imm: 12,
    }]);

    state.regs.write(EReg::A0, (-10i64).cast_unsigned());
    state.regs.write(EReg::A1, 10);

    let initial_pc = state.instruction_fetcher.get_pc();
    execute(&mut state).unwrap();

    assert_eq!(
        state.instruction_fetcher.get_pc(),
        initial_pc.wrapping_add(12)
    );
}

#[test]
fn test_bge_taken() {
    let mut state = initialize_state(vec![Rv64Instruction::Bge {
        rs1: EReg::A0,
        rs2: EReg::A1,
        imm: 16,
    }]);

    state.regs.write(EReg::A0, 10);
    state.regs.write(EReg::A1, 10);

    let initial_pc = state.instruction_fetcher.get_pc();
    execute(&mut state).unwrap();

    assert_eq!(
        state.instruction_fetcher.get_pc(),
        initial_pc.wrapping_add(16)
    );
}

#[test]
fn test_bltu_taken() {
    let mut state = initialize_state(vec![Rv64Instruction::Bltu {
        rs1: EReg::A0,
        rs2: EReg::A1,
        imm: 20,
    }]);

    state.regs.write(EReg::A0, 10);
    state.regs.write(EReg::A1, 20);

    let initial_pc = state.instruction_fetcher.get_pc();
    execute(&mut state).unwrap();

    assert_eq!(
        state.instruction_fetcher.get_pc(),
        initial_pc.wrapping_add(20)
    );
}

#[test]
fn test_bgeu_taken() {
    let mut state = initialize_state(vec![Rv64Instruction::Bgeu {
        rs1: EReg::A0,
        rs2: EReg::A1,
        imm: 24,
    }]);

    state.regs.write(EReg::A0, 20);
    state.regs.write(EReg::A1, 10);

    let initial_pc = state.instruction_fetcher.get_pc();
    execute(&mut state).unwrap();

    assert_eq!(
        state.instruction_fetcher.get_pc(),
        initial_pc.wrapping_add(24)
    );
}

// Jump Instructions

#[test]
fn test_jal() {
    let mut state = initialize_state(vec![
        Rv64Instruction::Jal {
            rd: EReg::Ra,
            // Skip next instruction
            imm: 8,
        },
        Rv64Instruction::Addi {
            rd: EReg::A2,
            rs1: EReg::Zero,
            // Should be skipped
            imm: 99,
        },
        Rv64Instruction::Addi {
            rd: EReg::A2,
            rs1: EReg::Zero,
            // Should execute
            imm: 42,
        },
    ]);

    let initial_pc = state.instruction_fetcher.get_pc();
    state.regs.write(EReg::A2, 0);

    execute(&mut state).unwrap();

    // Return address
    assert_eq!(state.regs.read(EReg::Ra), initial_pc + 4);
    assert_eq!(state.regs.read(EReg::A2), 42);
}

#[test]
fn test_jalr() {
    let mut state = initialize_state(vec![
        Rv64Instruction::Jalr {
            rd: EReg::Ra,
            rs1: EReg::A0,
            imm: 0,
        },
        Rv64Instruction::Addi {
            rd: EReg::A2,
            rs1: EReg::Zero,
            // Should be skipped
            imm: 99,
        },
        Rv64Instruction::Addi {
            rd: EReg::A2,
            rs1: EReg::Zero,
            // Should execute
            imm: 42,
        },
    ]);

    let initial_pc = state.instruction_fetcher.get_pc();
    let target_addr = TEST_BASE_ADDR + 8;
    state.regs.write(EReg::A0, target_addr);
    state.regs.write(EReg::A2, 0);

    execute(&mut state).unwrap();

    // Return address
    assert_eq!(state.regs.read(EReg::Ra), initial_pc + 4);
    assert_eq!(state.regs.read(EReg::A2), 42);
}

#[test]
fn test_jalr_clear_lsb() {
    let mut state = initialize_state(vec![
        Rv64Instruction::Jalr {
            rd: EReg::Ra,
            rs1: EReg::A0,
            imm: 0,
        },
        Rv64Instruction::Addi {
            rd: EReg::A2,
            rs1: EReg::Zero,
            imm: 99,
        },
        Rv64Instruction::Addi {
            rd: EReg::A2,
            rs1: EReg::Zero,
            imm: 42,
        },
    ]);

    let initial_pc = state.instruction_fetcher.get_pc();
    // Odd address
    state.regs.write(EReg::A0, TEST_BASE_ADDR + 9);
    state.regs.write(EReg::A2, 0);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::Ra), initial_pc + 4);
    // LSB cleared: 9 -> 8
    assert_eq!(state.regs.read(EReg::A2), 42);
}

// Upper Immediate Instructions

#[test]
fn test_lui() {
    let mut state = initialize_state(vec![Rv64Instruction::Lui {
        rd: EReg::A0,
        // Already shifted - bits [31:12]
        imm: 0x12345000,
    }]);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A0), 0x12345000u64);
}

#[test]
fn test_lui_negative() {
    let mut state = initialize_state(vec![Rv64Instruction::Lui {
        rd: EReg::A0,
        // 0xFFFFF000 as upper 20 bits (already shifted)
        imm: 0xfffff000u32.cast_signed(),
    }]);

    execute(&mut state).unwrap();

    // Should be sign-extended: 0xfffffffffffff000
    assert_eq!(state.regs.read(EReg::A0), 0xfffffffffffff000u64);
}

#[test]
fn test_auipc() {
    let mut state = initialize_state(vec![Rv64Instruction::Auipc {
        rd: EReg::A0,
        // Already shifted - bits [31:12]
        imm: 0x12345000,
    }]);

    let initial_pc = state.instruction_fetcher.get_pc();

    execute(&mut state).unwrap();

    assert_eq!(
        state.regs.read(EReg::A0),
        initial_pc.wrapping_add(0x12345000u64)
    );
}

#[test]
fn test_auipc_negative() {
    let mut state = initialize_state(vec![Rv64Instruction::Auipc {
        rd: EReg::A0,
        // Negative immediate (all upper bits set)
        imm: 0xfffff000u32.cast_signed(),
    }]);

    let initial_pc = state.instruction_fetcher.get_pc();

    execute(&mut state).unwrap();

    // Should wrap around: PC + sign_extend(0xfffff000)
    assert_eq!(
        state.regs.read(EReg::A0),
        initial_pc.wrapping_add(0xfffffffffffff000u64)
    );
}

// Special Instructions

#[test]
fn test_fence() {
    let mut state = initialize_state(vec![Rv64Instruction::Fence {
        pred: 0xF,
        succ: 0xF,
    }]);

    execute(&mut state).unwrap();

    // Should execute without error (NOP in single-threaded)
}

#[test]
fn test_ebreak() {
    let mut state = initialize_state(vec![Rv64Instruction::Ebreak]);

    execute(&mut state).unwrap();

    // Should execute without error (NOP by default)
}

#[test]
fn test_ecall_unsupported() {
    let mut state = initialize_state(vec![Rv64Instruction::Ecall]);

    let result = execute(&mut state);

    assert!(matches!(
        result,
        Err(ExecutionError::UnsupportedInstruction { .. })
    ));
}

#[test]
fn test_unimp() {
    let mut state = initialize_state(vec![Rv64Instruction::Unimp]);

    let result = execute(&mut state);

    assert!(matches!(
        result,
        Err(ExecutionError::UnimpInstruction { .. })
    ));
}

// Error Conditions

#[test]
fn test_out_of_bounds_read() {
    let mut state = initialize_state(vec![Rv64Instruction::Ld {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 0,
    }]);

    // Invalid address
    state.regs.write(EReg::A0, 0x0);

    let result = execute(&mut state);

    assert!(matches!(result, Err(ExecutionError::MemoryAccess(_))));
}

#[test]
fn test_out_of_bounds_write() {
    let mut state = initialize_state(vec![Rv64Instruction::Sd {
        rs1: EReg::A0,
        rs2: EReg::A1,
        imm: 0,
    }]);

    // Invalid address
    state.regs.write(EReg::A0, 0x0);
    state.regs.write(EReg::A1, 42);

    let result = execute(&mut state);

    assert!(matches!(result, Err(ExecutionError::MemoryAccess(_))));
}

// Register Zero Tests

#[test]
fn test_write_to_zero_register() {
    let mut state = initialize_state(vec![Rv64Instruction::Addi {
        rd: EReg::Zero,
        rs1: EReg::Zero,
        imm: 100,
    }]);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::Zero), 0);
}

#[test]
fn test_read_from_zero_register() {
    let mut state = initialize_state(vec![Rv64Instruction::Add {
        rd: EReg::A0,
        rs1: EReg::Zero,
        rs2: EReg::Zero,
    }]);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A0), 0);
}

// Complex Programs

#[test]
fn test_fibonacci() {
    // Fibonacci loop - iterate 9 times to go from fib(1) to fib(10)
    let mut instructions = vec![];

    for _ in 0..9 {
        // a3 = a1 + a2 (next fib number)
        instructions.push(Rv64Instruction::Add {
            rd: EReg::A3,
            rs1: EReg::A1,
            rs2: EReg::A2,
        });
        // a1 = a2 (shift window)
        instructions.push(Rv64Instruction::Add {
            rd: EReg::A1,
            rs1: EReg::A2,
            rs2: EReg::Zero,
        });
        // a2 = a3 (shift window)
        instructions.push(Rv64Instruction::Add {
            rd: EReg::A2,
            rs1: EReg::A3,
            rs2: EReg::Zero,
        });
    }

    let mut state = initialize_state(instructions);

    // Calculate fib(10)
    // fib(0) = 0, fib(1) = 1, fib(2) = 1, ..., fib(10) = 55

    // fib(n-2) = fib(0)
    state.regs.write(EReg::A1, 0);
    // fib(n-1) = fib(1)
    state.regs.write(EReg::A2, 1);

    execute(&mut state).unwrap();

    // fib(10) = 55
    assert_eq!(state.regs.read(EReg::A2), 55);
}
