use ab_blake3::{CHUNK_LEN, OUT_LEN};
use ab_contract_file::instruction::{ContractInstruction, ContractRegister};
use ab_core_primitives::ed25519::{Ed25519PublicKey, Ed25519Signature};
use ab_io_type::bool::Bool;
use core::mem::offset_of;

/// Contract file bytes
pub const RISCV_CONTRACT_BYTES: &[u8] = cfg_select! {
    not(feature = "build-contract") => &[],
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

/// Instruction stored by `BasicEagerInstructions::decode()` in slots whose bytes do not decode
/// into a valid instruction and after the last one.
///
/// Contract code is only expected to contain legal instructions, so this is only reachable by
/// jumping into the middle of one or by falling through the end of the code. `unimp` always fails,
/// so it never continues to the next instruction, as `BasicEagerInstructions::decode()`
/// requires.
pub const UNDECODABLE_INSTRUCTION: ContractInstruction = ContractInstruction::Unimp {
    rs1: ContractRegister::Zero,
    rs2: ContractRegister::Zero,
};
