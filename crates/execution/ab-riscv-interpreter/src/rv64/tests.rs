extern crate alloc;

use crate::tests_utils::{TEST_BASE_ADDR, TestInstructionHandler, setup_test};
use crate::{ExecuteError, VirtualMemory, execute_rv64mbzbc};
use ab_riscv_primitives::instruction::Rv64MBZbcInstruction;
use ab_riscv_primitives::instruction::rv64::Rv64Instruction;
use ab_riscv_primitives::registers::{EReg64, GenericRegisters64};
use alloc::vec;

// Arithmetic Instructions

#[test]
fn test_add() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, 10);
    regs.write(EReg64::A1, 20);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Add {
        rd: EReg64::A2,
        rs1: EReg64::A0,
        rs2: EReg64::A1,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A2), 30);
}

#[test]
fn test_add_overflow() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, u64::MAX);
    regs.write(EReg64::A1, 1);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Add {
        rd: EReg64::A2,
        rs1: EReg64::A0,
        rs2: EReg64::A1,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    // Wrapping behavior
    assert_eq!(regs.read(EReg64::A2), 0);
}

#[test]
fn test_sub() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, 50);
    regs.write(EReg64::A1, 20);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Sub {
        rd: EReg64::A2,
        rs1: EReg64::A0,
        rs2: EReg64::A1,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A2), 30);
}

#[test]
fn test_sub_underflow() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, 0);
    regs.write(EReg64::A1, 1);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Sub {
        rd: EReg64::A2,
        rs1: EReg64::A0,
        rs2: EReg64::A1,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A2), u64::MAX);
}

// Logical Instructions

#[test]
fn test_and() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, 0b1111_0000);
    regs.write(EReg64::A1, 0b1010_1010);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::And {
        rd: EReg64::A2,
        rs1: EReg64::A0,
        rs2: EReg64::A1,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A2), 0b1010_0000);
}

#[test]
fn test_or() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, 0b1111_0000);
    regs.write(EReg64::A1, 0b0000_1111);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Or {
        rd: EReg64::A2,
        rs1: EReg64::A0,
        rs2: EReg64::A1,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A2), 0b1111_1111);
}

#[test]
fn test_xor() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, 0b1111_0000);
    regs.write(EReg64::A1, 0b1010_1010);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Xor {
        rd: EReg64::A2,
        rs1: EReg64::A0,
        rs2: EReg64::A1,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A2), 0b0101_1010);
}

// Shift Instructions

#[test]
fn test_sll() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, 1);
    regs.write(EReg64::A1, 4);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Sll {
        rd: EReg64::A2,
        rs1: EReg64::A0,
        rs2: EReg64::A1,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A2), 16);
}

#[test]
fn test_sll_mask() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, 1);
    // High bits should be masked
    regs.write(EReg64::A1, 0x100);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Sll {
        rd: EReg64::A2,
        rs1: EReg64::A0,
        rs2: EReg64::A1,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    // Only lower 6 bits used
    assert_eq!(regs.read(EReg64::A2), 1);
}

#[test]
fn test_srl() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, 16);
    regs.write(EReg64::A1, 2);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Srl {
        rd: EReg64::A2,
        rs1: EReg64::A0,
        rs2: EReg64::A1,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A2), 4);
}

#[test]
fn test_sra() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, (-16i64).cast_unsigned());
    regs.write(EReg64::A1, 2);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Sra {
        rd: EReg64::A2,
        rs1: EReg64::A0,
        rs2: EReg64::A1,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A2), (-4i64).cast_unsigned());
}

// Comparison Instructions

#[test]
fn test_slt_less() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, (-5i64).cast_unsigned());
    regs.write(EReg64::A1, 10);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Slt {
        rd: EReg64::A2,
        rs1: EReg64::A0,
        rs2: EReg64::A1,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A2), 1);
}

#[test]
fn test_slt_greater() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, 10);
    regs.write(EReg64::A1, (-5i64).cast_unsigned());

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Slt {
        rd: EReg64::A2,
        rs1: EReg64::A0,
        rs2: EReg64::A1,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A2), 0);
}

