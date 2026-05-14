extern crate alloc;

use crate::rv32::test_utils::{TEST_BASE_ADDR, execute, initialize_state};
use crate::{ExecutionError, ProgramCounter, RegisterFile, VirtualMemory};
use ab_riscv_primitives::prelude::*;
use alloc::vec;

// Arithmetic Instructions

#[test]
fn test_add() {
    let mut state = initialize_state([Rv32Instruction::Add {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 10);
    state.regs.write(Reg::A1, 20);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 30);
}

#[test]
fn test_add_overflow() {
    let mut state = initialize_state([Rv32Instruction::Add {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, u32::MAX);
    state.regs.write(Reg::A1, 1);

    execute(&mut state).unwrap();

    // Wrapping behavior
    assert_eq!(state.regs.read(Reg::A2), 0);
}

#[test]
fn test_sub() {
    let mut state = initialize_state([Rv32Instruction::Sub {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 50);
    state.regs.write(Reg::A1, 20);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 30);
}

#[test]
fn test_sub_underflow() {
    let mut state = initialize_state([Rv32Instruction::Sub {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0);
    state.regs.write(Reg::A1, 1);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), u32::MAX);
}

// Logical Instructions

#[test]
fn test_and() {
    let mut state = initialize_state([Rv32Instruction::And {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0b1111_0000);
    state.regs.write(Reg::A1, 0b1010_1010);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0b1010_0000);
}

#[test]
fn test_or() {
    let mut state = initialize_state([Rv32Instruction::Or {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0b1111_0000);
    state.regs.write(Reg::A1, 0b0000_1111);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0b1111_1111);
}

#[test]
fn test_xor() {
    let mut state = initialize_state([Rv32Instruction::Xor {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0b1111_0000);
    state.regs.write(Reg::A1, 0b1010_1010);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0b0101_1010);
}

// Shift Instructions

#[test]
fn test_sll() {
    let mut state = initialize_state([Rv32Instruction::Sll {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 1);
    state.regs.write(Reg::A1, 4);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 16);
}

#[test]
fn test_sll_mask() {
    let mut state = initialize_state([Rv32Instruction::Sll {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 1);
    // High bits should be masked - only lower 5 bits used in RV32
    state.regs.write(Reg::A1, 0x20);

    execute(&mut state).unwrap();

    // 0x20 & 0x1f == 0, so shift by 0
    assert_eq!(state.regs.read(Reg::A2), 1);
}

#[test]
fn test_srl() {
    let mut state = initialize_state([Rv32Instruction::Srl {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 16);
    state.regs.write(Reg::A1, 2);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 4);
}

#[test]
fn test_sra() {
    let mut state = initialize_state([Rv32Instruction::Sra {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, (-16i32).cast_unsigned());
    state.regs.write(Reg::A1, 2);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), (-4i32).cast_unsigned());
}

// Comparison Instructions

#[test]
fn test_slt_less() {
    let mut state = initialize_state([Rv32Instruction::Slt {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, (-5i32).cast_unsigned());
    state.regs.write(Reg::A1, 10);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 1);
}

#[test]
fn test_slt_greater() {
    let mut state = initialize_state([Rv32Instruction::Slt {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 10);
    state.regs.write(Reg::A1, (-5i32).cast_unsigned());

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0);
}

#[test]
fn test_sltu() {
    let mut state = initialize_state([Rv32Instruction::Sltu {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 5);
    state.regs.write(Reg::A1, (-1i32).cast_unsigned());

    execute(&mut state).unwrap();

    // 5 < MAX unsigned
    assert_eq!(state.regs.read(Reg::A2), 1);
}

// Immediate Instructions

#[test]
fn test_addi() {
    let mut state = initialize_state([Rv32Instruction::Addi {
        rd: Reg::A1,
        rs1: Reg::A0,
        imm: 5,
        rs2: Reg::Zero,
    }]);

    state.regs.write(Reg::A0, 10);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A1), 15);
}

#[test]
fn test_addi_negative() {
    let mut state = initialize_state([Rv32Instruction::Addi {
        rd: Reg::A1,
        rs1: Reg::A0,
        imm: -5,
        rs2: Reg::Zero,
    }]);

    state.regs.write(Reg::A0, 10);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A1), 5);
}

#[test]
fn test_slti() {
    let mut state = initialize_state([Rv32Instruction::Slti {
        rd: Reg::A1,
        rs1: Reg::A0,
        imm: 10,
        rs2: Reg::Zero,
    }]);

    state.regs.write(Reg::A0, (-5i32).cast_unsigned());

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A1), 1);
}

#[test]
fn test_sltiu() {
    let mut state = initialize_state([Rv32Instruction::Sltiu {
        rd: Reg::A1,
        rs1: Reg::A0,
        imm: -1,
        rs2: Reg::Zero,
    }]);

    state.regs.write(Reg::A0, 5);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A1), 1);
}

#[test]
fn test_xori() {
    let mut state = initialize_state([Rv32Instruction::Xori {
        rd: Reg::A1,
        rs1: Reg::A0,
        imm: 0xAA,
        rs2: Reg::Zero,
    }]);

    state.regs.write(Reg::A0, 0xFF);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A1), 0x55);
}

#[test]
fn test_ori() {
    let mut state = initialize_state([Rv32Instruction::Ori {
        rd: Reg::A1,
        rs1: Reg::A0,
        imm: 0x0F,
        rs2: Reg::Zero,
    }]);

    state.regs.write(Reg::A0, 0xF0);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A1), 0xFF);
}

#[test]
fn test_andi() {
    let mut state = initialize_state([Rv32Instruction::Andi {
        rd: Reg::A1,
        rs1: Reg::A0,
        imm: 0x0F,
        rs2: Reg::Zero,
    }]);

    state.regs.write(Reg::A0, 0xFF);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A1), 0x0F);
}

#[test]
fn test_slli() {
    let mut state = initialize_state([Rv32Instruction::Slli {
        rd: Reg::A1,
        rs1: Reg::A0,
        shamt: 4,
        rs2: Reg::Zero,
    }]);

    state.regs.write(Reg::A0, 1);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A1), 16);
}

