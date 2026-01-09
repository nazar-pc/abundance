extern crate alloc;

use crate::rv64::{Rv64SystemInstructionHandler, execute_rv64};
use crate::{
    BasicInt, ExecutionError, FetchInstructionResult, InstructionFetcher, ProgramCounter,
    ProgramCounterError, VirtualMemory, VirtualMemoryError,
};
use ab_riscv_primitives::instruction::Instruction;
use ab_riscv_primitives::instruction::rv64::Rv64Instruction;
use ab_riscv_primitives::registers::{EReg, Registers};
use alloc::vec;
use alloc::vec::Vec;
use core::ops::ControlFlow;

const TEST_BASE_ADDR: u64 = 0x1000;
const TRAP_ADDRESS: u64 = 0;

/// Simple test memory implementation
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

    unsafe fn read_unchecked<T>(&self, address: u64) -> T
    where
        T: BasicInt,
    {
        // SAFETY: Guaranteed by function contract
        unsafe {
            let offset = address.unchecked_sub(self.base_addr) as usize;
            self.data
                .as_ptr()
                .cast::<T>()
                .byte_add(offset)
                .read_unaligned()
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
struct TestInstructionFetcher {
    instructions: Vec<Rv64Instruction<EReg<u64>>>,
    return_trap_address: u64,
    base_address: u64,
    pc: u64,
}

impl ProgramCounter<u64, TestMemory, &'static str> for TestInstructionFetcher {
    #[inline(always)]
    fn get_pc(&self) -> u64 {
        self.pc
    }

    fn set_pc(
        &mut self,
        _memory: &mut TestMemory,
        pc: u64,
    ) -> Result<ControlFlow<()>, ProgramCounterError<u64, &'static str>> {
        self.pc = pc;

        Ok(ControlFlow::Continue(()))
    }
}

impl InstructionFetcher<Rv64Instruction<EReg<u64>>, TestMemory, &'static str>
    for TestInstructionFetcher
{
    #[inline]
    fn fetch_instruction(
        &mut self,
        _memory: &mut TestMemory,
    ) -> Result<
        FetchInstructionResult<Rv64Instruction<EReg<u64>>>,
        ExecutionError<Rv64Instruction<EReg<u64>>, &'static str>,
    > {
        if self.pc == self.return_trap_address {
            return Ok(FetchInstructionResult::ControlFlow(ControlFlow::Break(())));
        }

        let Some(&instruction) = self
            .instructions
            .get((self.pc - self.base_address) as usize / size_of::<u32>())
        else {
            return Ok(FetchInstructionResult::ControlFlow(ControlFlow::Break(())));
        };
        self.pc += 4;

        Ok(FetchInstructionResult::Instruction(instruction))
    }
}

struct TestInstructionHandler;

impl Rv64SystemInstructionHandler<EReg<u64>, TestMemory, TestInstructionFetcher, &'static str>
    for TestInstructionHandler
{
    #[inline(always)]
    fn handle_ecall(
        &mut self,
        _regs: &mut Registers<EReg<u64>>,
        _memory: &mut TestMemory,
        program_counter: &mut TestInstructionFetcher,
    ) -> Result<ControlFlow<()>, ExecutionError<Rv64Instruction<EReg<u64>>, &'static str>> {
        let instruction = Rv64Instruction::Ecall;
        Err(ExecutionError::UnsupportedInstruction {
            address: program_counter.get_pc() - u64::from(instruction.size()),
            instruction,
        })
    }
}

impl TestInstructionFetcher {
    /// Create a new instance
    #[inline(always)]
    pub fn new(
        instructions: Vec<Rv64Instruction<EReg<u64>>>,
        return_trap_address: u64,
        base_address: u64,
        pc: u64,
    ) -> Self {
        Self {
            instructions,
            return_trap_address,
            base_address,
            pc,
        }
    }
}

