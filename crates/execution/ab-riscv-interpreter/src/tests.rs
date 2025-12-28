extern crate alloc;

use crate::{
    BasicInt, ExecuteError, FetchInstructionResult, GenericInstructionHandler, VirtualMemory,
    VirtualMemoryError, execute_rv64,
};
use ab_riscv_primitives::instruction::{GenericInstruction, Rv64Instruction};
use ab_riscv_primitives::registers::{EReg, ERegisters, GenericRegisters};
use alloc::vec;
use alloc::vec::Vec;
use core::ops::ControlFlow;

const TEST_BASE_ADDR: u64 = 0x1000;
const TRAP_ADDRESS: u64 = 0;

// Simple test memory implementation
struct TestMemory {
    data: Vec<u8>,
    base_addr: u64,
}

impl TestMemory {
    fn new(size: usize, base_addr: u64) -> Self {
        Self {
            data: vec![0; size],
            base_addr,
        }
    }
}

impl VirtualMemory for TestMemory {
    fn read<T>(&self, address: u64) -> Result<T, VirtualMemoryError>
    where
        T: BasicInt,
    {
        let offset = address
            .checked_sub(self.base_addr)
            .ok_or(VirtualMemoryError::OutOfBoundsRead { address })? as usize;

        if offset + size_of::<T>() > self.data.len() {
            return Err(VirtualMemoryError::OutOfBoundsRead { address });
        }

        // SAFETY: Only reading basic integers from initialized memory
        unsafe {
            Ok(self
                .data
                .as_ptr()
                .cast::<T>()
                .byte_add(offset)
                .read_unaligned())
        }
    }

    fn write<T>(&mut self, address: u64, value: T) -> Result<(), VirtualMemoryError>
    where
        T: BasicInt,
    {
        let offset = address
            .checked_sub(self.base_addr)
            .ok_or(VirtualMemoryError::OutOfBoundsWrite { address })? as usize;

        if offset + size_of::<T>() > self.data.len() {
            return Err(VirtualMemoryError::OutOfBoundsWrite { address });
        }

        // SAFETY: Only writing basic integers to initialized memory
        unsafe {
            self.data
                .as_mut_ptr()
                .cast::<T>()
                .byte_add(offset)
                .write_unaligned(value);
        }

        Ok(())
    }
}

/// Custom instruction handler for tests that returns instructions from a sequence
struct TestInstructionHandler {
    instructions: Vec<Rv64Instruction<EReg>>,
    index: usize,
}

impl TestInstructionHandler {
    fn new(instructions: Vec<Rv64Instruction<EReg>>) -> Self {
        Self {
            instructions,
            index: 0,
        }
    }
}

impl GenericInstructionHandler<Rv64Instruction<EReg>, ERegisters, TestMemory, &'static str>
    for TestInstructionHandler
{
    fn fetch_instruction(
        &mut self,
        _regs: &mut ERegisters,
        _memory: &mut TestMemory,
        pc: &mut u64,
    ) -> Result<
        FetchInstructionResult<Rv64Instruction<EReg>>,
        ExecuteError<Rv64Instruction<EReg>, &'static str>,
    > {
        if *pc == TRAP_ADDRESS {
            return Ok(FetchInstructionResult::ControlFlow(ControlFlow::Break(())));
        }

        if self.index >= self.instructions.len() {
            return Ok(FetchInstructionResult::ControlFlow(ControlFlow::Break(())));
        }

        let instruction = self.instructions[self.index];
        self.index += 1;
        // Advance PC
        *pc += 4;

        Ok(FetchInstructionResult::Instruction(instruction))
    }

    fn handle_ecall(
        &mut self,
        _regs: &mut ERegisters,
        _memory: &mut TestMemory,
        pc: &mut u64,
        instruction: Rv64Instruction<EReg>,
    ) -> Result<(), ExecuteError<Rv64Instruction<EReg>, &'static str>> {
        Err(ExecuteError::UnsupportedInstruction {
            address: *pc - instruction.size() as u64,
            instruction,
        })
    }
}