#[test]
fn test_srli() {
    let mut state = initialize_state([Rv32Instruction::Srli {
        rd: Reg::A1,
        rs1: Reg::A0,
        shamt: 2,
        rs2: Reg::Zero,
    }]);

    state.regs.write(Reg::A0, 16);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A1), 4);
}

#[test]
fn test_srai() {
    let mut state = initialize_state([Rv32Instruction::Srai {
        rd: Reg::A1,
        rs1: Reg::A0,
        shamt: 2,
        rs2: Reg::Zero,
    }]);

    state.regs.write(Reg::A0, (-16i32).cast_unsigned());

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A1), (-4i32).cast_unsigned());
}

// Load Instructions

#[test]
fn test_lb() {
    let mut state = initialize_state([Rv32Instruction::Lb {
        rd: Reg::A1,
        rs1: Reg::A0,
        imm: 10,
        rs2: Reg::Zero,
    }]);

    let data_addr = TEST_BASE_ADDR + 0x100;
    state
        .memory
        .write::<i8>(u64::from(data_addr + 10), -5)
        .unwrap();
    state.regs.write(Reg::A0, data_addr);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A1), (-5i32).cast_unsigned());
}

#[test]
fn test_lh() {
    let mut state = initialize_state([Rv32Instruction::Lh {
        rd: Reg::A1,
        rs1: Reg::A0,
        imm: 0,
        rs2: Reg::Zero,
    }]);

    let data_addr = TEST_BASE_ADDR + 0x100;
    state
        .memory
        .write::<i16>(u64::from(data_addr), -300)
        .unwrap();
    state.regs.write(Reg::A0, data_addr);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A1), (-300i32).cast_unsigned());
}

#[test]
fn test_lw() {
    let mut state = initialize_state([Rv32Instruction::Lw {
        rd: Reg::A1,
        rs1: Reg::A0,
        imm: 0,
        rs2: Reg::Zero,
    }]);

    let data_addr = TEST_BASE_ADDR + 0x100;
    state
        .memory
        .write::<u32>(u64::from(data_addr), 0x1234_5678)
        .unwrap();
    state.regs.write(Reg::A0, data_addr);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A1), 0x1234_5678);
}

#[test]
fn test_lbu() {
    let mut state = initialize_state([Rv32Instruction::Lbu {
        rd: Reg::A1,
        rs1: Reg::A0,
        imm: 0,
        rs2: Reg::Zero,
    }]);

    let data_addr = TEST_BASE_ADDR + 0x100;
    state
        .memory
        .write::<u8>(u64::from(data_addr), 0xFF)
        .unwrap();
    state.regs.write(Reg::A0, data_addr);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A1), 0xFF);
}

#[test]
fn test_lhu() {
    let mut state = initialize_state([Rv32Instruction::Lhu {
        rd: Reg::A1,
        rs1: Reg::A0,
        imm: 0,
        rs2: Reg::Zero,
    }]);

    let data_addr = TEST_BASE_ADDR + 0x100;
    state
        .memory
        .write::<u16>(u64::from(data_addr), 0xFFFF)
        .unwrap();
    state.regs.write(Reg::A0, data_addr);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A1), 0xFFFF);
}

