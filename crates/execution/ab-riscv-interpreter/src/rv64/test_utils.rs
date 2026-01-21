extern crate alloc;

use crate::rv64::{Rv64InterpreterState, Rv64SystemInstructionHandler};
use crate::{
    Address, BasicInt, ExecutableInstruction, ExecutionError, FetchInstructionResult,
    InstructionFetcher, ProgramCounter, ProgramCounterError, VirtualMemory, VirtualMemoryError,
};
use ab_riscv_primitives::instruction::Instruction;
use ab_riscv_primitives::instruction::rv64::Rv64Instruction;
use ab_riscv_primitives::registers::{EReg, Registers};
use alloc::vec;
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::ops::ControlFlow;

pub(super) const TEST_BASE_ADDR: u64 = 0x1000;
const TRAP_ADDRESS: u64 = 0;

/// Simple test memory implementation
pub(super) struct TestMemory {
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
pub(super) struct TestInstructionFetcher<I> {
    instructions: Vec<I>,
    return_trap_address: u64,
    base_address: u64,
    pc: u64,
}

impl<I> ProgramCounter<u64, TestMemory, &'static str> for TestInstructionFetcher<I>
where
    I: Instruction<Reg = EReg<u64>>,
{
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

impl<I> InstructionFetcher<I, TestMemory, &'static str> for TestInstructionFetcher<I>
where
    I: Instruction<Reg = EReg<u64>>,
{
    #[inline]
    fn fetch_instruction(
        &mut self,
        _memory: &mut TestMemory,
    ) -> Result<FetchInstructionResult<I>, ExecutionError<Address<I>, I, &'static str>> {
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

pub(super) struct TestInstructionHandler;

impl<I> Rv64SystemInstructionHandler<EReg<u64>, TestMemory, TestInstructionFetcher<I>, &'static str>
    for TestInstructionHandler
where
    I: Instruction<Reg = EReg<u64>>,
{
    #[inline(always)]
    fn handle_ecall(
        &mut self,
        _regs: &mut Registers<EReg<u64>>,
        _memory: &mut TestMemory,
        program_counter: &mut TestInstructionFetcher<I>,
    ) -> Result<ControlFlow<()>, ExecutionError<u64, Rv64Instruction<EReg<u64>>, &'static str>>
    {
        let instruction = Rv64Instruction::Ecall;
        Err(ExecutionError::UnsupportedInstruction {
            address: program_counter.get_pc() - u64::from(instruction.size()),
            instruction,
        })
    }
}

impl<I> TestInstructionFetcher<I> {
    /// Create a new instance
    #[inline(always)]
    fn new(instructions: Vec<I>, return_trap_address: u64, base_address: u64, pc: u64) -> Self {
        Self {
            instructions,
            return_trap_address,
            base_address,
            pc,
        }
    }
}

pub(super) type TestInterpreterState<Instruction> = Rv64InterpreterState<
    EReg<u64>,
    TestMemory,
    TestInstructionFetcher<Instruction>,
    TestInstructionHandler,
    &'static str,
>;

pub(super) fn initialize_state<Instruction, Instructions>(
    instructions: Instructions,
) -> TestInterpreterState<Instruction>
where
    Instructions: Into<Vec<Instruction>>,
{
    Rv64InterpreterState {
        regs: Registers::default(),
        memory: TestMemory::new(8192, TEST_BASE_ADDR),
        instruction_fetcher: TestInstructionFetcher::new(
            instructions.into(),
            TRAP_ADDRESS,
            TEST_BASE_ADDR,
            TEST_BASE_ADDR,
        ),
        system_instruction_handler: TestInstructionHandler,
        _phantom: PhantomData,
    }
}

pub(super) fn execute<I>(
    state: &mut TestInterpreterState<I>,
) -> Result<(), ExecutionError<Address<I>, I, &'static str>>
where
    I: Instruction<Reg = EReg<u64>> + ExecutableInstruction<TestInterpreterState<I>, &'static str>,
{
    loop {
        let instruction = match state
            .instruction_fetcher
            .fetch_instruction(&mut state.memory)?
        {
            FetchInstructionResult::Instruction(instruction) => instruction,
            FetchInstructionResult::ControlFlow(ControlFlow::Continue(())) => {
                continue;
            }
            FetchInstructionResult::ControlFlow(ControlFlow::Break(())) => {
                break;
            }
        };

        match instruction.execute(state)? {
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