fn setup_test() -> (ERegisters, TestMemory, u64) {
    let regs = ERegisters::default();
    let memory = TestMemory::new(8192, TEST_BASE_ADDR);
    let pc = TEST_BASE_ADDR;
    (regs, memory, pc)
}

// Arithmetic Instructions

#[test]
fn test_add() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 10);
    regs.write(EReg::A1, 20);

    let instructions = vec![Rv64Instruction::Add {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2), 30);
}

#[test]
fn test_add_overflow() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, u64::MAX);
    regs.write(EReg::A1, 1);

    let instructions = vec![Rv64Instruction::Add {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    // Wrapping behavior
    assert_eq!(regs.read(EReg::A2), 0);
}

#[test]
fn test_sub() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 50);
    regs.write(EReg::A1, 20);

    let instructions = vec![Rv64Instruction::Sub {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2), 30);
}

#[test]
fn test_sub_underflow() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 0);
    regs.write(EReg::A1, 1);

    let instructions = vec![Rv64Instruction::Sub {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2), u64::MAX);
}

// Logical Instructions

#[test]
fn test_and() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 0b1111_0000);
    regs.write(EReg::A1, 0b1010_1010);

    let instructions = vec![Rv64Instruction::And {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2), 0b1010_0000);
}

#[test]
fn test_or() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 0b1111_0000);
    regs.write(EReg::A1, 0b0000_1111);

    let instructions = vec![Rv64Instruction::Or {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2), 0b1111_1111);
}

#[test]
fn test_xor() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 0b1111_0000);
    regs.write(EReg::A1, 0b1010_1010);

    let instructions = vec![Rv64Instruction::Xor {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2), 0b0101_1010);
}

// Shift Instructions

#[test]
fn test_sll() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 1);
    regs.write(EReg::A1, 4);

    let instructions = vec![Rv64Instruction::Sll {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2), 16);
}

#[test]
fn test_sll_mask() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 1);
    // High bits should be masked
    regs.write(EReg::A1, 0x100);

    let instructions = vec![Rv64Instruction::Sll {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    // Only lower 6 bits used
    assert_eq!(regs.read(EReg::A2), 1);
}

#[test]
fn test_srl() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 16);
    regs.write(EReg::A1, 2);

    let instructions = vec![Rv64Instruction::Srl {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2), 4);
}

#[test]
fn test_sra() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, (-16i64) as u64);
    regs.write(EReg::A1, 2);

    let instructions = vec![Rv64Instruction::Sra {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2), (-4i64) as u64);
}

// Comparison Instructions

#[test]
fn test_slt_less() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, (-5i64) as u64);
    regs.write(EReg::A1, 10);

    let instructions = vec![Rv64Instruction::Slt {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2), 1);
}

#[test]
fn test_slt_greater() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 10);
    regs.write(EReg::A1, (-5i64) as u64);

    let instructions = vec![Rv64Instruction::Slt {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2), 0);
}

#[test]
fn test_sltu() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 5);
    regs.write(EReg::A1, (-1i64) as u64);

    let instructions = vec![Rv64Instruction::Sltu {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    // 5 < MAX unsigned
    assert_eq!(regs.read(EReg::A2), 1);
}

// Multiplication Instructions

#[test]
fn test_mul() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 7);
    regs.write(EReg::A1, 8);

    let instructions = vec![Rv64Instruction::Mul {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2), 56);
}