fn setup_test(
    instructions: Vec<Rv64Instruction<EReg<u64>>>,
) -> (Registers<EReg<u64>>, TestMemory, TestInstructionFetcher) {
    let regs = Registers::default();
    let memory = TestMemory::new(8192, TEST_BASE_ADDR);
    (
        regs,
        memory,
        TestInstructionFetcher::new(instructions, TRAP_ADDRESS, TEST_BASE_ADDR, TEST_BASE_ADDR),
    )
}

fn execute(
    regs: &mut Registers<EReg<u64>>,
    memory: &mut TestMemory,
    instruction_fetcher: &mut TestInstructionFetcher,
) -> Result<(), ExecutionError<Rv64Instruction<EReg<u64>>, &'static str>> {
    loop {
        let instruction = match instruction_fetcher.fetch_instruction(memory)? {
            FetchInstructionResult::Instruction(instruction) => instruction,
            FetchInstructionResult::ControlFlow(ControlFlow::Continue(())) => {
                continue;
            }
            FetchInstructionResult::ControlFlow(ControlFlow::Break(())) => {
                break;
            }
        };

        match execute_rv64(
            regs,
            memory,
            instruction_fetcher,
            &mut TestInstructionHandler,
            instruction,
        )? {
            ControlFlow::Continue(()) => {
                continue;
            }
            ControlFlow::Break(()) => {
                break;
            }
        }
    }

    Ok(())
}

// Arithmetic Instructions

#[test]
fn test_add() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Add {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    regs.write(EReg::A0, 10);
    regs.write(EReg::A1, 20);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A2), 30);
}

#[test]
fn test_add_overflow() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Add {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    regs.write(EReg::A0, u64::MAX);
    regs.write(EReg::A1, 1);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    // Wrapping behavior
    assert_eq!(regs.read(EReg::A2), 0);
}

#[test]
fn test_sub() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Sub {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    regs.write(EReg::A0, 50);
    regs.write(EReg::A1, 20);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A2), 30);
}

#[test]
fn test_sub_underflow() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Sub {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    regs.write(EReg::A0, 0);
    regs.write(EReg::A1, 1);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A2), u64::MAX);
}

// Logical Instructions

#[test]
fn test_and() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::And {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    regs.write(EReg::A0, 0b1111_0000);
    regs.write(EReg::A1, 0b1010_1010);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A2), 0b1010_0000);
}

#[test]
fn test_or() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Or {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    regs.write(EReg::A0, 0b1111_0000);
    regs.write(EReg::A1, 0b0000_1111);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A2), 0b1111_1111);
}

#[test]
fn test_xor() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Xor {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    regs.write(EReg::A0, 0b1111_0000);
    regs.write(EReg::A1, 0b1010_1010);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A2), 0b0101_1010);
}

// Shift Instructions

#[test]
fn test_sll() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Sll {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    regs.write(EReg::A0, 1);
    regs.write(EReg::A1, 4);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A2), 16);
}

#[test]
fn test_sll_mask() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Sll {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    regs.write(EReg::A0, 1);
    // High bits should be masked
    regs.write(EReg::A1, 0x100);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    // Only lower 6 bits used
    assert_eq!(regs.read(EReg::A2), 1);
}

#[test]
fn test_srl() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Srl {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    regs.write(EReg::A0, 16);
    regs.write(EReg::A1, 2);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A2), 4);
}

#[test]
fn test_sra() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Sra {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    regs.write(EReg::A0, (-16i64).cast_unsigned());
    regs.write(EReg::A1, 2);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A2), (-4i64).cast_unsigned());
}

// Comparison Instructions

#[test]
fn test_slt_less() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Slt {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    regs.write(EReg::A0, (-5i64).cast_unsigned());
    regs.write(EReg::A1, 10);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A2), 1);
}

#[test]
fn test_slt_greater() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Slt {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    regs.write(EReg::A0, 10);
    regs.write(EReg::A1, (-5i64).cast_unsigned());

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A2), 0);
}

#[test]
fn test_sltu() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Sltu {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    regs.write(EReg::A0, 5);
    regs.write(EReg::A1, (-1i64).cast_unsigned());

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    // 5 < MAX unsigned
    assert_eq!(regs.read(EReg::A2), 1);
}

