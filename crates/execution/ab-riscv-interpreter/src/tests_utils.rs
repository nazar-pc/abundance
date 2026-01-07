extern crate alloc;

use crate::{
    BasicInt, ExecuteError, FetchInstructionResult, GenericInstructionHandler, VirtualMemory,
    VirtualMemoryError,
};
use ab_riscv_primitives::instruction::{GenericInstruction, Rv64MBZbcInstruction};
use ab_riscv_primitives::registers::{EReg, Registers};
use alloc::vec;
use alloc::vec::Vec;
use core::ops::ControlFlow;

pub(crate) const TEST_BASE_ADDR: u64 = 0x1000;
pub(crate) const TRAP_ADDRESS: u64 = 0;

/// Simple test memory implementation
pub(crate) struct TestMemory {
    data: Vec<u8>,
    base_addr: u64,
}

impl TestMemory {
    pub(crate) fn new(size: usize, base_addr: u64) -> Self {
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
pub(crate) struct TestInstructionHandler {
    instructions: Vec<Rv64MBZbcInstruction<EReg<u64>>>,
    index: usize,
}

impl TestInstructionHandler {
    pub(crate) fn new(instructions: Vec<Rv64MBZbcInstruction<EReg<u64>>>) -> Self {
        Self {
            instructions,
            index: 0,
        }
    }
}

impl GenericInstructionHandler<Rv64MBZbcInstruction<EReg<u64>>, EReg<u64>, TestMemory, &'static str>
    for TestInstructionHandler
{
    #[inline(always)]
    fn fetch_instruction(
        &mut self,
        _regs: &mut Registers<EReg<u64>>,
        _memory: &mut TestMemory,
        pc: &mut u64,
    ) -> Result<
        FetchInstructionResult<Rv64MBZbcInstruction<EReg<u64>>>,
        ExecuteError<Rv64MBZbcInstruction<EReg<u64>>, &'static str>,
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

    #[inline(always)]
    fn handle_ecall(
        &mut self,
        _regs: &mut Registers<EReg<u64>>,
        _memory: &mut TestMemory,
        pc: &mut u64,
        instruction: Rv64MBZbcInstruction<EReg<u64>>,
    ) -> Result<(), ExecuteError<Rv64MBZbcInstruction<EReg<u64>>, &'static str>> {
        Err(ExecuteError::UnsupportedInstruction {
            address: *pc - instruction.size() as u64,
            instruction,
        })
    }
}

pub(crate) fn setup_test() -> (Registers<EReg<u64>>, TestMemory, u64) {
    let regs = Registers::default();
    let memory = TestMemory::new(8192, TEST_BASE_ADDR);
    let pc = TEST_BASE_ADDR;
    (regs, memory, pc)
}
