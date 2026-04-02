use ab_riscv_interpreter::{
    BasicInt, ExecutionError, FetchInstructionResult, InstructionFetcher, ProgramCounter,
    ProgramCounterError, SystemInstructionHandler, VirtualMemory, VirtualMemoryError,
};
use ab_riscv_primitives::instructions::Instruction;
use ab_riscv_primitives::registers::general_purpose::{RegType, Register, Registers};
use std::any::Any;
use std::ops::ControlFlow;

pub(crate) struct Act4Memory<const BASE_ADDR: u64, const SIZE: usize> {
    data: Box<[u8; SIZE]>,
    tohost_addr: u64,
    tohost_value: Option<u64>,
}

impl<const BASE_ADDR: u64, const SIZE: usize> VirtualMemory for Act4Memory<BASE_ADDR, SIZE> {
    fn read<T>(&self, address: u64) -> Result<T, VirtualMemoryError>
    where
        T: BasicInt,
    {
        let offset = address
            .checked_sub(BASE_ADDR)
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
            let offset = address.unchecked_sub(BASE_ADDR) as usize;
            self.data
                .as_ptr()
                .cast::<T>()
                .byte_add(offset)
                .read_unaligned()
        }
    }

    fn read_slice(&self, address: u64, len: u32) -> Result<&[u8], VirtualMemoryError> {
        let offset = address
            .checked_sub(BASE_ADDR)
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
        let Some(offset) = address.checked_sub(BASE_ADDR) else {
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
        if address == self.tohost_addr {
            if let Some(raw) = <dyn Any>::downcast_ref::<u64>(&value) {
                self.tohost_value = Some(*raw);
            } else if let Some(raw) = <dyn Any>::downcast_ref::<u32>(&value) {
                self.tohost_value = Some(u64::from(*raw));
            }
        }

        let offset = address
            .checked_sub(BASE_ADDR)
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
            .checked_sub(BASE_ADDR)
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

impl<const BASE_ADDR: u64, const SIZE: usize> Act4Memory<BASE_ADDR, SIZE> {
    pub(crate) fn new(tohost_addr: u64) -> Self {
        Self {
            // TODO: Should have been just `::new()`, but https://github.com/rust-lang/rust/issues/53827
            // SAFETY: Data structure filled with zeroes is a valid invariant
            data: unsafe { Box::new_zeroed().assume_init() },
            tohost_addr,
            tohost_value: None,
        }
    }

    pub(crate) fn tohost_value(&self) -> Option<u64> {
        self.tohost_value
    }
}

pub(crate) struct Act4InstructionFetcher<I>
where
    I: Instruction,
{
    pc: <I::Reg as Register>::Type,
}

impl<I, Memory> ProgramCounter<<I::Reg as Register>::Type, Memory> for Act4InstructionFetcher<I>
where
    I: Instruction,
    Memory: VirtualMemory,
{
    #[inline(always)]
    fn get_pc(&self) -> <I::Reg as Register>::Type {
        self.pc
    }

    fn set_pc(
        &mut self,
        memory: &Memory,
        pc: <I::Reg as Register>::Type,
    ) -> Result<ControlFlow<()>, ProgramCounterError<<I::Reg as Register>::Type>> {
        if !pc.as_u64().is_multiple_of(u64::from(I::alignment())) {
            return Err(ProgramCounterError::UnalignedInstruction { address: pc });
        }
        memory.read::<u32>(pc.as_u64())?;
        self.pc = pc;
        Ok(ControlFlow::Continue(()))
    }
}

impl<I, Memory> InstructionFetcher<I, Memory> for Act4InstructionFetcher<I>
where
    I: Instruction,
    Memory: VirtualMemory,
{
    fn fetch_instruction(
        &mut self,
        memory: &Memory,
    ) -> Result<FetchInstructionResult<I>, ExecutionError<<I::Reg as Register>::Type>> {
        let instruction = memory.read(self.pc.as_u64())?;
        let instruction = I::try_decode(instruction)
            .ok_or(ExecutionError::IllegalInstruction { address: self.pc })?;
        self.pc += instruction.size().into();

        Ok(FetchInstructionResult::Instruction(instruction))
    }
}

impl<I> Act4InstructionFetcher<I>
where
    I: Instruction,
{
    pub(crate) fn new(pc: <I::Reg as Register>::Type) -> Self {
        Self { pc }
    }
}

pub(crate) struct Act4SystemHandler;

impl<Reg, Memory, PC> SystemInstructionHandler<Reg, Memory, PC> for Act4SystemHandler
where
    Reg: Register,
    [(); Reg::N]:,
    Memory: VirtualMemory,
{
    fn handle_ecall(
        &mut self,
        _regs: &mut Registers<Reg>,
        _memory: &mut Memory,
        _pc: &mut PC,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type>> {
        Ok(ControlFlow::Continue(()))
    }
}