// RV64 32-bit Instructions

#[test]
fn test_addw() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Addw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    regs.write(EReg::A0, 0x0000_0001_8000_0000);
    regs.write(EReg::A1, 0x0000_0000_8000_0000);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    // Sign-extended result
    assert_eq!(regs.read(EReg::A2), 0);
}

#[test]
fn test_subw() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Subw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    regs.write(EReg::A0, 0x0000_0001_0000_0000);
    regs.write(EReg::A1, 1);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A2), (-1i64).cast_unsigned());
}

#[test]
fn test_sllw() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Sllw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    regs.write(EReg::A0, 1);
    regs.write(EReg::A1, 31);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A2), 0xFFFF_FFFF_8000_0000);
}

#[test]
fn test_srlw() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Srlw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    regs.write(EReg::A0, 0xFFFF_FFFF_8000_0000);
    regs.write(EReg::A1, 1);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A2), 0x0000_0000_4000_0000);
}

#[test]
fn test_sraw() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Sraw {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    regs.write(EReg::A0, 0xFFFF_FFFF_8000_0000);
    regs.write(EReg::A1, 1);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A2), 0xFFFF_FFFF_C000_0000);
}

// Immediate Instructions

#[test]
fn test_addi() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Addi {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 5,
    }]);

    regs.write(EReg::A0, 10);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A1), 15);
}

#[test]
fn test_addi_negative() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Addi {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: -5,
    }]);

    regs.write(EReg::A0, 10);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A1), 5);
}

#[test]
fn test_slti() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Slti {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 10,
    }]);

    regs.write(EReg::A0, (-5i64).cast_unsigned());

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A1), 1);
}

#[test]
fn test_sltiu() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Sltiu {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: -1,
    }]);

    regs.write(EReg::A0, 5);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A1), 1);
}

#[test]
fn test_xori() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Xori {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 0xAA,
    }]);

    regs.write(EReg::A0, 0xFF);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A1), 0x55);
}

#[test]
fn test_ori() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Ori {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 0x0F,
    }]);

    regs.write(EReg::A0, 0xF0);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A1), 0xFF);
}

#[test]
fn test_andi() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Andi {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 0x0F,
    }]);

    regs.write(EReg::A0, 0xFF);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A1), 0x0F);
}

#[test]
fn test_slli() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Slli {
        rd: EReg::A1,
        rs1: EReg::A0,
        shamt: 4,
    }]);

    regs.write(EReg::A0, 1);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A1), 16);
}

#[test]
fn test_srli() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Srli {
        rd: EReg::A1,
        rs1: EReg::A0,
        shamt: 2,
    }]);

    regs.write(EReg::A0, 16);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A1), 4);
}

#[test]
fn test_srai() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Srai {
        rd: EReg::A1,
        rs1: EReg::A0,
        shamt: 2,
    }]);

    regs.write(EReg::A0, (-16i64).cast_unsigned());

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A1), (-4i64).cast_unsigned());
}

#[test]
fn test_addiw() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Addiw {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 5,
    }]);

    // -5 sign-extended
    regs.write(EReg::A0, 0xFFFF_FFFF_FFFF_FFFB);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    // -5 + 5 = 0
    assert_eq!(regs.read(EReg::A1), 0);
}

#[test]
fn test_slliw() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Slliw {
        rd: EReg::A1,
        rs1: EReg::A0,
        shamt: 31,
    }]);

    regs.write(EReg::A0, 1);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A1), 0xFFFF_FFFF_8000_0000);
}

#[test]
fn test_srliw() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Srliw {
        rd: EReg::A1,
        rs1: EReg::A0,
        shamt: 1,
    }]);

    regs.write(EReg::A0, 0xFFFF_FFFF_8000_0000);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A1), 0x0000_0000_4000_0000);
}

#[test]
fn test_sraiw() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Sraiw {
        rd: EReg::A1,
        rs1: EReg::A0,
        shamt: 1,
    }]);

    regs.write(EReg::A0, 0xFFFF_FFFF_8000_0000);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A1), 0xFFFF_FFFF_C000_0000);
}