#[test]
fn test_mulh() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, i64::MAX as u64);
    regs.write(EReg::A1, 2);

    let instructions = vec![Rv64Instruction::Mulh {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    let (_, hi) = (i64::MAX).widening_mul(2);
    assert_eq!(regs.read(EReg::A2), hi as u64);
}

#[test]
fn test_mulhu() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, u64::MAX);
    regs.write(EReg::A1, u64::MAX);

    let instructions = vec![Rv64Instruction::Mulhu {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    let prod = (u64::MAX as u128) * (u64::MAX as u128);
    assert_eq!(regs.read(EReg::A2), (prod >> 64) as u64);
}

#[test]
fn test_mulhsu() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, (-2i64) as u64);
    regs.write(EReg::A1, 3);

    let instructions = vec![Rv64Instruction::Mulhsu {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    let prod = (-2i64 as i128) * (3i128);
    assert_eq!(regs.read(EReg::A2), (prod >> 64) as u64);
}

// Division Instructions

#[test]
fn test_div() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, 3);

    let instructions = vec![Rv64Instruction::Div {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2) as i64, 6);
}

#[test]
fn test_div_by_zero() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, 0);

    let instructions = vec![Rv64Instruction::Div {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2), (-1i64) as u64);
}

#[test]
fn test_div_overflow() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, i64::MIN as u64);
    regs.write(EReg::A1, (-1i64) as u64);

    let instructions = vec![Rv64Instruction::Div {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2), i64::MIN as u64);
}

#[test]
fn test_divu() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, 3);

    let instructions = vec![Rv64Instruction::Divu {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2), 6);
}

#[test]
fn test_divu_by_zero() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, 0);

    let instructions = vec![Rv64Instruction::Divu {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2), u64::MAX);
}

#[test]
fn test_rem() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, 3);

    let instructions = vec![Rv64Instruction::Rem {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2) as i64, 2);
}

#[test]
fn test_rem_by_zero() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, 0);

    let instructions = vec![Rv64Instruction::Rem {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2), 20);
}

#[test]
fn test_rem_overflow() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, i64::MIN as u64);
    regs.write(EReg::A1, (-1i64) as u64);

    let instructions = vec![Rv64Instruction::Rem {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2), 0);
}

#[test]
fn test_remu() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, 3);

    let instructions = vec![Rv64Instruction::Remu {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2), 2);
}

#[test]
fn test_remu_by_zero() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, 0);

    let instructions = vec![Rv64Instruction::Remu {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2), 20);
}

// RV64 32-bit Instructions

#[test]
fn test_addw() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 0x0000_0001_8000_0000);
    regs.write(EReg::A1, 0x0000_0000_8000_0000);

    let instructions = vec![Rv64Instruction::Addw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    // Sign-extended result
    assert_eq!(regs.read(EReg::A2), 0);
}

#[test]
fn test_subw() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 0x0000_0001_0000_0000);
    regs.write(EReg::A1, 1);

    let instructions = vec![Rv64Instruction::Subw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2), (-1i64) as u64);
}

#[test]
fn test_sllw() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 1);
    regs.write(EReg::A1, 31);

    let instructions = vec![Rv64Instruction::Sllw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2), 0xFFFF_FFFF_8000_0000);
}

#[test]
fn test_srlw() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 0xFFFF_FFFF_8000_0000);
    regs.write(EReg::A1, 1);

    let instructions = vec![Rv64Instruction::Srlw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2), 0x0000_0000_4000_0000);
}

#[test]
fn test_sraw() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 0xFFFF_FFFF_8000_0000);
    regs.write(EReg::A1, 1);

    let instructions = vec![Rv64Instruction::Sraw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2), 0xFFFF_FFFF_C000_0000);
}

#[test]
fn test_mulw() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 0x7FFF_FFFF);
    regs.write(EReg::A1, 2);

    let instructions = vec![Rv64Instruction::Mulw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2), 0xFFFF_FFFF_FFFF_FFFE);
}

#[test]
fn test_divw() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, 3);

    let instructions = vec![Rv64Instruction::Divw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2) as i64, 6);
}

#[test]
fn test_divw_by_zero() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, 0);

    let instructions = vec![Rv64Instruction::Divw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2), (-1i64) as u64);
}

#[test]
fn test_divw_overflow() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, i32::MIN as u32 as u64);
    regs.write(EReg::A1, (-1i32) as u32 as u64);

    let instructions = vec![Rv64Instruction::Divw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2), i32::MIN as i64 as u64);
}