// Store Instructions

#[test]
fn test_sb() {
    let mut state = initialize_state([Rv32Instruction::Sb {
        rs1: Reg::A0,
        rs2: Reg::A1,
        imm: 0,
    }]);

    let data_addr = TEST_BASE_ADDR + 0x100;
    state.regs.write(Reg::A0, data_addr);
    state.regs.write(Reg::A1, 0x12);

    execute(&mut state).unwrap();

    assert_eq!(state.memory.read::<u8>(u64::from(data_addr)).unwrap(), 0x12);
}

#[test]
fn test_sh() {
    let mut state = initialize_state([Rv32Instruction::Sh {
        rs1: Reg::A0,
        rs2: Reg::A1,
        imm: 0,
    }]);

    let data_addr = TEST_BASE_ADDR + 0x100;
    state.regs.write(Reg::A0, data_addr);
    state.regs.write(Reg::A1, 0x1234);

    execute(&mut state).unwrap();

    assert_eq!(
        state.memory.read::<u16>(u64::from(data_addr)).unwrap(),
        0x1234
    );
}

#[test]
fn test_sw() {
    let mut state = initialize_state([Rv32Instruction::Sw {
        rs1: Reg::A0,
        rs2: Reg::A1,
        imm: 0,
    }]);

    let data_addr = TEST_BASE_ADDR + 0x100;
    state.regs.write(Reg::A0, data_addr);
    state.regs.write(Reg::A1, 0x1234_5678);

    execute(&mut state).unwrap();

    assert_eq!(
        state.memory.read::<u32>(u64::from(data_addr)).unwrap(),
        0x1234_5678
    );
}

// Branch Instructions

#[test]
fn test_beq_taken() {
    let mut state = initialize_state([Rv32Instruction::Beq {
        rs1: Reg::A0,
        rs2: Reg::A1,
        imm: I24::from_i32(8),
    }]);

    state.regs.write(Reg::A0, 10);
    state.regs.write(Reg::A1, 10);

    let initial_pc = state.instruction_fetcher.get_pc();
    execute(&mut state).unwrap();

    assert_eq!(
        state.instruction_fetcher.get_pc(),
        initial_pc.wrapping_add(8)
    );
}

#[test]
fn test_beq_not_taken() {
    let mut state = initialize_state([
        Rv32Instruction::Beq {
            rs1: Reg::A0,
            rs2: Reg::A1,
            imm: I24::from_i32(8),
        },
        Rv32Instruction::Addi {
            rd: Reg::A2,
            rs1: Reg::Zero,
            // This should execute
            imm: 99,
            rs2: Reg::Zero,
        },
    ]);

    state.regs.write(Reg::A0, 10);
    state.regs.write(Reg::A1, 20);

    execute(&mut state).unwrap();

    // Verify the branch was NOT taken - next instruction executed
    assert_eq!(state.regs.read(Reg::A2), 99);
}

#[test]
fn test_bne_taken() {
    let mut state = initialize_state([Rv32Instruction::Bne {
        rs1: Reg::A0,
        rs2: Reg::A1,
        imm: I24::from_i32(8),
    }]);

    state.regs.write(Reg::A0, 10);
    state.regs.write(Reg::A1, 20);

    let initial_pc = state.instruction_fetcher.get_pc();
    execute(&mut state).unwrap();

    assert_eq!(
        state.instruction_fetcher.get_pc(),
        initial_pc.wrapping_add(8)
    );
}

#[test]
fn test_blt_taken() {
    let mut state = initialize_state([Rv32Instruction::Blt {
        rs1: Reg::A0,
        rs2: Reg::A1,
        imm: I24::from_i32(12),
    }]);

    state.regs.write(Reg::A0, (-10i32).cast_unsigned());
    state.regs.write(Reg::A1, 10);

    let initial_pc = state.instruction_fetcher.get_pc();
    execute(&mut state).unwrap();

    assert_eq!(
        state.instruction_fetcher.get_pc(),
        initial_pc.wrapping_add(12)
    );
}

#[test]
fn test_bge_taken() {
    let mut state = initialize_state([Rv32Instruction::Bge {
        rs1: Reg::A0,
        rs2: Reg::A1,
        imm: I24::from_i32(16),
    }]);

    state.regs.write(Reg::A0, 10);
    state.regs.write(Reg::A1, 10);

    let initial_pc = state.instruction_fetcher.get_pc();
    execute(&mut state).unwrap();

    assert_eq!(
        state.instruction_fetcher.get_pc(),
        initial_pc.wrapping_add(16)
    );
}