// Load Instructions

#[test]
fn test_lb() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Lb {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 10,
    }]);

    let data_addr = TEST_BASE_ADDR + 0x100;
    mem.write::<i8>(data_addr + 10, -5).unwrap();
    regs.write(EReg::A0, data_addr);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A1), (-5i64).cast_unsigned());
}

#[test]
fn test_lh() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Lh {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 0,
    }]);

    let data_addr = TEST_BASE_ADDR + 0x100;
    mem.write::<i16>(data_addr, -300).unwrap();
    regs.write(EReg::A0, data_addr);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A1), (-300i64).cast_unsigned());
}

#[test]
fn test_lw() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Lw {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 0,
    }]);

    let data_addr = TEST_BASE_ADDR + 0x100;
    mem.write::<i32>(data_addr, -100000).unwrap();
    regs.write(EReg::A0, data_addr);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A1), (-100000i64).cast_unsigned());
}

#[test]
fn test_ld() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Ld {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 0,
    }]);

    let data_addr = TEST_BASE_ADDR + 0x100;
    mem.write::<u64>(data_addr, 0x1234_5678_9ABC_DEF0).unwrap();
    regs.write(EReg::A0, data_addr);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A1), 0x1234_5678_9ABC_DEF0);
}

#[test]
fn test_lbu() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Lbu {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 0,
    }]);

    let data_addr = TEST_BASE_ADDR + 0x100;
    mem.write::<u8>(data_addr, 0xFF).unwrap();
    regs.write(EReg::A0, data_addr);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A1), 0xFF);
}

#[test]
fn test_lhu() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Lhu {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 0,
    }]);

    let data_addr = TEST_BASE_ADDR + 0x100;
    mem.write::<u16>(data_addr, 0xFFFF).unwrap();
    regs.write(EReg::A0, data_addr);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A1), 0xFFFF);
}

#[test]
fn test_lwu() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Lwu {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 0,
    }]);

    let data_addr = TEST_BASE_ADDR + 0x100;
    mem.write::<u32>(data_addr, 0xFFFF_FFFF).unwrap();
    regs.write(EReg::A0, data_addr);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A1), 0xFFFF_FFFF);
}

// Store Instructions

#[test]
fn test_sb() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Sb {
        rs1: EReg::A0,
        rs2: EReg::A1,
        imm: 0,
    }]);

    let data_addr = TEST_BASE_ADDR + 0x100;
    regs.write(EReg::A0, data_addr);
    regs.write(EReg::A1, 0x12);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(mem.read::<u8>(data_addr).unwrap(), 0x12);
}

#[test]
fn test_sh() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Sh {
        rs1: EReg::A0,
        rs2: EReg::A1,
        imm: 0,
    }]);

    let data_addr = TEST_BASE_ADDR + 0x100;
    regs.write(EReg::A0, data_addr);
    regs.write(EReg::A1, 0x1234);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(mem.read::<u16>(data_addr).unwrap(), 0x1234);
}

#[test]
fn test_sw() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Sw {
        rs1: EReg::A0,
        rs2: EReg::A1,
        imm: 0,
    }]);

    let data_addr = TEST_BASE_ADDR + 0x100;
    regs.write(EReg::A0, data_addr);
    regs.write(EReg::A1, 0x1234_5678);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(mem.read::<u32>(data_addr).unwrap(), 0x1234_5678);
}

#[test]
fn test_sd() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Sd {
        rs1: EReg::A0,
        rs2: EReg::A1,
        imm: 0,
    }]);

    let data_addr = TEST_BASE_ADDR + 0x100;
    regs.write(EReg::A0, data_addr);
    regs.write(EReg::A1, 0x1234_5678_9ABC_DEF0);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(mem.read::<u64>(data_addr).unwrap(), 0x1234_5678_9ABC_DEF0);
}

// Branch Instructions