#[test]
fn test_sltu() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, 5);
    regs.write(EReg64::A1, (-1i64).cast_unsigned());

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Sltu {
        rd: EReg64::A2,
        rs1: EReg64::A0,
        rs2: EReg64::A1,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    // 5 < MAX unsigned
    assert_eq!(regs.read(EReg64::A2), 1);
}

// RV64 32-bit Instructions

#[test]
fn test_addw() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, 0x0000_0001_8000_0000);
    regs.write(EReg64::A1, 0x0000_0000_8000_0000);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Addw {
        rd: EReg64::A2,
        rs1: EReg64::A0,
        rs2: EReg64::A1,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    // Sign-extended result
    assert_eq!(regs.read(EReg64::A2), 0);
}

#[test]
fn test_subw() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, 0x0000_0001_0000_0000);
    regs.write(EReg64::A1, 1);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Subw {
        rd: EReg64::A2,
        rs1: EReg64::A0,
        rs2: EReg64::A1,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A2), (-1i64).cast_unsigned());
}

#[test]
fn test_sllw() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, 1);
    regs.write(EReg64::A1, 31);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Sllw {
        rd: EReg64::A2,
        rs1: EReg64::A0,
        rs2: EReg64::A1,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A2), 0xFFFF_FFFF_8000_0000);
}

#[test]
fn test_srlw() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, 0xFFFF_FFFF_8000_0000);
    regs.write(EReg64::A1, 1);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Srlw {
        rd: EReg64::A2,
        rs1: EReg64::A0,
        rs2: EReg64::A1,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A2), 0x0000_0000_4000_0000);
}

#[test]
fn test_sraw() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, 0xFFFF_FFFF_8000_0000);
    regs.write(EReg64::A1, 1);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Sraw {
        rd: EReg64::A2,
        rs1: EReg64::A0,
        rs2: EReg64::A1,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A2), 0xFFFF_FFFF_C000_0000);
}

// Immediate Instructions

#[test]
fn test_addi() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, 10);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Addi {
        rd: EReg64::A1,
        rs1: EReg64::A0,
        imm: 5,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A1), 15);
}

#[test]
fn test_addi_negative() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, 10);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Addi {
        rd: EReg64::A1,
        rs1: EReg64::A0,
        imm: -5,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A1), 5);
}

#[test]
fn test_slti() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, (-5i64).cast_unsigned());

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Slti {
        rd: EReg64::A1,
        rs1: EReg64::A0,
        imm: 10,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A1), 1);
}

#[test]
fn test_sltiu() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, 5);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Sltiu {
        rd: EReg64::A1,
        rs1: EReg64::A0,
        imm: -1,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A1), 1);
}

#[test]
fn test_xori() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, 0xFF);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Xori {
        rd: EReg64::A1,
        rs1: EReg64::A0,
        imm: 0xAA,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A1), 0x55);
}

#[test]
fn test_ori() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, 0xF0);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Ori {
        rd: EReg64::A1,
        rs1: EReg64::A0,
        imm: 0x0F,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A1), 0xFF);
}

#[test]
fn test_andi() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, 0xFF);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Andi {
        rd: EReg64::A1,
        rs1: EReg64::A0,
        imm: 0x0F,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A1), 0x0F);
}

#[test]
fn test_slli() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, 1);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Slli {
        rd: EReg64::A1,
        rs1: EReg64::A0,
        shamt: 4,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A1), 16);
}

#[test]
fn test_srli() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, 16);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Srli {
        rd: EReg64::A1,
        rs1: EReg64::A0,
        shamt: 2,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A1), 4);
}

#[test]
fn test_srai() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, (-16i64).cast_unsigned());

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Srai {
        rd: EReg64::A1,
        rs1: EReg64::A0,
        shamt: 2,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A1), (-4i64).cast_unsigned());
}

