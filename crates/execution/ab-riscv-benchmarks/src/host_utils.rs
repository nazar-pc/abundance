use ab_blake3::{CHUNK_LEN, OUT_LEN};
use ab_contract_file::instruction::{ContractInstruction, ContractRegister};
use ab_core_primitives::ed25519::{Ed25519PublicKey, Ed25519Signature};
use ab_io_type::bool::Bool;
use ab_riscv_interpreter::prelude::*;
use ab_riscv_primitives::prelude::*;
use core::hint::cold_path;
use core::mem::offset_of;
use core::ops::ControlFlow;

/// Contract file bytes
pub const RISCV_CONTRACT_BYTES: &[u8] = cfg_select! {
    target_env = "abundance" => &[],
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

const _: () = {
    assert!(
        size_of::<Blake3HashChunkInternalArgs>()
            == offset_of!(Blake3HashChunkInternalArgs, result) + size_of::<[u8; OUT_LEN]>(),
        "`Blake3HashChunkInternalArgs` must not have implicit padding"
    );
};

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
    /// Explicit trailing padding.
    ///
    /// The host copies the byte representation of this data structure into guest memory, which is
    /// only sound if every byte of it is initialized, hence implicit padding must not exist here.
    pub padding: [u8; 7],
}

const _: () = {
    assert!(
        size_of::<Ed25519VerifyInternalArgs>()
            == offset_of!(Ed25519VerifyInternalArgs, padding) + size_of::<[u8; 7]>(),
        "`Ed25519VerifyInternalArgs` must not have implicit padding"
    );
};

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
            padding: [0; _],
        }
    }

    /// Extract result
    pub fn result(&self) -> Bool {
        self.result
    }
}

/// Instruction stored by [`BasicEagerInstructions::decode()`] in slots whose bytes do not decode
/// into a valid instruction.
///
/// Contract code is only expected to contain legal instructions, so this is only reachable by
/// jumping into the middle of one.
///
/// [`BasicEagerInstructions::decode()`]: ab_riscv_interpreter::basic::BasicEagerInstructions::decode
pub const UNDECODABLE_INSTRUCTION: ContractInstruction = ContractInstruction::Unimp {
    rs1: ContractRegister::Zero,
    rs2: ContractRegister::Zero,
};

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

    #[inline(always)]
    unsafe fn try_set_pc_relative(&mut self, instruction_size: u8, offset: i32) -> bool {
        let old_pc = <Self as ProgramCounter<_, Memory>>::old_pc(self, instruction_size);
        let pc = old_pc.wrapping_add_signed(i64::from(offset));
        // Stored either way: on the way out it is what `failed_branch()` reports on, and until then
        // nothing else is allowed to look at it
        self.pc = pc;

        pc != self.return_trap_address
            && pc.is_multiple_of(u64::from(
                ContractInstruction::<ContractRegister>::ALIGNMENT,
            ))
    }

    #[cold]
    #[inline(never)]
    unsafe fn failed_branch(
        &mut self,
        memory: &Memory,
    ) -> Result<ControlFlow<()>, ExecutionError<u64>> {
        // The program counter holds the refused target, and `set_pc()` is what says what is wrong
        // with it
        self.set_pc(memory, self.pc)
    }

    #[inline]
    fn set_pc(&mut self, memory: &Memory, pc: u64) -> Result<ControlFlow<()>, ExecutionError<u64>> {
        if pc == self.return_trap_address {
            cold_path();
            return Ok(ControlFlow::Break(()));
        }

        if !pc.is_multiple_of(u64::from(
            ContractInstruction::<ContractRegister>::ALIGNMENT,
        )) {
            cold_path();
            return Err(ExecutionError::UnalignedInstruction {
                address: PackedAddress::new(pc),
            });
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
    type Peeked = ContractInstruction;

    #[inline(always)]
    fn peeked_instruction<'a>(
        &'a self,
        peeked: &'a ContractInstruction,
    ) -> &'a ContractInstruction {
        peeked
    }

    #[inline]
    fn peek_instruction(&mut self, memory: &Memory) -> FetchInstructionResult<ContractInstruction> {
        // SAFETY: Constructor guarantees that the last instruction is a jump, which means going
        // through `Self::set_pc()` method does the necessary bounds check, so the program counter
        // always sits on an instruction.
        let instruction = unsafe { memory.read_unchecked(self.pc) };
        // SAFETY: All instructions are valid, according to the constructor contract
        let instruction =
            unsafe { ContractInstruction::try_decode(instruction).unwrap_unchecked() };

        FetchInstructionResult::Instruction(instruction)
    }

    #[inline]
    unsafe fn advance(&mut self, instruction_size: u8) {
        self.pc = self.pc.wrapping_add(u64::from(instruction_size));
    }

    #[inline]
    fn fetch_instruction(
        &mut self,
        memory: &Memory,
    ) -> FetchInstructionResult<ContractInstruction> {
        let result =
            InstructionFetcher::<ContractInstruction, Memory>::peek_instruction(self, memory);

        if let FetchInstructionResult::Instruction(instruction) = result {
            // SAFETY: The instruction was just peeked successfully, and this is the only place that
            // moves past it
            unsafe {
                InstructionFetcher::<ContractInstruction, Memory>::advance(
                    self,
                    instruction.size(),
                );
            }
        }

        result
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