#[test]
fn test_beq_taken() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Beq {
        rs1: EReg::A0,
        rs2: EReg::A1,
        // Branch offset from PC before increment
        imm: 8,
    }]);

    regs.write(EReg::A0, 10);
    regs.write(EReg::A1, 10);

    let initial_pc = instruction_fetcher.get_pc();
    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    // Branch calculates: old_pc (stored before fetch incremented it) + offset
    // The implementation stores old_pc before PC is incremented
    // So: initial_pc + 8 = 0x1000 + 8 = 0x1008
    assert_eq!(instruction_fetcher.get_pc(), initial_pc.wrapping_add(8));
}

#[test]
fn test_beq_not_taken() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![
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

    regs.write(EReg::A0, 10);
    regs.write(EReg::A1, 20);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    // Verify the branch was NOT taken - next instruction executed
    assert_eq!(regs.read(EReg::A2), 99);
}

#[test]
fn test_bne_taken() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Bne {
        rs1: EReg::A0,
        rs2: EReg::A1,
        imm: 8,
    }]);

    regs.write(EReg::A0, 10);
    regs.write(EReg::A1, 20);

    let initial_pc = instruction_fetcher.get_pc();
    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(instruction_fetcher.get_pc(), initial_pc.wrapping_add(8));
}

#[test]
fn test_blt_taken() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Blt {
        rs1: EReg::A0,
        rs2: EReg::A1,
        imm: 12,
    }]);

    regs.write(EReg::A0, (-10i64).cast_unsigned());
    regs.write(EReg::A1, 10);

    let initial_pc = instruction_fetcher.get_pc();
    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(instruction_fetcher.get_pc(), initial_pc.wrapping_add(12));
}

#[test]
fn test_bge_taken() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Bge {
        rs1: EReg::A0,
        rs2: EReg::A1,
        imm: 16,
    }]);

    regs.write(EReg::A0, 10);
    regs.write(EReg::A1, 10);

    let initial_pc = instruction_fetcher.get_pc();
    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(instruction_fetcher.get_pc(), initial_pc.wrapping_add(16));
}

#[test]
fn test_bltu_taken() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Bltu {
        rs1: EReg::A0,
        rs2: EReg::A1,
        imm: 20,
    }]);

    regs.write(EReg::A0, 10);
    regs.write(EReg::A1, 20);

    let initial_pc = instruction_fetcher.get_pc();
    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(instruction_fetcher.get_pc(), initial_pc.wrapping_add(20));
}

#[test]
fn test_bgeu_taken() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Bgeu {
        rs1: EReg::A0,
        rs2: EReg::A1,
        imm: 24,
    }]);

    regs.write(EReg::A0, 20);
    regs.write(EReg::A1, 10);

    let initial_pc = instruction_fetcher.get_pc();
    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(instruction_fetcher.get_pc(), initial_pc.wrapping_add(24));
}

// Jump Instructions

#[test]
fn test_jal() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![
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

    let initial_pc = instruction_fetcher.get_pc();
    regs.write(EReg::A2, 0);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    // Return address
    assert_eq!(regs.read(EReg::Ra), initial_pc + 4);
    assert_eq!(regs.read(EReg::A2), 42);
}

#[test]
fn test_jalr() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![
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

    let initial_pc = instruction_fetcher.get_pc();
    let target_addr = TEST_BASE_ADDR + 8;
    regs.write(EReg::A0, target_addr);
    regs.write(EReg::A2, 0);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    // Return address
    assert_eq!(regs.read(EReg::Ra), initial_pc + 4);
    assert_eq!(regs.read(EReg::A2), 42);
}

#[test]
fn test_jalr_clear_lsb() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![
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

    let initial_pc = instruction_fetcher.get_pc();
    // Odd address
    regs.write(EReg::A0, TEST_BASE_ADDR + 9);
    regs.write(EReg::A2, 0);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::Ra), initial_pc + 4);
    // LSB cleared: 9 -> 8
    assert_eq!(regs.read(EReg::A2), 42);
}

// Upper Immediate Instructions