#[test]
fn test_addiw() {
    let (mut regs, mut mem, mut pc) = setup_test();

    // -5 sign-extended
    regs.write(EReg64::A0, 0xFFFF_FFFF_FFFF_FFFB);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Addiw {
        rd: EReg64::A1,
        rs1: EReg64::A0,
        imm: 5,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    // -5 + 5 = 0
    assert_eq!(regs.read(EReg64::A1), 0);
}

#[test]
fn test_slliw() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, 1);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Slliw {
        rd: EReg64::A1,
        rs1: EReg64::A0,
        shamt: 31,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A1), 0xFFFF_FFFF_8000_0000);
}

#[test]
fn test_srliw() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, 0xFFFF_FFFF_8000_0000);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Srliw {
        rd: EReg64::A1,
        rs1: EReg64::A0,
        shamt: 1,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A1), 0x0000_0000_4000_0000);
}

#[test]
fn test_sraiw() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, 0xFFFF_FFFF_8000_0000);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Sraiw {
        rd: EReg64::A1,
        rs1: EReg64::A0,
        shamt: 1,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A1), 0xFFFF_FFFF_C000_0000);
}

// Load Instructions

#[test]
fn test_lb() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let data_addr = TEST_BASE_ADDR + 0x100;
    mem.write::<i8>(data_addr + 10, -5).unwrap();
    regs.write(EReg64::A0, data_addr);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Lb {
        rd: EReg64::A1,
        rs1: EReg64::A0,
        imm: 10,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A1), (-5i64).cast_unsigned());
}

#[test]
fn test_lh() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let data_addr = TEST_BASE_ADDR + 0x100;
    mem.write::<i16>(data_addr, -300).unwrap();
    regs.write(EReg64::A0, data_addr);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Lh {
        rd: EReg64::A1,
        rs1: EReg64::A0,
        imm: 0,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A1), (-300i64).cast_unsigned());
}

#[test]
fn test_lw() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let data_addr = TEST_BASE_ADDR + 0x100;
    mem.write::<i32>(data_addr, -100000).unwrap();
    regs.write(EReg64::A0, data_addr);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Lw {
        rd: EReg64::A1,
        rs1: EReg64::A0,
        imm: 0,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A1), (-100000i64).cast_unsigned());
}

#[test]
fn test_ld() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let data_addr = TEST_BASE_ADDR + 0x100;
    mem.write::<u64>(data_addr, 0x1234_5678_9ABC_DEF0).unwrap();
    regs.write(EReg64::A0, data_addr);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Ld {
        rd: EReg64::A1,
        rs1: EReg64::A0,
        imm: 0,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A1), 0x1234_5678_9ABC_DEF0);
}

#[test]
fn test_lbu() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let data_addr = TEST_BASE_ADDR + 0x100;
    mem.write::<u8>(data_addr, 0xFF).unwrap();
    regs.write(EReg64::A0, data_addr);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Lbu {
        rd: EReg64::A1,
        rs1: EReg64::A0,
        imm: 0,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A1), 0xFF);
}

#[test]
fn test_lhu() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let data_addr = TEST_BASE_ADDR + 0x100;
    mem.write::<u16>(data_addr, 0xFFFF).unwrap();
    regs.write(EReg64::A0, data_addr);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Lhu {
        rd: EReg64::A1,
        rs1: EReg64::A0,
        imm: 0,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A1), 0xFFFF);
}

#[test]
fn test_lwu() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let data_addr = TEST_BASE_ADDR + 0x100;
    mem.write::<u32>(data_addr, 0xFFFF_FFFF).unwrap();
    regs.write(EReg64::A0, data_addr);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Lwu {
        rd: EReg64::A1,
        rs1: EReg64::A0,
        imm: 0,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A1), 0xFFFF_FFFF);
}

// Store Instructions

#[test]
fn test_sb() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let data_addr = TEST_BASE_ADDR + 0x100;
    regs.write(EReg64::A0, data_addr);
    regs.write(EReg64::A1, 0x12);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Sb {
        rs1: EReg64::A0,
        rs2: EReg64::A1,
        imm: 0,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(mem.read::<u8>(data_addr).unwrap(), 0x12);
}

