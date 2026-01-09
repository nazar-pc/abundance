#![feature(ptr_as_ref_unchecked)]
#![expect(incomplete_features, reason = "generic_const_exprs")]
// TODO: This feature is not actually used in this crate, but is added as a workaround for
//  https://github.com/rust-lang/rust/issues/141492
#![feature(generic_const_exprs)]

use ab_blake3::OUT_LEN;
use ab_contract_file::{ContractFile, Register};
use ab_core_primitives::ed25519::{Ed25519PublicKey, Ed25519Signature};
use ab_riscv_benchmarks::Benchmarks;
use ab_riscv_benchmarks::host_utils::{
    Blake3HashChunkInternalArgs, EagerTestInstructionFetcher, Ed25519VerifyInternalArgs,
    RISCV_CONTRACT_BYTES, TestMemory, execute,
};
use ab_riscv_interpreter::BasicInstructionFetcher;
use ab_riscv_primitives::instruction::BaseInstruction;
use ab_riscv_primitives::registers::Registers;
use ed25519_zebra::SigningKey;
use std::collections::HashMap;
use std::mem::MaybeUninit;
use std::{mem, ptr, slice};

const MEMORY_BASE_ADDRESS: u64 = 0x1000;
const TRAP_ADDRESS: u64 = 0;
const MEMORY_SIZE: usize = 128 * 1024;

enum RunType {
    Lazy,
    Eager,
}

fn call_method<IA, CIA>(method_name: &str, create_internal_args: CIA, run_type: RunType) -> IA
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

    let mut regs = Registers::default();
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

    regs.write(Register::A0, internal_args_addr);
    regs.write(Register::Sp, MEMORY_BASE_ADDRESS + MEMORY_SIZE as u64);

    let pc = MEMORY_BASE_ADDRESS + u64::from(*methods.get(method_name.as_bytes()).unwrap());
    match run_type {
        RunType::Lazy => {
            // SAFETY: Program counter is trusted
            let mut instruction_fetcher = unsafe { BasicInstructionFetcher::new(TRAP_ADDRESS, pc) };

            execute(&mut regs, &mut memory, &mut instruction_fetcher).unwrap();
        }
        RunType::Eager => {
            // SAFETY: Program counter is trusted
            let mut instruction_fetcher = unsafe {
                EagerTestInstructionFetcher::new(
                    contract_file
                        .get_code()
                        .chunks_exact(size_of::<u32>())
                        .map(|instruction| {
                            let instruction = u32::from_le_bytes([
                                instruction[0],
                                instruction[1],
                                instruction[2],
                                instruction[3],
                            ]);
                            BaseInstruction::decode(instruction)
                        })
                        .collect(),
                    TRAP_ADDRESS,
                    MEMORY_BASE_ADDRESS
                        + contract_file.header().read_only_section_memory_size as u64,
                    pc,
                )
            };

            execute(&mut regs, &mut memory, &mut instruction_fetcher).unwrap();
        }
    }

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
fn blake3_hash_chunk_lazy() {
    let data_to_hash = [1; _];
    let expected_hash = Benchmarks::blake3_hash_chunk(&data_to_hash);

    let internal_args = call_method(
        "benchmarks_blake3_hash_chunk",
        |internal_args_addr| Blake3HashChunkInternalArgs::new(internal_args_addr, data_to_hash),
        RunType::Lazy,
    );
    let actual_hash = internal_args.result();

    assert_eq!(expected_hash, actual_hash);
}

#[test]
fn blake3_hash_chunk_eager() {
    let data_to_hash = [1; _];
    let expected_hash = Benchmarks::blake3_hash_chunk(&data_to_hash);

    let internal_args = call_method(
        "benchmarks_blake3_hash_chunk",
        |internal_args_addr| Blake3HashChunkInternalArgs::new(internal_args_addr, data_to_hash),
        RunType::Eager,
    );
    let actual_hash = internal_args.result();

    assert_eq!(expected_hash, actual_hash);
}

// TODO: Unlock if it becomes fast enough to run in CI
#[cfg_attr(miri, ignore)]
#[test]
fn ed25519_verify_valid_lazy() {
    let signing_key = SigningKey::from([1; _]);
    let public_key = Ed25519PublicKey::from(signing_key.verification_key());
    let message = [2; OUT_LEN];
    let signature = Ed25519Signature::from(signing_key.sign(&message));

    assert!(Benchmarks::ed25519_verify(&public_key, &signature, &message).get());

    let internal_args = call_method(
        "benchmarks_ed25519_verify",
        |internal_args_addr| {
            Ed25519VerifyInternalArgs::new(internal_args_addr, public_key, signature, message)
        },
        RunType::Lazy,
    );

    assert!(internal_args.result.get());
}

// TODO: Unlock if it becomes fast enough to run in CI
#[cfg_attr(miri, ignore)]
#[test]
fn ed25519_verify_invalid_lazy() {
    let signing_key = SigningKey::from([1; _]);
    let public_key = Ed25519PublicKey::from(signing_key.verification_key());
    let message = [2; OUT_LEN];
    let other_message = [3; OUT_LEN];
    let signature = Ed25519Signature::from(signing_key.sign(&message));

    assert!(!Benchmarks::ed25519_verify(&public_key, &signature, &other_message).get());

    let internal_args = call_method(
        "benchmarks_ed25519_verify",
        |internal_args_addr| {
            Ed25519VerifyInternalArgs::new(internal_args_addr, public_key, signature, other_message)
        },
        RunType::Lazy,
    );

    assert!(!internal_args.result.get());
}

// TODO: Unlock if it becomes fast enough to run in CI
#[cfg_attr(miri, ignore)]
#[test]
fn ed25519_verify_valid_eager() {
    let signing_key = SigningKey::from([1; _]);
    let public_key = Ed25519PublicKey::from(signing_key.verification_key());
    let message = [2; OUT_LEN];
    let signature = Ed25519Signature::from(signing_key.sign(&message));

    assert!(Benchmarks::ed25519_verify(&public_key, &signature, &message).get());

    let internal_args = call_method(
        "benchmarks_ed25519_verify",
        |internal_args_addr| {
            Ed25519VerifyInternalArgs::new(internal_args_addr, public_key, signature, message)
        },
        RunType::Eager,
    );

    assert!(internal_args.result.get());
}

// TODO: Unlock if it becomes fast enough to run in CI
#[cfg_attr(miri, ignore)]
#[test]
fn ed25519_verify_invalid_eager() {
    let signing_key = SigningKey::from([1; _]);
    let public_key = Ed25519PublicKey::from(signing_key.verification_key());
    let message = [2; OUT_LEN];
    let other_message = [3; OUT_LEN];
    let signature = Ed25519Signature::from(signing_key.sign(&message));

    assert!(!Benchmarks::ed25519_verify(&public_key, &signature, &other_message).get());

    let internal_args = call_method(
        "benchmarks_ed25519_verify",
        |internal_args_addr| {
            Ed25519VerifyInternalArgs::new(internal_args_addr, public_key, signature, other_message)
        },
        RunType::Eager,
    );

    assert!(!internal_args.result.get());
}
