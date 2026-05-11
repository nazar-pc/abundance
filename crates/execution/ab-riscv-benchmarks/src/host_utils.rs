extern crate alloc;

use ab_blake3::{CHUNK_LEN, OUT_LEN};
use ab_contract_file::instruction::{ContractInstruction, ContractRegister};
use ab_core_primitives::ed25519::{Ed25519PublicKey, Ed25519Signature};
use ab_io_type::IoType;
use ab_io_type::bool::Bool;
use ab_riscv_interpreter::basic::{BasicInterpreterState, IgnoreEcallSystemInstructionHandler};
use ab_riscv_interpreter::prelude::*;
use ab_riscv_primitives::prelude::*;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::hint::cold_path;
use core::mem::offset_of;
use core::ops::ControlFlow;

/// Contract file bytes
pub const RISCV_CONTRACT_BYTES: &[u8] = cfg_select! {
    target_env = "abundance" => {
        &[]
    }
    _ => {
        include_bytes!(env!("CONTRACT_PATH"))
    }
};

// TODO: Generate similar helper data structures in the `#[contract]` macro itself, maybe introduce
//  `SimpleInternalArgs` data trait for this or something
/// Helper data structure for [`Benchmarks::blake3_hash_chunk()`] method
///
/// [`Benchmarks::blake3_hash_chunk()`]: crate::Benchmarks::blake3_hash_chunk
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct Blake3HashChunkInternalArgs {
    chunk_ptr: u64,
    chunk_size: u32,
    chunk_capacity: u32,
    result_ptr: u64,
    chunk: [u8; CHUNK_LEN],
    result: [u8; OUT_LEN],
}

impl Blake3HashChunkInternalArgs {
    /// Create a new instance
    pub fn new(internal_args_addr: u64, chunk: [u8; CHUNK_LEN]) -> Self {
        Self {
            chunk_ptr: internal_args_addr + offset_of!(Self, chunk) as u64,
            chunk_size: CHUNK_LEN as u32,
            chunk_capacity: CHUNK_LEN as u32,
            result_ptr: internal_args_addr + offset_of!(Self, result) as u64,
            chunk,
            result: [0; _],
        }
    }

    /// Extract result
    pub fn result(&self) -> [u8; OUT_LEN] {
        self.result
    }
}

// TODO: Generate similar helper data structures in the `#[contract]` macro itself, maybe introduce
//  `SimpleInternalArgs` data trait for this or something
/// Helper data structure for [`Benchmarks::ed25519_verify()`] method
///
/// [`Benchmarks::ed25519_verify()`]: crate::Benchmarks::ed25519_verify
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct Ed25519VerifyInternalArgs {
    pub public_key_ptr: u64,
    pub public_key_size: u32,
    pub public_key_capacity: u32,
    pub signature_ptr: u64,
    pub signature_size: u32,
    pub signature_capacity: u32,
    pub message_ptr: u64,
    pub message_size: u32,
    pub message_capacity: u32,
    pub result_ptr: u64,
    pub public_key: Ed25519PublicKey,
    pub signature: Ed25519Signature,
    pub message: [u8; OUT_LEN],
    pub result: Bool,
}

impl Ed25519VerifyInternalArgs {
    /// Create a new instance
    pub fn new(
        internal_args_addr: u64,
        public_key: Ed25519PublicKey,
        signature: Ed25519Signature,
        message: [u8; OUT_LEN],
    ) -> Self {
        Self {
            public_key_ptr: internal_args_addr + offset_of!(Self, public_key) as u64,
            public_key_size: Ed25519PublicKey::SIZE as u32,
            public_key_capacity: Ed25519PublicKey::SIZE as u32,
            signature_ptr: internal_args_addr + offset_of!(Self, signature) as u64,
            signature_size: Ed25519Signature::SIZE as u32,
            signature_capacity: Ed25519Signature::SIZE as u32,
            message_ptr: internal_args_addr + offset_of!(Self, message) as u64,
            message_size: OUT_LEN as u32,
            message_capacity: OUT_LEN as u32,
            result_ptr: internal_args_addr + offset_of!(Self, result) as u64,
            public_key,
            signature,
            message,
            result: Bool::new(false),
        }
    }