#[test]
fn test_sh() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let data_addr = TEST_BASE_ADDR + 0x100;
    regs.write(EReg64::A0, data_addr);
    regs.write(EReg64::A1, 0x1234);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Sh {
        rs1: EReg64::A0,
        rs2: EReg64::A1,
        imm: 0,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(mem.read::<u16>(data_addr).unwrap(), 0x1234);
}

#[test]
fn test_sw() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let data_addr = TEST_BASE_ADDR + 0x100;
    regs.write(EReg64::A0, data_addr);
    regs.write(EReg64::A1, 0x1234_5678);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Sw {
        rs1: EReg64::A0,
        rs2: EReg64::A1,
        imm: 0,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(mem.read::<u32>(data_addr).unwrap(), 0x1234_5678);
}

#[test]
fn test_sd() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let data_addr = TEST_BASE_ADDR + 0x100;
    regs.write(EReg64::A0, data_addr);
    regs.write(EReg64::A1, 0x1234_5678_9ABC_DEF0);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Sd {
        rs1: EReg64::A0,
        rs2: EReg64::A1,
        imm: 0,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(mem.read::<u64>(data_addr).unwrap(), 0x1234_5678_9ABC_DEF0);
}

// Branch Instructions

#[test]
fn test_beq_taken() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, 10);
    regs.write(EReg64::A1, 10);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Beq {
        rs1: EReg64::A0,
        rs2: EReg64::A1,
        // Branch offset from PC before increment
        imm: 8,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    let initial_pc = pc;
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    // Branch calculates: old_pc (stored before fetch incremented it) + offset
    // The implementation stores old_pc before PC is incremented
    // So: initial_pc + 8 = 0x1000 + 8 = 0x1008
    assert_eq!(pc, initial_pc.wrapping_add(8));
}

#[test]
fn test_beq_not_taken() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, 10);
    regs.write(EReg64::A1, 20);

    let instructions = vec![
        Rv64MBZbcInstruction::Base(Rv64Instruction::Beq {
            rs1: EReg64::A0,
            rs2: EReg64::A1,
            imm: 8,
        }),
        Rv64MBZbcInstruction::Base(Rv64Instruction::Addi {
            rd: EReg64::A2,
            rs1: EReg64::Zero,
            // This should execute
            imm: 99,
        }),
    ];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    // Verify the branch was NOT taken - next instruction executed
    assert_eq!(regs.read(EReg64::A2), 99);
}

#[test]
fn test_bne_taken() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, 10);
    regs.write(EReg64::A1, 20);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Bne {
        rs1: EReg64::A0,
        rs2: EReg64::A1,
        imm: 8,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    let initial_pc = pc;
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(pc, initial_pc.wrapping_add(8));
}

#[test]
fn test_blt_taken() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, (-10i64).cast_unsigned());
    regs.write(EReg64::A1, 10);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Blt {
        rs1: EReg64::A0,
        rs2: EReg64::A1,
        imm: 12,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    let initial_pc = pc;
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(pc, initial_pc.wrapping_add(12));
}

#[test]
fn test_bge_taken() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, 10);
    regs.write(EReg64::A1, 10);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Bge {
        rs1: EReg64::A0,
        rs2: EReg64::A1,
        imm: 16,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    let initial_pc = pc;
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(pc, initial_pc.wrapping_add(16));
}

#[test]
fn test_bltu_taken() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, 10);
    regs.write(EReg64::A1, 20);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Bltu {
        rs1: EReg64::A0,
        rs2: EReg64::A1,
        imm: 20,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    let initial_pc = pc;
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(pc, initial_pc.wrapping_add(20));
}

#[test]
fn test_bgeu_taken() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg64::A0, 20);
    regs.write(EReg64::A1, 10);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Bgeu {
        rs1: EReg64::A0,
        rs2: EReg64::A1,
        imm: 24,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    let initial_pc = pc;
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(pc, initial_pc.wrapping_add(24));
}