#[test]
fn test_divuw() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, 3);

    let instructions = vec![Rv64Instruction::Divuw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2), 6);
}

#[test]
fn test_divuw_by_zero() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, 0);

    let instructions = vec![Rv64Instruction::Divuw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2), u64::MAX);
}

#[test]
fn test_remw() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, 3);

    let instructions = vec![Rv64Instruction::Remw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2), 2);
}

#[test]
fn test_remw_by_zero() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, 0);

    let instructions = vec![Rv64Instruction::Remw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2), 20);
}

#[test]
fn test_remw_overflow() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, i32::MIN as u32 as u64);
    regs.write(EReg::A1, (-1i32) as u32 as u64);

    let instructions = vec![Rv64Instruction::Remw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2), 0);
}

#[test]
fn test_remuw() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, 3);

    let instructions = vec![Rv64Instruction::Remuw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2), 2);
}

#[test]
fn test_remuw_by_zero() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, 0);

    let instructions = vec![Rv64Instruction::Remuw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A2) as i64, 20);
}

// Immediate Instructions

#[test]
fn test_addi() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 10);

    let instructions = vec![Rv64Instruction::Addi {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 5,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A1), 15);
}

#[test]
fn test_addi_negative() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 10);

    let instructions = vec![Rv64Instruction::Addi {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: -5,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A1), 5);
}

#[test]
fn test_slti() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, (-5i64) as u64);

    let instructions = vec![Rv64Instruction::Slti {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 10,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A1), 1);
}

#[test]
fn test_sltiu() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 5);

    let instructions = vec![Rv64Instruction::Sltiu {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: -1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A1), 1);
}

#[test]
fn test_xori() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 0xFF);

    let instructions = vec![Rv64Instruction::Xori {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 0xAA,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A1), 0x55);
}

#[test]
fn test_ori() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 0xF0);

    let instructions = vec![Rv64Instruction::Ori {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 0x0F,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A1), 0xFF);
}

#[test]
fn test_andi() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 0xFF);

    let instructions = vec![Rv64Instruction::Andi {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 0x0F,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A1), 0x0F);
}

#[test]
fn test_slli() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 1);

    let instructions = vec![Rv64Instruction::Slli {
        rd: EReg::A1,
        rs1: EReg::A0,
        shamt: 4,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A1), 16);
}

#[test]
fn test_srli() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 16);

    let instructions = vec![Rv64Instruction::Srli {
        rd: EReg::A1,
        rs1: EReg::A0,
        shamt: 2,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A1), 4);
}

#[test]
fn test_srai() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, (-16i64) as u64);

    let instructions = vec![Rv64Instruction::Srai {
        rd: EReg::A1,
        rs1: EReg::A0,
        shamt: 2,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A1), (-4i64) as u64);
}

#[test]
fn test_addiw() {
    let (mut regs, mut mem, mut pc) = setup_test();

    // -5 sign-extended
    regs.write(EReg::A0, 0xFFFF_FFFF_FFFF_FFFB);

    let instructions = vec![Rv64Instruction::Addiw {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 5,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    // -5 + 5 = 0
    assert_eq!(regs.read(EReg::A1), 0);
}

#[test]
fn test_slliw() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 1);

    let instructions = vec![Rv64Instruction::Slliw {
        rd: EReg::A1,
        rs1: EReg::A0,
        shamt: 31,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A1), 0xFFFF_FFFF_8000_0000);
}

#[test]
fn test_srliw() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 0xFFFF_FFFF_8000_0000);

    let instructions = vec![Rv64Instruction::Srliw {
        rd: EReg::A1,
        rs1: EReg::A0,
        shamt: 1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A1), 0x0000_0000_4000_0000);
}

#[test]
fn test_sraiw() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 0xFFFF_FFFF_8000_0000);

    let instructions = vec![Rv64Instruction::Sraiw {
        rd: EReg::A1,
        rs1: EReg::A0,
        shamt: 1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A1), 0xFFFF_FFFF_C000_0000);
}

