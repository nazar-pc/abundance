extern crate alloc;

use ab_blake3::{CHUNK_LEN, OUT_LEN};
use ab_core_primitives::ed25519::{Ed25519PublicKey, Ed25519Signature};
use ab_io_type::IoType;
use ab_io_type::bool::Bool;
use ab_riscv_interpreter::rv64::Rv64SystemInstructionHandler;
use ab_riscv_interpreter::{
    BasicInt, ExecuteError, FetchInstructionResult, GenericInstructionHandler, VirtualMemory,
    VirtualMemoryError,
};
use ab_riscv_primitives::instruction::GenericInstruction;
use ab_riscv_primitives::instruction::rv64::Rv64Instruction;
use ab_riscv_primitives::registers::{GenericRegister, Registers};
use alloc::vec::Vec;
use core::mem::offset_of;
use core::ops::ControlFlow;

/// Contract file bytes
pub const RISCV_CONTRACT_BYTES: &[u8] = {
    #[cfg(target_env = "abundance")]
    {
        &[]
    }
    #[cfg(not(target_env = "abundance"))]
    {
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

// Simple test memory implementation
#[derive(Debug)]
pub struct TestMemory<const MEMORY_SIZE: usize> {
    data: [u8; MEMORY_SIZE],
    base_addr: u64,
}

impl<const MEMORY_SIZE: usize> VirtualMemory for TestMemory<MEMORY_SIZE> {
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

impl<const MEMORY_SIZE: usize> TestMemory<MEMORY_SIZE> {
    /// Create a new test memory instance with the specified base address
    pub fn new(base_addr: u64) -> Self {
        Self {
            data: [0; _],
            base_addr,
        }
    }

    /// Get a slice of memory
    pub fn get_bytes(&self, address: u64, size: usize) -> Result<&[u8], VirtualMemoryError> {
        let offset = address
            .checked_sub(self.base_addr)
            .ok_or(VirtualMemoryError::OutOfBoundsRead { address })? as usize;

        if offset + size > self.data.len() {
            return Err(VirtualMemoryError::OutOfBoundsRead { address });
        }

        Ok(&self.data[offset..][..size])
    }

    /// Get a mutable slice of memory
    pub fn get_mut_bytes(
        &mut self,
        address: u64,
        size: usize,
    ) -> Result<&mut [u8], VirtualMemoryError> {
        let offset = address
            .checked_sub(self.base_addr)
            .ok_or(VirtualMemoryError::OutOfBoundsRead { address })? as usize;

        if offset + size > self.data.len() {
            return Err(VirtualMemoryError::OutOfBoundsRead { address });
        }

        Ok(&mut self.data[offset..][..size])
    }
}

/// Eager instruction handler eagerly decodes all instructions upfront.
///
/// `RETURN_TRAP_ADDRESS` is the address at which the interpreter will stop execution (gracefully)
#[derive(Debug, Default, Clone)]
pub struct EagerTestInstructionHandler<const RETURN_TRAP_ADDRESS: u64, Instruction> {
    instructions: Vec<Instruction>,
    base_addr: u64,
}

impl<const RETURN_TRAP_ADDRESS: u64, Instruction, Reg, Memory>
    GenericInstructionHandler<Instruction, Reg, Memory, &'static str>
    for EagerTestInstructionHandler<RETURN_TRAP_ADDRESS, Instruction>
where
    Instruction: GenericInstruction,
    Reg: GenericRegister<Type = u64>,
    [(); Reg::N]:,
    Memory: VirtualMemory,
{
    #[inline(always)]
    fn fetch_instruction(
        &mut self,
        _regs: &mut Registers<Reg>,
        _memory: &mut Memory,
        pc: &mut u64,
    ) -> Result<FetchInstructionResult<Instruction>, ExecuteError<Instruction, &'static str>> {
        let address = *pc;

        if address == RETURN_TRAP_ADDRESS {
            return Ok(FetchInstructionResult::ControlFlow(ControlFlow::Break(())));
        }

        if !address.is_multiple_of(size_of::<u32>() as u64) {
            return Err(ExecuteError::UnalignedInstructionFetch { address });
        }

        let offset = address
            .checked_sub(self.base_addr)
            .ok_or(VirtualMemoryError::OutOfBoundsRead { address })? as usize;
        let instruction_offset = offset / size_of::<u32>();

        let instruction = *self
            .instructions
            .get(instruction_offset)
            .ok_or(VirtualMemoryError::OutOfBoundsRead { address })?;
        *pc += instruction.size() as u64;

        Ok(FetchInstructionResult::Instruction(instruction))
    }
}

impl<const RETURN_TRAP_ADDRESS: u64, Instruction, Reg, Memory>
    Rv64SystemInstructionHandler<Reg, Memory, &'static str>
    for EagerTestInstructionHandler<RETURN_TRAP_ADDRESS, Instruction>
where
    Instruction: GenericInstruction,
    Reg: GenericRegister<Type = u64>,
    [(); Reg::N]:,
    Memory: VirtualMemory,
{
    #[inline(always)]
    fn handle_ecall(
        &mut self,
        _regs: &mut Registers<Reg>,
        _memory: &mut Memory,
        pc: &mut u64,
        instruction: Rv64Instruction<Reg>,
    ) -> Result<(), ExecuteError<Rv64Instruction<Reg>, &'static str>> {
        Err(ExecuteError::UnsupportedInstruction {
            address: *pc - instruction.size() as u64,
            instruction,
        })
    }
}

impl<const RETURN_TRAP_ADDRESS: u64, Instruction>
    EagerTestInstructionHandler<RETURN_TRAP_ADDRESS, Instruction>
{
    /// Create a new instance with the specified instructions and base address.
    ///
    /// Instructions are in the same order as they appear in the binary and base address corresponds
    /// to the first instruction.
    pub fn new(instructions: Vec<Instruction>, base_addr: u64) -> Self {
        Self {
            instructions,
            base_addr,
        }
    }
}