// Jump Instructions

#[test]
fn test_jal() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let initial_pc = pc;
    regs.write(EReg64::A2, 0);

    let instructions = vec![
        Rv64MBZbcInstruction::Base(Rv64Instruction::Jal {
            rd: EReg64::Ra,
            // Skip next instruction
            imm: 8,
        }),
        Rv64MBZbcInstruction::Base(Rv64Instruction::Addi {
            rd: EReg64::A2,
            rs1: EReg64::Zero,
            // Should be skipped
            imm: 99,
        }),
        Rv64MBZbcInstruction::Base(Rv64Instruction::Addi {
            rd: EReg64::A2,
            rs1: EReg64::Zero,
            // Should execute
            imm: 42,
        }),
    ];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    // Return address
    assert_eq!(regs.read(EReg64::Ra), initial_pc + 4);
    assert_eq!(regs.read(EReg64::A2), 42);
}

#[test]
fn test_jalr() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let initial_pc = pc;
    let target_addr = TEST_BASE_ADDR + 8;
    regs.write(EReg64::A0, target_addr);
    regs.write(EReg64::A2, 0);

    let instructions = vec![
        Rv64MBZbcInstruction::Base(Rv64Instruction::Jalr {
            rd: EReg64::Ra,
            rs1: EReg64::A0,
            imm: 0,
        }),
        Rv64MBZbcInstruction::Base(Rv64Instruction::Addi {
            rd: EReg64::A2,
            rs1: EReg64::Zero,
            // Should be skipped
            imm: 99,
        }),
        Rv64MBZbcInstruction::Base(Rv64Instruction::Addi {
            rd: EReg64::A2,
            rs1: EReg64::Zero,
            // Should execute
            imm: 42,
        }),
    ];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    // Return address
    assert_eq!(regs.read(EReg64::Ra), initial_pc + 4);
    assert_eq!(regs.read(EReg64::A2), 42);
}

#[test]
fn test_jalr_clear_lsb() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let initial_pc = pc;
    // Odd address
    regs.write(EReg64::A0, TEST_BASE_ADDR + 9);
    regs.write(EReg64::A2, 0);

    let instructions = vec![
        Rv64MBZbcInstruction::Base(Rv64Instruction::Jalr {
            rd: EReg64::Ra,
            rs1: EReg64::A0,
            imm: 0,
        }),
        Rv64MBZbcInstruction::Base(Rv64Instruction::Addi {
            rd: EReg64::A2,
            rs1: EReg64::Zero,
            imm: 99,
        }),
        Rv64MBZbcInstruction::Base(Rv64Instruction::Addi {
            rd: EReg64::A2,
            rs1: EReg64::Zero,
            imm: 42,
        }),
    ];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::Ra), initial_pc + 4);
    // LSB cleared: 9 -> 8
    assert_eq!(regs.read(EReg64::A2), 42);
}

// Upper Immediate Instructions

#[test]
fn test_lui() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Lui {
        rd: EReg64::A0,
        // Already shifted - bits [31:12]
        imm: 0x12345000,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A0), 0x12345000u64);
}

#[test]
fn test_lui_negative() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Lui {
        rd: EReg64::A0,
        // 0xFFFFF000 as upper 20 bits (already shifted)
        imm: 0xfffff000u32.cast_signed(),
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    // Should be sign-extended: 0xfffffffffffff000
    assert_eq!(regs.read(EReg64::A0), 0xfffffffffffff000u64);
}

#[test]
fn test_auipc() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let initial_pc = pc;

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Auipc {
        rd: EReg64::A0,
        // Already shifted - bits [31:12]
        imm: 0x12345000,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(
        regs.read(EReg64::A0),
        initial_pc.wrapping_add(0x12345000u64)
    );
}

