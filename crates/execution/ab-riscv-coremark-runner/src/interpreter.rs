use crate::instruction::CoremarkInstruction;
use ab_riscv_interpreter::prelude::*;
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;
use std::hint::cold_path;

/// Flat guest memory
#[derive(Debug, Copy, Clone)]
#[repr(align(16))]
pub(crate) struct GuestMemory<const BASE_ADDR: u64, const SIZE: usize> {
    data: [u8; SIZE],
}

impl<const BASE_ADDR: u64, const SIZE: usize> VirtualMemory for GuestMemory<BASE_ADDR, SIZE> {
    #[inline(always)]
    fn read<T>(&self, address: u64) -> Result<T, VirtualMemoryError>
    where
        T: BasicInt,
    {
        let offset = address.wrapping_sub(BASE_ADDR);

        if offset.saturating_add(size_of::<T>() as u64) > self.data.len() as u64 {
            cold_path();
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

    #[inline(always)]
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
        let offset = address.wrapping_sub(BASE_ADDR);

        if offset > self.data.len() as u64 {
            cold_path();
            return Err(VirtualMemoryError::OutOfBoundsRead { address });
        }

        self.data
            .get(offset as usize..)
            .and_then(|data| data.get(..len as usize))
            .ok_or(VirtualMemoryError::OutOfBoundsRead { address })
    }

    fn read_slice_up_to(&self, address: u64, len: u32) -> &[u8] {
        let offset = address.wrapping_sub(BASE_ADDR);

        if offset > self.data.len() as u64 {
            cold_path();
            return &[];
        }

        let remaining = self.data.get(offset as usize..).unwrap_or_default();
        remaining.get(..len as usize).unwrap_or(remaining)
    }

    #[inline(always)]
    fn write<T>(&mut self, address: u64, value: T) -> Result<(), VirtualMemoryError>
    where
        T: BasicInt,
    {
        let offset = address.wrapping_sub(BASE_ADDR);

        if offset.saturating_add(size_of::<T>() as u64) > self.data.len() as u64 {
            cold_path();
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
        let offset = address.wrapping_sub(BASE_ADDR);

        if offset > self.data.len() as u64 {
            cold_path();
            return Err(VirtualMemoryError::OutOfBoundsWrite { address });
        }

        let len = data.len();
        let Some(target_data) = self
            .data
            .get_mut(offset as usize..)
            .and_then(|data| data.get_mut(..len))
        else {
            cold_path();
            return Err(VirtualMemoryError::OutOfBoundsWrite { address });
        };

        target_data.copy_from_slice(data);

        Ok(())
    }
}

impl<const BASE_ADDR: u64, const SIZE: usize> Default for GuestMemory<BASE_ADDR, SIZE> {
    fn default() -> Self {
        Self { data: [0; SIZE] }
    }
}

/// Eager instruction handler eagerly decodes all instructions upfront
#[derive(Debug, Default, Clone)]
#[repr(C, align(16))]
pub(crate) struct EagerInstructionFetcher {
    instructions: Box<[CoremarkInstruction]>,
    instruction_offset: usize,
    return_trap_address: u64,
    base_addr: u64,
}

impl<Memory> ProgramCounter<u64, Memory> for EagerInstructionFetcher
where
    Memory: VirtualMemory,
{
    #[inline(always)]
    fn get_pc(&self) -> u64 {
        self.base_addr + self.instruction_offset as u64 * size_of::<u16>() as u64
    }

    #[inline]
    fn set_pc(
        &mut self,
        _memory: &Memory,
        pc: u64,
    ) -> Result<ControlFlow<()>, ProgramCounterError<u64>> {
        let address = pc;

        if address == self.return_trap_address {
            cold_path();
            return Ok(ControlFlow::Break(()));
        }

        if !address.is_multiple_of(size_of::<u16>() as u64) {
            cold_path();
            return Err(ProgramCounterError::UnalignedInstruction { address });
        }

        let Some(offset) = address.checked_sub(self.base_addr) else {
            cold_path();
            return Err(ProgramCounterError::MemoryAccess(
                VirtualMemoryError::OutOfBoundsRead { address },
            ));
        };
        let offset = offset as usize;
        let instruction_offset = offset / size_of::<u16>();

        if instruction_offset >= self.instructions.len() {
            cold_path();
            return Err(VirtualMemoryError::OutOfBoundsRead { address }.into());
        }

        self.instruction_offset = instruction_offset;

        Ok(ControlFlow::Continue(()))
    }
}

impl<Memory> InstructionFetcher<CoremarkInstruction, Memory> for EagerInstructionFetcher
where
    Memory: VirtualMemory,
{
    #[inline(always)]
    fn fetch_instruction(
        &mut self,
        _memory: &Memory,
    ) -> Result<FetchInstructionResult<CoremarkInstruction>, ExecutionError<u64>> {
        // SAFETY: Constructor guarantees that the last instruction is a jump, which means going
        // through `Self::set_pc()` method does the necessary bounds check and advancing forward by
        // one instruction can't result in out-of-bounds access.
        let instruction = *unsafe { self.instructions.get_unchecked(self.instruction_offset) };
        self.instruction_offset += usize::from(instruction.size()) / size_of::<u16>();

        Ok(FetchInstructionResult::Instruction(instruction))
    }
}

impl EagerInstructionFetcher {
    /// Create a new instance with the specified instructions and base address.
    ///
    /// `return_trap_address` is the address at which the interpreter will stop execution
    /// (gracefully).
    ///
    /// # Safety
    /// The program counter must be valid and aligned, the instructions processed must end with a
    /// jump instruction.
    #[inline(always)]
    pub(super) unsafe fn new(
        instructions: &[u8],
        return_trap_address: u64,
        base_addr: u64,
        pc: u64,
    ) -> Self {
        let mut decoded_instructions = Vec::with_capacity(instructions.len() / size_of::<u16>());

        let mut offset = 0;
        while let Some(instruction_bytes) = instructions.get(offset..offset + size_of::<u32>()) {
            let decoded_instruction = u32::from_le_bytes([
                instruction_bytes[0],
                instruction_bytes[1],
                instruction_bytes[2],
                instruction_bytes[3],
            ]);
            // Use `Unimp` as a fallback, though contract is expected to only contain legal
            // instructions
            let decoded_instruction =
                Instruction::try_decode(decoded_instruction).unwrap_or(CoremarkInstruction::Unimp);
            decoded_instructions.push(decoded_instruction);
            match decoded_instruction.size() {
                2 => {
                    offset += 2;
                }
                4 => {
                    // The second half of a 32-bit instruction is a valid offset and may or may not
                    // decode to a valid instruction on its own. Try to decode it but ignore
                    // decoding failures.

                    offset += 2;

                    // Could be both 16-bit and 32-bit instruction, need to handle end of the
                    // instruction stream
                    let instruction_word = if let Some(instruction_bytes) =
                        instructions.get(offset..offset + size_of::<u32>())
                    {
                        u32::from_le_bytes([
                            instruction_bytes[0],
                            instruction_bytes[1],
                            instruction_bytes[2],
                            instruction_bytes[3],
                        ])
                    } else {
                        u32::from_le_bytes([instruction_bytes[2], instruction_bytes[3], 0, 0])
                    };

                    decoded_instructions.push(
                        Instruction::try_decode(instruction_word)
                            .unwrap_or(CoremarkInstruction::Unimp),
                    );
                    offset += 2;
                }
                instruction_size => {
                    unreachable!("Invalid instruction size {instruction_size}, expected 2 or 4");
                }
            }
        }

        let remainder_bytes = instructions.get(offset..).unwrap_or(&[]);

        if remainder_bytes.len() == size_of::<u16>() {
            let instruction_word =
                u32::from_le_bytes([remainder_bytes[0], remainder_bytes[1], 0, 0]);
            decoded_instructions.push(
                Instruction::try_decode(instruction_word).unwrap_or(CoremarkInstruction::Unimp),
            );
        }

        Self {
            instructions: decoded_instructions.into_boxed_slice(),
            instruction_offset: (pc - base_addr) as usize / size_of::<u16>(),
            return_trap_address,
            base_addr,
        }
    }
}