#[test]
fn test_lui() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Lui {
        rd: EReg::A0,
        // Already shifted - bits [31:12]
        imm: 0x12345000,
    }]);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A0), 0x12345000u64);
}

#[test]
fn test_lui_negative() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Lui {
        rd: EReg::A0,
        // 0xFFFFF000 as upper 20 bits (already shifted)
        imm: 0xfffff000u32.cast_signed(),
    }]);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    // Should be sign-extended: 0xfffffffffffff000
    assert_eq!(regs.read(EReg::A0), 0xfffffffffffff000u64);
}

#[test]
fn test_auipc() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Auipc {
        rd: EReg::A0,
        // Already shifted - bits [31:12]
        imm: 0x12345000,
    }]);

    let initial_pc = instruction_fetcher.get_pc();

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A0), initial_pc.wrapping_add(0x12345000u64));
}

#[test]
fn test_auipc_negative() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Auipc {
        rd: EReg::A0,
        // Negative immediate (all upper bits set)
        imm: 0xfffff000u32.cast_signed(),
    }]);

    let initial_pc = instruction_fetcher.get_pc();

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    // Should wrap around: PC + sign_extend(0xfffff000)
    assert_eq!(
        regs.read(EReg::A0),
        initial_pc.wrapping_add(0xfffffffffffff000u64)
    );
}

// Special Instructions

#[test]
fn test_fence() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Fence {
        pred: 0xF,
        succ: 0xF,
        fm: 0,
    }]);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    // Should execute without error (NOP in single-threaded)
}

#[test]
fn test_ebreak() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Ebreak]);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    // Should execute without error (NOP by default)
}

#[test]
fn test_ecall_unsupported() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Ecall]);

    let result = execute(&mut regs, &mut mem, &mut instruction_fetcher);

    assert!(matches!(
        result,
        Err(ExecutionError::UnsupportedInstruction { .. })
    ));
}

#[test]
fn test_unimp() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Unimp]);

    let result = execute(&mut regs, &mut mem, &mut instruction_fetcher);

    assert!(matches!(
        result,
        Err(ExecutionError::UnimpInstruction { .. })
    ));
}

// Error Conditions

#[test]
fn test_out_of_bounds_read() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Ld {
        rd: EReg::A1,
        rs1: EReg::A0,
        imm: 0,
    }]);

    // Invalid address
    regs.write(EReg::A0, 0x0);

    let result = execute(&mut regs, &mut mem, &mut instruction_fetcher);

    assert!(matches!(result, Err(ExecutionError::MemoryAccess(_))));
}

#[test]
fn test_out_of_bounds_write() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Sd {
        rs1: EReg::A0,
        rs2: EReg::A1,
        imm: 0,
    }]);

    // Invalid address
    regs.write(EReg::A0, 0x0);
    regs.write(EReg::A1, 42);

    let result = execute(&mut regs, &mut mem, &mut instruction_fetcher);

    assert!(matches!(result, Err(ExecutionError::MemoryAccess(_))));
}

// Register Zero Tests

#[test]
fn test_write_to_zero_register() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Addi {
        rd: EReg::Zero,
        rs1: EReg::Zero,
        imm: 100,
    }]);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::Zero), 0);
}

#[test]
fn test_read_from_zero_register() {
    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(vec![Rv64Instruction::Add {
        rd: EReg::A0,
        rs1: EReg::Zero,
        rs2: EReg::Zero,
    }]);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    assert_eq!(regs.read(EReg::A0), 0);
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

    let (mut regs, mut mem, mut instruction_fetcher) = setup_test(instructions);

    // Calculate fib(10)
    // fib(0) = 0, fib(1) = 1, fib(2) = 1, ..., fib(10) = 55

    // fib(n-2) = fib(0)
    regs.write(EReg::A1, 0);
    // fib(n-1) = fib(1)
    regs.write(EReg::A2, 1);

    execute(&mut regs, &mut mem, &mut instruction_fetcher).unwrap();

    // fib(10) = 55
    assert_eq!(regs.read(EReg::A2), 55);
}
