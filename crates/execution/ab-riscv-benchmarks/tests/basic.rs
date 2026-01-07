#![feature(ptr_as_ref_unchecked)]

use ab_contract_file::ContractFile;
use ab_core_primitives::ed25519::{Ed25519PublicKey, Ed25519Signature};
use ab_riscv_benchmarks::Benchmarks;
use ab_riscv_benchmarks::host_utils::{
    Blake3HashChunkInternalArgs, Ed25519VerifyInternalArgs, RISCV_CONTRACT_BYTES,
    TestInstructionHandler, TestMemory,
};
use ab_riscv_interpreter::execute_rv64mbzbc;
use ab_riscv_primitives::registers::{EReg64, ERegisters64, GenericRegisters64};
use ed25519_zebra::SigningKey;
use std::collections::HashMap;
use std::mem::MaybeUninit;
use std::{mem, ptr, slice};

const MEMORY_BASE_ADDRESS: u64 = 0x1000;
const TRAP_ADDRESS: u64 = 0;
const MEMORY_SIZE: usize = 128 * 1024;

fn call_method<IA, CIA>(method_name: &str, create_internal_args: CIA) -> IA
where
    IA: Copy,
    CIA: FnOnce(u64) -> IA,
{
    let mut methods = HashMap::new();
    let contract_file = ContractFile::parse(RISCV_CONTRACT_BYTES, |contract_file_method| {
        methods.insert(
            contract_file_method.method_metadata_item.method_name,
            contract_file_method.address,
        );
        Ok(())
    })
    .unwrap();

    let mut memory = TestMemory::<MEMORY_SIZE>::new(MEMORY_BASE_ADDRESS);

    let contract_memory_size = contract_file.contract_memory_size() as usize;
    if !contract_file.initialize_contract_memory({
        let output_memory = memory
            .get_mut_bytes(MEMORY_BASE_ADDRESS, contract_memory_size)
            .unwrap();
        // SAFETY: Casting initialized memory into uninitialized memory of the same size is safe
        unsafe { mem::transmute::<&mut [u8], &mut [MaybeUninit<u8>]>(output_memory) }
    }) {
        panic!(
            "Failed to initialize contract memory of size {contract_memory_size} bytes at base \
            address 0x{MEMORY_BASE_ADDRESS:x}",
        );
    }

    let mut regs = ERegisters64::default();
    let internal_args_addr = (MEMORY_BASE_ADDRESS + contract_memory_size as u64)
        .next_multiple_of(size_of::<u128>() as u64);

    {
        let internal_args = create_internal_args(internal_args_addr);
        // SAFETY: Byte representation of `#[repr(C)]` without internal padding
        let internal_args_bytes = unsafe {
            slice::from_raw_parts(ptr::from_ref(&internal_args).cast::<u8>(), size_of::<IA>())
        };

        memory
            .get_mut_bytes(internal_args_addr, size_of::<IA>())
            .unwrap()
            .copy_from_slice(internal_args_bytes);
    }

    regs.write(EReg64::A0, internal_args_addr);
    regs.write(EReg64::Sp, MEMORY_BASE_ADDRESS + MEMORY_SIZE as u64);

    let mut pc = MEMORY_BASE_ADDRESS + u64::from(*methods.get(method_name.as_bytes()).unwrap());
    let mut handler = TestInstructionHandler::<TRAP_ADDRESS>;

    execute_rv64mbzbc(&mut regs, &mut memory, &mut pc, &mut handler).unwrap();

    // SAFETY: Byte representation of `#[repr(C)]` without internal padding
    *unsafe {
        memory
            .get_bytes(internal_args_addr, size_of::<IA>())
            .unwrap()
            .as_ptr()
            .cast::<IA>()
            .as_ref_unchecked()
    }
}

// TODO: Unlock if it becomes fast enough to run in CI
#[cfg_attr(miri, ignore)]
#[test]
fn blake3_hash_chunk() {
    let data_to_hash = [1; _];
    let expected_hash = Benchmarks::blake3_hash_chunk(&data_to_hash);

    let internal_args = call_method("benchmarks_blake3_hash_chunk", |internal_args_addr| {
        Blake3HashChunkInternalArgs::new(internal_args_addr, data_to_hash)
    });
    let actual_hash = internal_args.result();

    assert_eq!(expected_hash, actual_hash);
}

// TODO: Unlock if it becomes fast enough to run in CI
#[cfg_attr(miri, ignore)]
#[test]
fn ed25519_verify() {
    let signing_key = SigningKey::from([1; _]);
    let public_key = Ed25519PublicKey::from(signing_key.verification_key());
    let message = [2; _];
    let other_message = [3; _];
    let signature = Ed25519Signature::from(signing_key.sign(&message));

    // Valid
    assert!(Benchmarks::ed25519_verify(&public_key, &signature, &message).get());
    // Invalid
    assert!(!Benchmarks::ed25519_verify(&public_key, &signature, &other_message).get());

    // Valid
    {
        let internal_args = call_method("benchmarks_ed25519_verify", |internal_args_addr| {
            Ed25519VerifyInternalArgs::new(internal_args_addr, public_key, signature, message)
        });

        assert!(internal_args.result.get());
    }
    // Invalid
    {
        let internal_args = call_method("benchmarks_ed25519_verify", |internal_args_addr| {
            Ed25519VerifyInternalArgs::new(internal_args_addr, public_key, signature, other_message)
        });

        assert!(!internal_args.result.get());
    }
}
