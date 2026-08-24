extern crate alloc;

use crate::basic::{BasicInterpreterState, BasicRegisters};
use crate::rv32::a::ReservationSet;
use crate::zawrs::WrsHandler;
use crate::{
    Address, BasicInt, ExecutableInstruction, ExecutionError, ExecutionResult,
    FetchInstructionResult, InstructionFetcher, PackedAddress, ProgramCounter, RegisterFile,
    Rs1Rs2OperandValues, Rs1Rs2Operands, SystemInstructionHandler, VirtualMemory,
    VirtualMemoryError,
};
use ab_riscv_primitives::prelude::*;
use alloc::vec;
use alloc::vec::Vec;
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

    #[inline(always)]
    unsafe fn try_set_pc_relative(&mut self, instruction_size: u8, offset: i32) -> bool {
        let old_pc = self.old_pc(instruction_size);
        self.pc = old_pc.wrapping_add_signed(offset);

        true
    }

    #[cold]
    #[inline(never)]
    unsafe fn failed_branch(
        &mut self,
        _memory: &TestMemory,
    ) -> Result<ControlFlow<()>, ExecutionError<u32>> {
        // Every target is accepted above, so there is never one to report on
        unreachable!("`try_set_pc_relative()` never refuses a target")
    }

    fn set_pc(
        &mut self,
        _memory: &TestMemory,
        pc: u32,
    ) -> Result<ControlFlow<()>, ExecutionError<u32>> {
        self.pc = pc;

        Ok(ControlFlow::Continue(()))
    }
}

impl<I> InstructionFetcher<I, TestMemory> for TestInstructionFetcher<I>
where
    I: Instruction<Reg = Reg<u32>>,
{
    #[inline]
    fn peek_instruction(&mut self, _memory: &TestMemory) -> FetchInstructionResult<I> {
        if self.pc == self.return_trap_address {
            return FetchInstructionResult::Break;
        }

        let Some(&maybe_instruction) = self
            .instructions
            .get((self.pc - self.base_address) as usize / size_of::<u16>())
        else {
            return FetchInstructionResult::Break;
        };

        let Some(instruction) = maybe_instruction else {
            return FetchInstructionResult::Err(ExecutionError::IllegalInstruction {
                address: PackedAddress::new(self.pc),
            });
        };
        FetchInstructionResult::Instruction(instruction)
    }

    #[inline]
    unsafe fn advance(&mut self, instruction_size: u8) {
        self.pc = self.pc.wrapping_add(u32::from(instruction_size));
    }

    #[inline]
    fn fetch_instruction(&mut self, memory: &TestMemory) -> FetchInstructionResult<I> {
        let result = self.peek_instruction(memory);

        if let FetchInstructionResult::Instruction(instruction) = result {
            // SAFETY: The instruction was just peeked successfully, and this is the only place that
            // moves past it
            unsafe {
                self.advance(instruction.size());
            }
        }

        result
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
                        4 => {
                            // Intentionally trigger illegal instruction on the second half-word
                            Some(None)
                        }
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

impl<Regs, I> SystemInstructionHandler<Reg<u32>, Regs, TestMemory, TestInstructionFetcher<I>>
    for TestInstructionHandler
where
    I: Instruction<Reg = Reg<u32>>,
{
    #[inline(always)]
    fn handle_ecall(
        &mut self,
        _regs: &mut Regs,
        _memory: &mut TestMemory,
        program_counter: &mut TestInstructionFetcher<I>,
    ) -> Result<ControlFlow<()>, ExecutionError<u32>> {
        Err(ExecutionError::EcallUnsupported {
            address: crate::PackedAddress::new(
                program_counter.old_pc(
                    Rv32Instruction::<Reg<u32>>::Ecall {
                        rs1: Reg::Zero,
                        rs2: Reg::Zero,
                    }
                    .size(),
                ),
            ),
        })
    }
}

impl WrsHandler for TestInstructionHandler {}

/// Extended state used by RV32 tests.
///
/// Currently only holds the reservation set used by `Zalrsc`.
#[derive(Default)]
pub(crate) struct ExtState {
    reservation: Option<u32>,
}

impl ReservationSet<Reg<u32>> for ExtState {
    fn reservation(&self) -> Option<u32> {
        self.reservation
    }

    fn set_reservation(&mut self, address: u32) {
        self.reservation = Some(address);
    }

    fn clear_reservation(&mut self) {
        self.reservation = None;
    }
}

pub(crate) type TestInterpreterState<Instruction> = BasicInterpreterState<
    BasicRegisters<Reg<u32>, false>,
    ExtState,
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
    BasicInterpreterState {
        regs: BasicRegisters::default(),
        ext_state: ExtState::default(),
        memory: TestMemory::new(8192, u64::from(TEST_BASE_ADDR)),
        instruction_fetcher: TestInstructionFetcher::new(
            instructions,
            TRAP_ADDRESS,
            TEST_BASE_ADDR,
            TEST_BASE_ADDR,
        ),
        system_instruction_handler: TestInstructionHandler,
    }
}

pub(crate) fn execute<I>(
    state: &mut TestInterpreterState<I>,
) -> Result<(), ExecutionError<Address<I>>>
where
    I: Instruction<Reg = Reg<u32>>
        + ExecutableInstruction<
            BasicRegisters<Reg<u32>, false>,
            ExtState,
            TestMemory,
            TestInstructionFetcher<I>,
            TestInstructionHandler,
        >,
{
    loop {
        let instruction = match state.instruction_fetcher.fetch_instruction(&state.memory) {
            FetchInstructionResult::Instruction(instruction) => instruction,
            FetchInstructionResult::Continue => {
                continue;
            }
            FetchInstructionResult::Break => {
                break;
            }
            FetchInstructionResult::Err(error) => {
                return Err(error);
            }
        };

        let Rs1Rs2Operands { rs1, rs2 } = instruction.get_rs1_rs2_operands();
        let rs1rs2_values = Rs1Rs2OperandValues {
            rs1_value: state.regs.read(rs1),
            rs2_value: state.regs.read(rs2),
        };

        match instruction.execute(
            rs1rs2_values,
            &mut state.regs,
            &mut state.ext_state,
            &mut state.memory,
            &mut state.instruction_fetcher,
            &mut state.system_instruction_handler,
        ) {
            ExecutionResult::Continue { rd, value } => {
                state.regs.write(rd, value);
            }
            ExecutionResult::ContinueNoWrite => {}
            ExecutionResult::Branch { offset } => {
                let control_flow = state.instruction_fetcher.set_pc_relative(
                    &state.memory,
                    instruction.size(),
                    offset,
                )?;
                if control_flow.is_break() {
                    break;
                }
            }
            ExecutionResult::Jump { target } => {
                if state
                    .instruction_fetcher
                    .set_pc(&state.memory, target)?
                    .is_break()
                {
                    break;
                }
            }
            ExecutionResult::Break => {
                break;
            }
            ExecutionResult::Err(error) => {
                return Err(error);
            }
        }
    }

    Ok(())
}
