extern crate alloc;

use crate::{
    Address, BasicInt, CsrError, Csrs, ExecutableInstruction, ExecutionError,
    FetchInstructionResult, InstructionFetcher, InterpreterState, ProgramCounter,
    ProgramCounterError, SystemInstructionHandler, VirtualMemory, VirtualMemoryError,
};
use ab_riscv_primitives::instructions::Instruction;
use ab_riscv_primitives::instructions::rv64::Rv64Instruction;
use ab_riscv_primitives::privilege::PrivilegeLevel;
use ab_riscv_primitives::registers::general_purpose::{Reg, Registers};
use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::ops::ControlFlow;

pub(crate) const TEST_BASE_ADDR: u64 = 0x1000;
const TRAP_ADDRESS: u64 = 0;

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

    fn read_slice(&self, address: u64, len: u32) -> Result<&[u8], VirtualMemoryError> {
        let offset = address
            .checked_sub(self.base_addr)
            .ok_or(VirtualMemoryError::OutOfBoundsRead { address })? as usize;

        self.data
            .get(offset..)
            .and_then(|data| data.get(..len as usize))
            .ok_or(VirtualMemoryError::OutOfBoundsRead { address })
    }

    fn read_slice_up_to(&self, address: u64, len: u32) -> &[u8] {
        let Some(offset) = address.checked_sub(self.base_addr) else {
            return &[];
        };

        let remaining = self.data.get(offset as usize..).unwrap_or_default();
        remaining.get(..len as usize).unwrap_or(remaining)
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

    fn write_slice(&mut self, address: u64, data: &[u8]) -> Result<(), VirtualMemoryError> {
        let offset = address
            .checked_sub(self.base_addr)
            .ok_or(VirtualMemoryError::OutOfBoundsWrite { address })? as usize;

        let len = data.len();
        self.data
            .get_mut(offset..)
            .and_then(|data| data.get_mut(..len))
            .ok_or(VirtualMemoryError::OutOfBoundsWrite { address })?
            .copy_from_slice(data);

        Ok(())
    }
}

/// Custom instruction handler for tests that returns instructions from a sequence
pub(crate) struct TestInstructionFetcher<I> {
    instructions: Vec<I>,
    return_trap_address: u64,
    base_address: u64,
    pc: u64,
}

impl<I> ProgramCounter<u64, TestMemory, &'static str> for TestInstructionFetcher<I>
where
    I: Instruction<Reg = Reg<u64>>,
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
    I: Instruction<Reg = Reg<u64>>,
{
    #[inline]
    fn fetch_instruction(
        &mut self,
        _memory: &mut TestMemory,
    ) -> Result<FetchInstructionResult<I>, ExecutionError<Address<I>, &'static str>> {
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

pub(crate) struct TestInstructionHandler;

impl<I> SystemInstructionHandler<Reg<u64>, TestMemory, TestInstructionFetcher<I>, &'static str>
    for TestInstructionHandler
where
    I: Instruction<Reg = Reg<u64>>,
{
    #[inline(always)]
    fn handle_ecall(
        &mut self,
        _regs: &mut Registers<Reg<u64>>,
        _memory: &mut TestMemory,
        program_counter: &mut TestInstructionFetcher<I>,
    ) -> Result<ControlFlow<()>, ExecutionError<u64, &'static str>> {
        let instruction = Rv64Instruction::<Reg<u64>>::Ecall;
        Err(ExecutionError::EcallUnsupported {
            address: program_counter.get_pc() - u64::from(instruction.size()),
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

struct CsrExtState {
    privilege_level: PrivilegeLevel,
    csrs: BTreeMap<u16, u64>,
    prepare_csr_read: fn(csr_index: u16, raw_value: u64) -> Result<u64, CsrError<&'static str>>,
    prepare_csr_write: fn(csr_index: u16, write_value: u64) -> Result<u64, CsrError<&'static str>>,
}

impl Default for CsrExtState {
    #[inline(always)]
    fn default() -> Self {
        Self {
            privilege_level: PrivilegeLevel::Machine,
            csrs: BTreeMap::new(),
            prepare_csr_read: |csr_index, _| Err(CsrError::IllegalRead { csr_index }),
            prepare_csr_write: |csr_index, _| Err(CsrError::IllegalWrite { csr_index }),
        }
    }
}

pub(crate) struct ExtState {
    csr: CsrExtState,
}

impl Default for ExtState {
    #[inline(always)]
    fn default() -> Self {
        Self {
            csr: CsrExtState::default(),
        }
    }
}

impl Csrs<Reg<u64>, &'static str> for ExtState {
    fn privilege_level(&self) -> PrivilegeLevel {
        self.csr.privilege_level
    }

    fn read_csr(&self, csr_index: u16) -> Result<u64, CsrError<&'static str>> {
        self.csr
            .csrs
            .get(&csr_index)
            .copied()
            .ok_or(CsrError::IllegalRead { csr_index })
    }

    fn write_csr(&mut self, csr_index: u16, value: u64) -> Result<(), CsrError<&'static str>> {
        let stored_value = self
            .csr
            .csrs
            .get_mut(&csr_index)
            .ok_or(CsrError::IllegalWrite { csr_index })?;
        *stored_value = value;
        Ok(())
    }

    fn process_csr_read(
        &self,
        csr_index: u16,
        raw_value: u64,
    ) -> Result<u64, CsrError<&'static str>> {
        (self.csr.prepare_csr_read)(csr_index, raw_value)
    }

    fn process_csr_write(
        &mut self,
        csr_index: u16,
        write_value: u64,
    ) -> Result<u64, CsrError<&'static str>> {
        (self.csr.prepare_csr_write)(csr_index, write_value)
    }
}

impl ExtState {
    pub(crate) fn set_privilege_level(&mut self, privilege_level: PrivilegeLevel) {
        self.csr.privilege_level = privilege_level;
    }

    pub(crate) fn set_prepare_csr_read_write(
        &mut self,
        prepare_csr_read: fn(csr_index: u16, raw_value: u64) -> Result<u64, CsrError<&'static str>>,
        prepare_csr_write: fn(
            csr_index: u16,
            write_value: u64,
        ) -> Result<u64, CsrError<&'static str>>,
    ) {
        self.csr.prepare_csr_read = prepare_csr_read;
        self.csr.prepare_csr_write = prepare_csr_write;
    }

    pub(crate) fn init_csr(&mut self, csr_index: u16, value: u64) {
        self.csr.csrs.insert(csr_index, value);
    }
}

pub(crate) type TestInterpreterState<Instruction> = InterpreterState<
    Reg<u64>,
    ExtState,
    TestMemory,
    TestInstructionFetcher<Instruction>,
    TestInstructionHandler,
    &'static str,
>;

pub(crate) fn initialize_state<I, Instructions>(
    instructions: Instructions,
) -> TestInterpreterState<I>
where
    I: Instruction<Reg = Reg<u64>>,
    Instructions: Into<Vec<I>>,
{
    InterpreterState {
        regs: Registers::default(),
        ext_state: ExtState::default(),
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

pub(crate) fn execute<I>(
    state: &mut TestInterpreterState<I>,
) -> Result<(), ExecutionError<Address<I>, &'static str>>
where
    I: Instruction<Reg = Reg<u64>> + ExecutableInstruction<TestInterpreterState<I>, &'static str>,
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