#[test]
fn test_bltu_taken() {
    let mut state = initialize_state([Rv32Instruction::Bltu {
        rs1: Reg::A0,
        rs2: Reg::A1,
        imm: I24::from_i32(20),
    }]);

    state.regs.write(Reg::A0, 10);
    state.regs.write(Reg::A1, 20);

    let initial_pc = state.instruction_fetcher.get_pc();
    execute(&mut state).unwrap();

    assert_eq!(
        state.instruction_fetcher.get_pc(),
        initial_pc.wrapping_add(20)
    );
}

#[test]
fn test_bgeu_taken() {
    let mut state = initialize_state([Rv32Instruction::Bgeu {
        rs1: Reg::A0,
        rs2: Reg::A1,
        imm: I24::from_i32(24),
    }]);

    state.regs.write(Reg::A0, 20);
    state.regs.write(Reg::A1, 10);

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
    let mut state = initialize_state([
        Rv32Instruction::Jal {
            rd: Reg::Ra,
            // Skip next instruction
            imm: I24::from_i32(8),
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
        Rv32Instruction::Addi {
            rd: Reg::A2,
            rs1: Reg::Zero,
            // Should be skipped
            imm: 99,
            rs2: Reg::Zero,
        },
        Rv32Instruction::Addi {
            rd: Reg::A2,
            rs1: Reg::Zero,
            // Should execute
            imm: 42,
            rs2: Reg::Zero,
        },
    ]);

    let initial_pc = state.instruction_fetcher.get_pc();
    state.regs.write(Reg::A2, 0);

    execute(&mut state).unwrap();

    // Return address
    assert_eq!(state.regs.read(Reg::Ra), initial_pc + 4);
    assert_eq!(state.regs.read(Reg::A2), 42);
}

#[test]
fn test_jalr() {
    let mut state = initialize_state([
        Rv32Instruction::Jalr {
            rd: Reg::Ra,
            rs1: Reg::A0,
            imm: 0,
            rs2: Reg::Zero,
        },
        Rv32Instruction::Addi {
            rd: Reg::A2,
            rs1: Reg::Zero,
            // Should be skipped
            imm: 99,
            rs2: Reg::Zero,
        },
        Rv32Instruction::Addi {
            rd: Reg::A2,
            rs1: Reg::Zero,
            // Should execute
            imm: 42,
            rs2: Reg::Zero,
        },
    ]);

    let initial_pc = state.instruction_fetcher.get_pc();
    let target_addr = TEST_BASE_ADDR + 8;
    state.regs.write(Reg::A0, target_addr);
    state.regs.write(Reg::A2, 0);

    execute(&mut state).unwrap();

    // Return address
    assert_eq!(state.regs.read(Reg::Ra), initial_pc + 4);
    assert_eq!(state.regs.read(Reg::A2), 42);
}

#[test]
fn test_jalr_clear_lsb() {
    let mut state = initialize_state([
        Rv32Instruction::Jalr {
            rd: Reg::Ra,
            rs1: Reg::A0,
            imm: 0,
            rs2: Reg::Zero,
        },
        Rv32Instruction::Addi {
            rd: Reg::A2,
            rs1: Reg::Zero,
            imm: 99,
            rs2: Reg::Zero,
        },
        Rv32Instruction::Addi {
            rd: Reg::A2,
            rs1: Reg::Zero,
            imm: 42,
            rs2: Reg::Zero,
        },
    ]);

    let initial_pc = state.instruction_fetcher.get_pc();
    // Odd address
    state.regs.write(Reg::A0, TEST_BASE_ADDR + 9);
    state.regs.write(Reg::A2, 0);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::Ra), initial_pc + 4);
    // LSB cleared: 9 -> 8
    assert_eq!(state.regs.read(Reg::A2), 42);
}

// Upper Immediate Instructions