    /// Extract result
    pub fn result(&self) -> Bool {
        self.result
    }
}

/// Simple test memory implementation
#[derive(Debug, Copy, Clone)]
#[repr(align(16))]
pub struct TestMemory<const BASE_ADDR: u64, const SIZE: usize> {
    data: [u8; SIZE],
}

impl<const BASE_ADDR: u64, const SIZE: usize> VirtualMemory for TestMemory<BASE_ADDR, SIZE> {
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

impl<const BASE_ADDR: u64, const SIZE: usize> Default for TestMemory<BASE_ADDR, SIZE> {
    fn default() -> Self {
        Self { data: [0; SIZE] }
    }
}

impl<const BASE_ADDR: u64, const SIZE: usize> TestMemory<BASE_ADDR, SIZE> {
    /// Get a mutable slice of memory
    pub fn get_mut_bytes(
        &mut self,
        address: u64,
        size: usize,
    ) -> Result<&mut [u8], VirtualMemoryError> {
        let Some(offset) = address.checked_sub(BASE_ADDR) else {
            cold_path();
            return Err(VirtualMemoryError::OutOfBoundsRead { address });
        };
        let offset = offset as usize;

        if offset + size > self.data.len() {
            cold_path();
            return Err(VirtualMemoryError::OutOfBoundsRead { address });
        }

        Ok(&mut self.data[offset..][..size])
    }
}

/// Lazy instruction fetcher implementation
#[derive(Debug, Copy, Clone)]
pub struct LazyInstructionFetcher {
    return_trap_address: u64,
    pc: u64,
}

impl<Memory> ProgramCounter<u64, Memory> for LazyInstructionFetcher
where
    Memory: VirtualMemory,
{
    #[inline(always)]
    fn get_pc(&self) -> u64 {
        self.pc
    }

    #[inline]
    fn set_pc(
        &mut self,
        memory: &Memory,
        pc: u64,
    ) -> Result<ControlFlow<()>, ProgramCounterError<u64>> {
        if pc == self.return_trap_address {
            cold_path();
            return Ok(ControlFlow::Break(()));
        }

        if !pc.is_multiple_of(u64::from(
            ContractInstruction::<ContractRegister>::alignment(),
        )) {
            cold_path();
            return Err(ProgramCounterError::UnalignedInstruction { address: pc });
        }

        // Note: This will not allow reading a 16-bit instruction at the very end of memory range,
        // but that is going to be the case here anyway since code is followed by read-write memory
        // anyway
        if let Err(error) = memory.read::<u32>(pc) {
            cold_path();
            return Err(error.into());
        }

        self.pc = pc;

        Ok(ControlFlow::Continue(()))
    }
}

impl<Memory> InstructionFetcher<ContractInstruction, Memory> for LazyInstructionFetcher
where
    Memory: VirtualMemory,
{
    #[inline]
    fn fetch_instruction(
        &mut self,
        memory: &Memory,
    ) -> Result<FetchInstructionResult<ContractInstruction>, ExecutionError<u64>> {
        // SAFETY: Constructor guarantees that the last instruction is a jump, which means going
        // through `Self::set_pc()` method does the necessary bounds check and advancing forward by
        // one instruction can't result in out-of-bounds access.
        let instruction = unsafe { memory.read_unchecked(self.pc) };
        // SAFETY: All instructions are valid, according to the constructor contract
        let instruction =
            unsafe { ContractInstruction::try_decode(instruction).unwrap_unchecked() };

        self.pc += u64::from(instruction.size());

        Ok(FetchInstructionResult::Instruction(instruction))
    }
}

impl LazyInstructionFetcher {
    /// Create a new instance.
    ///
    /// `return_trap_address` is the address at which the interpreter will stop execution
    /// (gracefully).
    ///
    /// # Safety
    /// The program counter must be valid and aligned, the instructions processed must be valid and
    /// end with a jump instruction.
    #[inline(always)]
    pub unsafe fn new(return_trap_address: u64, pc: u64) -> Self {
        Self {
            return_trap_address,
            pc,
        }
    }
}

/// Eager instruction handler eagerly decodes all instructions upfront
#[derive(Debug, Clone)]
#[repr(C, align(16))]
pub struct EagerTestInstructionFetcher {
    instructions: Box<[ContractInstruction]>,
    instruction_offset: usize,
    return_trap_address: u64,
    base_addr: u64,
}

impl<Memory> ProgramCounter<u64, Memory> for EagerTestInstructionFetcher
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

impl<Memory> InstructionFetcher<ContractInstruction, Memory> for EagerTestInstructionFetcher
where
    Memory: VirtualMemory,
{
    #[inline(always)]
    fn fetch_instruction(
        &mut self,
        _memory: &Memory,
    ) -> Result<FetchInstructionResult<ContractInstruction>, ExecutionError<u64>> {
        // SAFETY: Constructor guarantees that the last instruction is a jump, which means going
        // through `Self::set_pc()` method does the necessary bounds check and advancing forward by
        // one instruction can't result in out-of-bounds access.
        let instruction = *unsafe { self.instructions.get_unchecked(self.instruction_offset) };
        self.instruction_offset += usize::from(instruction.size()) / size_of::<u16>();

        Ok(FetchInstructionResult::Instruction(instruction))
    }
}

impl EagerTestInstructionFetcher {
    /// Create a new instance with the specified instructions and base address.
    ///
    /// Instructions are decoded during instantiation of the instruction fetcher, and the base
    /// address corresponds to the first instruction.
    ///
    /// `return_trap_address` is the address at which the interpreter will stop execution
    /// (gracefully).
    ///
    /// # Safety
    /// The program counter must be valid and aligned, the instructions processed must end with a
    /// jump instruction.
    #[inline(always)]
    pub unsafe fn new(
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
                Instruction::try_decode(decoded_instruction).unwrap_or(ContractInstruction::Unimp);
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
                            .unwrap_or(ContractInstruction::Unimp),
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
                Instruction::try_decode(instruction_word).unwrap_or(ContractInstruction::Unimp),
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

/// Execute [`ContractInstruction`]s
pub fn execute<Regs, Memory, IF>(
    state: &mut BasicInterpreterState<Regs, (), Memory, IF, IgnoreEcallSystemInstructionHandler>,
) -> Result<(), ExecutionError<u64>>
where
    Regs: RegisterFile<<ContractInstruction as Instruction>::Reg>,
    Memory: VirtualMemory,
    IF: InstructionFetcher<ContractInstruction, Memory>,
{
    loop {
        let instruction = match state.instruction_fetcher.fetch_instruction(&state.memory) {
            Ok(FetchInstructionResult::Instruction(instruction)) => instruction,
            Ok(FetchInstructionResult::ControlFlow(ControlFlow::Continue(()))) => {
                cold_path();
                continue;
            }
            Ok(FetchInstructionResult::ControlFlow(ControlFlow::Break(()))) => {
                cold_path();
                break;
            }
            Err(error) => {
                cold_path();
                return Err(error);
            }
        };

        match instruction.execute(
            &mut state.regs,
            &mut state.ext_state,
            &mut state.memory,
            &mut state.instruction_fetcher,
            &mut state.system_instruction_handler,
        ) {
            Ok(ControlFlow::Continue((rd, rd_value))) => {
                state.regs.write(rd, rd_value);
                continue;
            }
            Ok(ControlFlow::Break(())) => {
                cold_path();
                break;
            }
            Err(error) => {
                cold_path();
                return Err(error);
            }
        }
    }

    Ok(())
}
