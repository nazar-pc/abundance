extern crate alloc;

use crate::{
    Address, BasicInt, CustomErrorPlaceholder, ExecutableInstruction, ExecutionError,
    FetchInstructionResult, InstructionFetcher, InterpreterState, ProgramCounter,
    ProgramCounterError, SystemInstructionHandler, VirtualMemory, VirtualMemoryError,
};
use ab_riscv_primitives::instructions::Instruction;
use ab_riscv_primitives::instructions::rv32::Rv32Instruction;
use ab_riscv_primitives::registers::general_purpose::{Reg, Registers};
use alloc::vec;
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::ops::ControlFlow;

pub(crate) const TEST_BASE_ADDR: u32 = 0x1000;
const TRAP_ADDRESS: u32 = 0;

/// Simple test memory implementation
pub(crate) struct TestMemory {
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
            .ok_or(VirtualMemoryError::OutOfBoundsRead { address })?;

        if offset.saturating_add(size_of::<T>() as u64) > self.data.len() as u64 {
            return Err(VirtualMemoryError::OutOfBoundsRead { address });
        }

        // SAFETY: Only reading basic integers from initialized memory
        unsafe {
            Ok(self
                .data
                .as_ptr()
                .cast::<T>()
                .byte_add(offset as usize)
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

    fn read_slice(&self, address: u64, len: u32) -> Result<&[u8], VirtualMemoryError> {
        let offset = address
            .checked_sub(self.base_addr)
            .ok_or(VirtualMemoryError::OutOfBoundsRead { address })?;

        if offset > self.data.len() as u64 {
            return Err(VirtualMemoryError::OutOfBoundsRead { address });
        }

        self.data
            .get(offset as usize..)
            .and_then(|data| data.get(..len as usize))
            .ok_or(VirtualMemoryError::OutOfBoundsRead { address })
    }

    fn read_slice_up_to(&self, address: u64, len: u32) -> &[u8] {
        let Some(offset) = address.checked_sub(self.base_addr) else {
            return &[];
        };

        if offset > self.data.len() as u64 {
            return &[];
        }

        let remaining = self.data.get(offset as usize..).unwrap_or_default();
        remaining.get(..len as usize).unwrap_or(remaining)
    }

    fn write<T>(&mut self, address: u64, value: T) -> Result<(), VirtualMemoryError>
    where
        T: BasicInt,
    {
        let offset = address
            .checked_sub(self.base_addr)
            .ok_or(VirtualMemoryError::OutOfBoundsWrite { address })?;

        if offset.saturating_add(size_of::<T>() as u64) > self.data.len() as u64 {
            return Err(VirtualMemoryError::OutOfBoundsWrite { address });
        }

        // SAFETY: Only writing basic integers to initialized memory
        unsafe {
            self.data
                .as_mut_ptr()
                .cast::<T>()
                .byte_add(offset as usize)
                .write_unaligned(value);
        }

        Ok(())
    }

    fn write_slice(&mut self, address: u64, data: &[u8]) -> Result<(), VirtualMemoryError> {
        let offset = address
            .checked_sub(self.base_addr)
            .ok_or(VirtualMemoryError::OutOfBoundsWrite { address })?;

        if offset > self.data.len() as u64 {
            return Err(VirtualMemoryError::OutOfBoundsWrite { address });
        }

        let len = data.len();
        self.data
            .get_mut(offset as usize..)
            .and_then(|data| data.get_mut(..len))
            .ok_or(VirtualMemoryError::OutOfBoundsWrite { address })?
            .copy_from_slice(data);

        Ok(())
    }
}

/// Custom instruction handler for tests that returns instructions from a sequence
pub(crate) struct TestInstructionFetcher<I> {
    instructions: Vec<Option<I>>,
    return_trap_address: u32,
    base_address: u32,
    pc: u32,
}

impl<I> ProgramCounter<u32, TestMemory> for TestInstructionFetcher<I>
where
    I: Instruction<Reg = Reg<u32>>,
{
    #[inline(always)]
    fn get_pc(&self) -> u32 {
        self.pc
    }

    fn set_pc(
        &mut self,
        _memory: &TestMemory,
        pc: u32,
    ) -> Result<ControlFlow<()>, ProgramCounterError<u32>> {
        self.pc = pc;

        Ok(ControlFlow::Continue(()))
    }
}

impl<I> InstructionFetcher<I, TestMemory> for TestInstructionFetcher<I>
where
    I: Instruction<Reg = Reg<u32>>,
{
    #[inline]
    fn fetch_instruction(
        &mut self,
        _memory: &TestMemory,
    ) -> Result<FetchInstructionResult<I>, ExecutionError<Address<I>>> {
        if self.pc == self.return_trap_address {
            return Ok(FetchInstructionResult::ControlFlow(ControlFlow::Break(())));
        }

        let Some(&maybe_instruction) = self
            .instructions
            .get((self.pc - self.base_address) as usize / size_of::<u16>())
        else {
            return Ok(FetchInstructionResult::ControlFlow(ControlFlow::Break(())));
        };

        let Some(instruction) = maybe_instruction else {
            return Err(ExecutionError::IllegalInstruction {
                address: self.pc - self.base_address,
            });
        };
        self.pc += u32::from(instruction.size());

        Ok(FetchInstructionResult::Instruction(instruction))
    }
}

impl<I> TestInstructionFetcher<I> {
    /// Create a new instance
    #[inline(always)]
    fn new<Instructions>(
        instructions: Instructions,
        return_trap_address: u32,
        base_address: u32,
        pc: u32,
    ) -> Self
    where
        I: Instruction<Reg = Reg<u32>>,
        Instructions: IntoIterator<Item = I>,
    {
        Self {
            instructions: instructions
                .into_iter()
                .flat_map(|instruction| {
                    let maybe_second = match instruction.size() {
                        2 => None,
                        4 => Some(None),
                        instruction_size => {
                            panic!("Unexpected instruction size {instruction_size}");
                        }
                    };

                    [Some(instruction)].into_iter().chain(maybe_second)
                })
                .collect(),
            return_trap_address,
            base_address,
            pc,
        }
    }
}

pub(crate) struct TestInstructionHandler;

impl<I> SystemInstructionHandler<Reg<u32>, TestMemory, TestInstructionFetcher<I>>
    for TestInstructionHandler
where
    I: Instruction<Reg = Reg<u32>>,
{
    #[inline(always)]
    fn handle_ecall(
        &mut self,
        _regs: &mut Registers<Reg<u32>>,
        _memory: &mut TestMemory,
        program_counter: &mut TestInstructionFetcher<I>,
    ) -> Result<ControlFlow<()>, ExecutionError<u32>> {
        Err(ExecutionError::EcallUnsupported {
            address: program_counter.old_pc(Rv32Instruction::<Reg<u32>>::Ecall.size()),
        })
    }
}

pub(crate) type TestInterpreterState<Instruction> = InterpreterState<
    Reg<u32>,
    (),
    TestMemory,
    TestInstructionFetcher<Instruction>,
    TestInstructionHandler,
>;

pub(crate) fn initialize_state<I, Instructions>(
    instructions: Instructions,
) -> TestInterpreterState<I>
where
    I: Instruction<Reg = Reg<u32>>,
    Instructions: IntoIterator<Item = I>,
{
    InterpreterState {
        regs: Registers::default(),
        ext_state: (),
        memory: TestMemory::new(8192, u64::from(TEST_BASE_ADDR)),
        instruction_fetcher: TestInstructionFetcher::new(
            instructions,
            TRAP_ADDRESS,
            TEST_BASE_ADDR,
            TEST_BASE_ADDR,
        ),
        system_instruction_handler: TestInstructionHandler,
        custom_error: PhantomData,
    }
}

pub(crate) fn execute<I>(
    state: &mut TestInterpreterState<I>,
) -> Result<(), ExecutionError<Address<I>>>
where
    I: Instruction<Reg = Reg<u32>>
        + ExecutableInstruction<TestInterpreterState<I>, CustomErrorPlaceholder>,
{
    loop {
        let instruction = match state.instruction_fetcher.fetch_instruction(&state.memory)? {
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