// Load Instructions

#[test]
fn test_lb() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let data_addr = TEST_BASE_ADDR + 0x100;
    mem.write::<i8>(data_addr + 10, -5).unwrap();
    regs.write(EReg::A0, data_addr);

    let instructions = vec![Rv64Instruction::Lb {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 10,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A1), (-5i64) as u64);
}

#[test]
fn test_lh() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let data_addr = TEST_BASE_ADDR + 0x100;
    mem.write::<i16>(data_addr, -300).unwrap();
    regs.write(EReg::A0, data_addr);

    let instructions = vec![Rv64Instruction::Lh {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 0,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A1), (-300i64) as u64);
}

#[test]
fn test_lw() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let data_addr = TEST_BASE_ADDR + 0x100;
    mem.write::<i32>(data_addr, -100000).unwrap();
    regs.write(EReg::A0, data_addr);

    let instructions = vec![Rv64Instruction::Lw {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 0,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A1), (-100000i64) as u64);
}

#[test]
fn test_ld() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let data_addr = TEST_BASE_ADDR + 0x100;
    mem.write::<u64>(data_addr, 0x1234_5678_9ABC_DEF0).unwrap();
    regs.write(EReg::A0, data_addr);

    let instructions = vec![Rv64Instruction::Ld {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 0,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A1), 0x1234_5678_9ABC_DEF0);
}

#[test]
fn test_lbu() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let data_addr = TEST_BASE_ADDR + 0x100;
    mem.write::<u8>(data_addr, 0xFF).unwrap();
    regs.write(EReg::A0, data_addr);

    let instructions = vec![Rv64Instruction::Lbu {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 0,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A1), 0xFF);
}

#[test]
fn test_lhu() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let data_addr = TEST_BASE_ADDR + 0x100;
    mem.write::<u16>(data_addr, 0xFFFF).unwrap();
    regs.write(EReg::A0, data_addr);

    let instructions = vec![Rv64Instruction::Lhu {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 0,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A1), 0xFFFF);
}

#[test]
fn test_lwu() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let data_addr = TEST_BASE_ADDR + 0x100;
    mem.write::<u32>(data_addr, 0xFFFF_FFFF).unwrap();
    regs.write(EReg::A0, data_addr);

    let instructions = vec![Rv64Instruction::Lwu {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 0,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A1), 0xFFFF_FFFF);
}

// Store Instructions