#[test]
fn test_lui() {
    let mut state = initialize_state([Rv32Instruction::Lui {
        rd: Reg::A0,
        // Already shifted - bits [31:12]
        imm: I24WithZeroedBits::from_i32(0x12345000u32.cast_signed()),
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A0), 0x12345000u32);
}

#[test]
fn test_lui_negative() {
    let mut state = initialize_state([Rv32Instruction::Lui {
        rd: Reg::A0,
        imm: I24WithZeroedBits::from_i32(0xfffff000u32.cast_signed()),
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);

    execute(&mut state).unwrap();

    // In RV32 the register is 32-bit, no further sign extension needed
    assert_eq!(state.regs.read(Reg::A0), 0xfffff000u32);
}

#[test]
fn test_auipc() {
    let mut state = initialize_state([Rv32Instruction::Auipc {
        rd: Reg::A0,
        // Already shifted - bits [31:12]
        imm: I24WithZeroedBits::from_i32(0x12345000u32.cast_signed()),
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);

    let initial_pc = state.instruction_fetcher.get_pc();

    execute(&mut state).unwrap();

    assert_eq!(
        state.regs.read(Reg::A0),
        initial_pc.wrapping_add(0x12345000u32)
    );
}

#[test]
fn test_auipc_negative() {
    let mut state = initialize_state([Rv32Instruction::Auipc {
        rd: Reg::A0,
        imm: I24WithZeroedBits::from_i32(0xfffff000u32.cast_signed()),
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);

    let initial_pc = state.instruction_fetcher.get_pc();

    execute(&mut state).unwrap();

    // Should wrap around: PC + 0xfffff000
    assert_eq!(
        state.regs.read(Reg::A0),
        initial_pc.wrapping_add(0xfffff000u32)
    );
}

// Special Instructions

#[test]
fn test_fence() {
    let mut state = initialize_state([Rv32Instruction::Fence {
        pred: 0xF,
        succ: 0xF,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);

    execute(&mut state).unwrap();

    // Should execute without error (NOP in single-threaded)
}

#[test]
fn test_ebreak() {
    let mut state = initialize_state([Rv32Instruction::Ebreak {
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);

    execute(&mut state).unwrap();

    // Should execute without error (NOP by default)
}

#[test]
fn test_ecall_unsupported() {
    let mut state = initialize_state([Rv32Instruction::Ecall {
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);

    let result = execute(&mut state);

    assert!(matches!(
        result,
        Err(ExecutionError::EcallUnsupported { .. })
    ));
}

#[test]
fn test_unimp() {
    let mut state = initialize_state([Rv32Instruction::Unimp {
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);

    let result = execute(&mut state);

    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

// Error Conditions

#[test]
fn test_out_of_bounds_read() {
    let mut state = initialize_state([Rv32Instruction::Lw {
        rd: Reg::A1,
        rs1: Reg::A0,
        imm: 0,
        rs2: Reg::Zero,
    }]);

    // Invalid address
    state.regs.write(Reg::A0, 0x0);

    let result = execute(&mut state);

    assert!(matches!(result, Err(ExecutionError::MemoryAccess(_))));
}

#[test]
fn test_out_of_bounds_write() {
    let mut state = initialize_state([Rv32Instruction::Sw {
        rs1: Reg::A0,
        rs2: Reg::A1,
        imm: 0,
    }]);

    // Invalid address
    state.regs.write(Reg::A0, 0x0);
    state.regs.write(Reg::A1, 42);

    let result = execute(&mut state);

    assert!(matches!(result, Err(ExecutionError::MemoryAccess(_))));
}

// Register Zero Tests

#[test]
fn test_write_to_zero_register() {
    let mut state = initialize_state([Rv32Instruction::Addi {
        rd: Reg::Zero,
        rs1: Reg::Zero,
        imm: 100,
        rs2: Reg::Zero,
    }]);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::Zero), 0);
}

#[test]
fn test_read_from_zero_register() {
    let mut state = initialize_state([Rv32Instruction::Add {
        rd: Reg::A0,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A0), 0);
}

// Complex Programs

#[test]
fn test_fibonacci() {
    // Fibonacci loop - iterate 9 times to go from fib(1) to fib(10)
    let mut instructions = vec![];

    for _ in 0..9 {
        // a3 = a1 + a2 (next fib number)
        instructions.push(Rv32Instruction::Add {
            rd: Reg::A3,
            rs1: Reg::A1,
            rs2: Reg::A2,
        });
        // a1 = a2 (shift window)
        instructions.push(Rv32Instruction::Add {
            rd: Reg::A1,
            rs1: Reg::A2,
            rs2: Reg::Zero,
        });
        // a2 = a3 (shift window)
        instructions.push(Rv32Instruction::Add {
            rd: Reg::A2,
            rs1: Reg::A3,
            rs2: Reg::Zero,
        });
    }

    let mut state = initialize_state(instructions);

    // fib(n-2) = fib(0)
    state.regs.write(Reg::A1, 0);
    // fib(n-1) = fib(1)
    state.regs.write(Reg::A2, 1);

    execute(&mut state).unwrap();

    // fib(10) = 55
    assert_eq!(state.regs.read(Reg::A2), 55);
}