#[test]
fn test_auipc_negative() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let initial_pc = pc;

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Auipc {
        rd: EReg64::A0,
        // Negative immediate (all upper bits set)
        imm: 0xfffff000u32.cast_signed(),
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    // Should wrap around: PC + sign_extend(0xfffff000)
    assert_eq!(
        regs.read(EReg64::A0),
        initial_pc.wrapping_add(0xfffffffffffff000u64)
    );
}

// Special Instructions

#[test]
fn test_fence() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Fence {
        pred: 0xF,
        succ: 0xF,
        fm: 0,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    // Should execute without error (NOP in single-threaded)
}

#[test]
fn test_ebreak() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Ebreak)];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    // Should execute without error (NOP by default)
}

#[test]
fn test_ecall_unsupported() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Ecall)];

    let mut handler = TestInstructionHandler::new(instructions);
    let result = execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler);

    assert!(matches!(
        result,
        Err(ExecuteError::UnsupportedInstruction { .. })
    ));
}

#[test]
fn test_unimp() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Unimp)];

    let mut handler = TestInstructionHandler::new(instructions);
    let result = execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler);

    assert!(matches!(result, Err(ExecuteError::UnimpInstruction { .. })));
}

// Error Conditions

#[test]
fn test_out_of_bounds_read() {
    let (mut regs, mut mem, mut pc) = setup_test();

    // Invalid address
    regs.write(EReg64::A0, 0x0);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Ld {
        rd: EReg64::A1,
        rs1: EReg64::A0,
        imm: 0,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    let result = execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler);

    assert!(matches!(result, Err(ExecuteError::MemoryAccess(_))));
}

#[test]
fn test_out_of_bounds_write() {
    let (mut regs, mut mem, mut pc) = setup_test();

    // Invalid address
    regs.write(EReg64::A0, 0x0);
    regs.write(EReg64::A1, 42);

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Sd {
        rs1: EReg64::A0,
        rs2: EReg64::A1,
        imm: 0,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    let result = execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler);

    assert!(matches!(result, Err(ExecuteError::MemoryAccess(_))));
}

// Register Zero Tests

#[test]
fn test_write_to_zero_register() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Addi {
        rd: EReg64::Zero,
        rs1: EReg64::Zero,
        imm: 100,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::Zero), 0);
}

#[test]
fn test_read_from_zero_register() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let instructions = vec![Rv64MBZbcInstruction::Base(Rv64Instruction::Add {
        rd: EReg64::A0,
        rs1: EReg64::Zero,
        rs2: EReg64::Zero,
    })];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg64::A0), 0);
}

// Complex Programs

#[test]
fn test_fibonacci() {
    let (mut regs, mut mem, mut pc) = setup_test();

    // Calculate fib(10)
    // fib(0) = 0, fib(1) = 1, fib(2) = 1, ..., fib(10) = 55

    // fib(n-2) = fib(0)
    regs.write(EReg64::A1, 0);
    // fib(n-1) = fib(1)
    regs.write(EReg64::A2, 1);

    // Fibonacci loop - iterate 9 times to go from fib(1) to fib(10)
    let mut instructions = vec![];

    for _ in 0..9 {
        // a3 = a1 + a2 (next fib number)
        instructions.push(Rv64MBZbcInstruction::Base(Rv64Instruction::Add {
            rd: EReg64::A3,
            rs1: EReg64::A1,
            rs2: EReg64::A2,
        }));
        // a1 = a2 (shift window)
        instructions.push(Rv64MBZbcInstruction::Base(Rv64Instruction::Add {
            rd: EReg64::A1,
            rs1: EReg64::A2,
            rs2: EReg64::Zero,
        }));
        // a2 = a3 (shift window)
        instructions.push(Rv64MBZbcInstruction::Base(Rv64Instruction::Add {
            rd: EReg64::A2,
            rs1: EReg64::A3,
            rs2: EReg64::Zero,
        }));
    }

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64mbzbc(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    // fib(10) = 55
    assert_eq!(regs.read(EReg64::A2), 55);
}