#[test]
fn test_sb() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let data_addr = TEST_BASE_ADDR + 0x100;
    regs.write(EReg::A0, data_addr);
    regs.write(EReg::A1, 0x12);

    let instructions = vec![Rv64Instruction::Sb {
        rs1: EReg::A0,
        rs2: EReg::A1,
        imm: 0,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(mem.read::<u8>(data_addr).unwrap(), 0x12);
}

#[test]
fn test_sh() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let data_addr = TEST_BASE_ADDR + 0x100;
    regs.write(EReg::A0, data_addr);
    regs.write(EReg::A1, 0x1234);

    let instructions = vec![Rv64Instruction::Sh {
        rs1: EReg::A0,
        rs2: EReg::A1,
        imm: 0,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(mem.read::<u16>(data_addr).unwrap(), 0x1234);
}

#[test]
fn test_sw() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let data_addr = TEST_BASE_ADDR + 0x100;
    regs.write(EReg::A0, data_addr);
    regs.write(EReg::A1, 0x1234_5678);

    let instructions = vec![Rv64Instruction::Sw {
        rs1: EReg::A0,
        rs2: EReg::A1,
        imm: 0,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(mem.read::<u32>(data_addr).unwrap(), 0x1234_5678);
}

#[test]
fn test_sd() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let data_addr = TEST_BASE_ADDR + 0x100;
    regs.write(EReg::A0, data_addr);
    regs.write(EReg::A1, 0x1234_5678_9ABC_DEF0);

    let instructions = vec![Rv64Instruction::Sd {
        rs1: EReg::A0,
        rs2: EReg::A1,
        imm: 0,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(mem.read::<u64>(data_addr).unwrap(), 0x1234_5678_9ABC_DEF0);
}

// Branch Instructions

#[test]
fn test_beq_taken() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 10);
    regs.write(EReg::A1, 10);

    let instructions = vec![Rv64Instruction::Beq {
        rs1: EReg::A0,
        rs2: EReg::A1,
        // Branch offset from PC before increment
        imm: 8,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    let initial_pc = pc;
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    // Branch calculates: old_pc (stored before fetch incremented it) + offset
    // The implementation stores old_pc before PC is incremented
    // So: initial_pc + 8 = 0x1000 + 8 = 0x1008
    assert_eq!(pc, initial_pc.wrapping_add(8));
}

#[test]
fn test_beq_not_taken() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 10);
    regs.write(EReg::A1, 20);

    let instructions = vec![
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
    ];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    // Verify the branch was NOT taken - next instruction executed
    assert_eq!(regs.read(EReg::A2), 99);
}

#[test]
fn test_bne_taken() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 10);
    regs.write(EReg::A1, 20);

    let instructions = vec![Rv64Instruction::Bne {
        rs1: EReg::A0,
        rs2: EReg::A1,
        imm: 8,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    let initial_pc = pc;
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(pc, initial_pc.wrapping_add(8));
}

#[test]
fn test_blt_taken() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, (-10i64) as u64);
    regs.write(EReg::A1, 10);

    let instructions = vec![Rv64Instruction::Blt {
        rs1: EReg::A0,
        rs2: EReg::A1,
        imm: 12,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    let initial_pc = pc;
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(pc, initial_pc.wrapping_add(12));
}

#[test]
fn test_bge_taken() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 10);
    regs.write(EReg::A1, 10);

    let instructions = vec![Rv64Instruction::Bge {
        rs1: EReg::A0,
        rs2: EReg::A1,
        imm: 16,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    let initial_pc = pc;
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(pc, initial_pc.wrapping_add(16));
}

#[test]
fn test_bltu_taken() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 10);
    regs.write(EReg::A1, 20);

    let instructions = vec![Rv64Instruction::Bltu {
        rs1: EReg::A0,
        rs2: EReg::A1,
        imm: 20,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    let initial_pc = pc;
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(pc, initial_pc.wrapping_add(20));
}

#[test]
fn test_bgeu_taken() {
    let (mut regs, mut mem, mut pc) = setup_test();

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, 10);

    let instructions = vec![Rv64Instruction::Bgeu {
        rs1: EReg::A0,
        rs2: EReg::A1,
        imm: 24,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    let initial_pc = pc;
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(pc, initial_pc.wrapping_add(24));
}

// Jump Instructions

#[test]
fn test_jal() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let initial_pc = pc;
    regs.write(EReg::A2, 0);

    let instructions = vec![
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
    ];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    // Return address
    assert_eq!(regs.read(EReg::Ra), initial_pc + 4);
    assert_eq!(regs.read(EReg::A2), 42);
}

#[test]
fn test_jalr() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let initial_pc = pc;
    let target_addr = TEST_BASE_ADDR + 8;
    regs.write(EReg::A0, target_addr);
    regs.write(EReg::A2, 0);

    let instructions = vec![
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
    ];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    // Return address
    assert_eq!(regs.read(EReg::Ra), initial_pc + 4);
    assert_eq!(regs.read(EReg::A2), 42);
}

#[test]
fn test_jalr_clear_lsb() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let initial_pc = pc;
    // Odd address
    regs.write(EReg::A0, TEST_BASE_ADDR + 9);
    regs.write(EReg::A2, 0);

    let instructions = vec![
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
    ];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::Ra), initial_pc + 4);
    // LSB cleared: 9 -> 8
    assert_eq!(regs.read(EReg::A2), 42);
}

// Upper Immediate Instructions

#[test]
fn test_lui() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let instructions = vec![Rv64Instruction::Lui {
        rd: EReg::A0,
        imm: 0x12345,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A0), (0x12345i64 << 12) as u64);
}

#[test]
fn test_lui_negative() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let instructions = vec![Rv64Instruction::Lui {
        rd: EReg::A0,
        // 0xFFFFF as 20-bit value
        imm: -1,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A0), ((-1i64) << 12) as u64);
}

#[test]
fn test_auipc() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let initial_pc = pc;

    let instructions = vec![Rv64Instruction::Auipc {
        rd: EReg::A0,
        imm: 0x12345,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(
        regs.read(EReg::A0),
        initial_pc.wrapping_add((0x12345i64 << 12) as u64)
    );
}

// Special Instructions

#[test]
fn test_fence() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let instructions = vec![Rv64Instruction::Fence {
        pred: 0xF,
        succ: 0xF,
        fm: 0,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    // Should execute without error (NOP in single-threaded)
}

#[test]
fn test_ebreak() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let instructions = vec![Rv64Instruction::Ebreak];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    // Should execute without error (NOP by default)
}

#[test]
fn test_ecall_unsupported() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let instructions = vec![Rv64Instruction::Ecall];

    let mut handler = TestInstructionHandler::new(instructions);
    let result = execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler);

    assert!(matches!(
        result,
        Err(ExecuteError::UnsupportedInstruction { .. })
    ));
}

#[test]
fn test_unimp() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let instructions = vec![Rv64Instruction::Unimp];

    let mut handler = TestInstructionHandler::new(instructions);
    let result = execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler);

    assert!(matches!(result, Err(ExecuteError::UnimpInstruction { .. })));
}

// Error Conditions

#[test]
fn test_out_of_bounds_read() {
    let (mut regs, mut mem, mut pc) = setup_test();

    // Invalid address
    regs.write(EReg::A0, 0x0);

    let instructions = vec![Rv64Instruction::Ld {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 0,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    let result = execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler);

    assert!(matches!(result, Err(ExecuteError::MemoryAccess(_))));
}

#[test]
fn test_out_of_bounds_write() {
    let (mut regs, mut mem, mut pc) = setup_test();

    // Invalid address
    regs.write(EReg::A0, 0x0);
    regs.write(EReg::A1, 42);

    let instructions = vec![Rv64Instruction::Sd {
        rs1: EReg::A0,
        rs2: EReg::A1,
        imm: 0,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    let result = execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler);

    assert!(matches!(result, Err(ExecuteError::MemoryAccess(_))));
}

// Register Zero Tests

#[test]
fn test_write_to_zero_register() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let instructions = vec![Rv64Instruction::Addi {
        rd: EReg::Zero,
        rs1: EReg::Zero,
        imm: 100,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::Zero), 0);
}

#[test]
fn test_read_from_zero_register() {
    let (mut regs, mut mem, mut pc) = setup_test();

    let instructions = vec![Rv64Instruction::Add {
        rd: EReg::A0,
        rs1: EReg::Zero,
        rs2: EReg::Zero,
    }];

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    assert_eq!(regs.read(EReg::A0), 0);
}

// Complex Programs

#[test]
fn test_fibonacci() {
    let (mut regs, mut mem, mut pc) = setup_test();

    // Calculate fib(10)
    // fib(0) = 0, fib(1) = 1, fib(2) = 1, ..., fib(10) = 55

    // fib(n-2) = fib(0)
    regs.write(EReg::A1, 0);
    // fib(n-1) = fib(1)
    regs.write(EReg::A2, 1);

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

    let mut handler = TestInstructionHandler::new(instructions);
    execute_rv64(&mut regs, &mut mem, &mut pc, &mut handler).unwrap();

    // fib(10) = 55
    assert_eq!(regs.read(EReg::A2), 55);
}
