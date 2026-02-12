#![expect(incomplete_features, reason = "generic_const_exprs")]
// TODO: This feature is not actually used in this crate, but is added as a workaround for
//  https://github.com/rust-lang/rust/issues/141492
#![feature(generic_const_exprs)]
#![feature(control_flow_ok)]

use ab_blake3::CHUNK_LEN;
use ab_contract_file::{ContractFile, ContractInstruction, ContractRegister};
use ab_core_primitives::ed25519::{Ed25519PublicKey, Ed25519Signature};
use ab_riscv_benchmarks::Benchmarks;
use ab_riscv_benchmarks::host_utils::{
    Blake3HashChunkInternalArgs, EagerTestInstructionFetcher, Ed25519VerifyInternalArgs,
    NoopRv64SystemInstructionHandler, RISCV_CONTRACT_BYTES, TestMemory, execute,
};
use ab_riscv_interpreter::BasicInstructionFetcher;
use ab_riscv_interpreter::rv64::Rv64InterpreterState;
use ab_riscv_primitives::instructions::Instruction;
use ab_riscv_primitives::registers::general_purpose::Registers;
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use ed25519_dalek::{Signer, SigningKey};
use std::collections::HashMap;
use std::hint::black_box;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::{mem, ptr, slice};

const MEMORY_BASE_ADDRESS: u64 = 0x1000;
const TRAP_ADDRESS: u64 = 0;
const MEMORY_SIZE: usize = 128 * 1024;

fn criterion_benchmark(c: &mut Criterion) {
    let mut methods = HashMap::new();
    let contract_file = ContractFile::parse(RISCV_CONTRACT_BYTES, |contract_file_method| {
        methods.insert(
            contract_file_method.method_metadata_item.method_name,
            contract_file_method.address,
        );
        Ok(())
    })
    .unwrap();

    {
        let mut group = c.benchmark_group("file");
        group.throughput(Throughput::Elements(1));

        group.bench_function("parse-only", |b| {
            b.iter(|| {
                black_box(
                    ContractFile::parse(black_box(RISCV_CONTRACT_BYTES), |_| Ok(())).unwrap(),
                );
            });
        });
        group.bench_function("parse-with-methods", |b| {
            b.iter(|| {
                let mut methods = HashMap::new();
                black_box(
                    ContractFile::parse(black_box(RISCV_CONTRACT_BYTES), |contract_file_method| {
                        methods.insert(
                            contract_file_method.method_metadata_item.method_name,
                            contract_file_method.address,
                        );
                        Ok(())
                    })
                    .unwrap(),
                );
            });
        });
        group.bench_function("iterate-methods", |b| {
            b.iter(|| {
                black_box(contract_file.iterate_methods()).count();
            });
        });

        let code = contract_file.get_code();
        group.bench_function("decode-instructions", |b| {
            b.iter(|| {
                let mut instructions = Vec::with_capacity(code.len() / size_of::<u32>());
                for instruction in code.chunks_exact(size_of::<u32>()) {
                    let instruction = u32::from_le_bytes([
                        instruction[0],
                        instruction[1],
                        instruction[2],
                        instruction[3],
                    ]);
                    instructions.push(ContractInstruction::try_decode(instruction).unwrap());
                }
                black_box(instructions);
            });
        });
    }

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

    let internal_args_addr = (MEMORY_BASE_ADDRESS + contract_memory_size as u64)
        .next_multiple_of(size_of::<u128>() as u64);

    let mut lazy_state = Rv64InterpreterState {
        regs: Registers::default(),
        memory,
        // SAFETY: Program counter is set later to the correct address, all instructions are valid
        // and contract ends with a jump
        instruction_fetcher: unsafe {
            BasicInstructionFetcher::<ContractInstruction, &'static str>::new(
                TRAP_ADDRESS,
                MEMORY_BASE_ADDRESS,
            )
        },
        system_instruction_handler: NoopRv64SystemInstructionHandler::default(),
        _phantom: PhantomData,
    };

    let mut eager_state = Rv64InterpreterState {
        regs: Registers::default(),
        memory,
        // SAFETY: Program counter is set later to the correct address
        instruction_fetcher: unsafe {
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
                        Instruction::try_decode(instruction).unwrap()
                    })
                    .collect(),
                TRAP_ADDRESS,
                MEMORY_BASE_ADDRESS + contract_file.header().read_only_section_memory_size as u64,
                MEMORY_BASE_ADDRESS,
            )
        },
        system_instruction_handler: NoopRv64SystemInstructionHandler::default(),
        _phantom: PhantomData,
    };

    {
        let mut group = c.benchmark_group("blake3_hash_chunk");
        group.throughput(Throughput::Bytes(CHUNK_LEN as u64));

        let data_to_hash = [1; CHUNK_LEN];

        group.bench_function("native", |b| {
            b.iter(|| {
                black_box(Benchmarks::blake3_hash_chunk(black_box(&data_to_hash)));
            });
        });

        let benchmarks_blake3_hash_chunk_addr = MEMORY_BASE_ADDRESS
            + u64::from(
                *methods
                    .get("benchmarks_blake3_hash_chunk".as_bytes())
                    .unwrap(),
            );

        {
            let internal_args = Blake3HashChunkInternalArgs::new(internal_args_addr, data_to_hash);
            // SAFETY: Byte representation of `#[repr(C)]` without internal padding
            let internal_args_bytes = unsafe {
                slice::from_raw_parts(
                    ptr::from_ref(&internal_args).cast::<u8>(),
                    size_of::<Blake3HashChunkInternalArgs>(),
                )
            };

            lazy_state
                .memory
                .get_mut_bytes(internal_args_addr, size_of::<Blake3HashChunkInternalArgs>())
                .unwrap()
                .copy_from_slice(internal_args_bytes);
            eager_state
                .memory
                .get_mut_bytes(internal_args_addr, size_of::<Blake3HashChunkInternalArgs>())
                .unwrap()
                .copy_from_slice(internal_args_bytes);
        }

        group.bench_function("interpreter/lazy", |b| {
            b.iter(|| {
                lazy_state
                    .set_pc(benchmarks_blake3_hash_chunk_addr)
                    .unwrap()
                    .continue_ok()
                    .unwrap();
                lazy_state
                    .regs
                    .write(ContractRegister::A0, internal_args_addr);
                lazy_state.regs.write(
                    ContractRegister::Sp,
                    MEMORY_BASE_ADDRESS + MEMORY_SIZE as u64,
                );

                black_box(execute(black_box(&mut lazy_state))).unwrap();
            });
        });

        group.bench_function("interpreter/eager", |b| {
            b.iter(|| {
                eager_state
                    .set_pc(benchmarks_blake3_hash_chunk_addr)
                    .unwrap()
                    .continue_ok()
                    .unwrap();
                eager_state
                    .regs
                    .write(ContractRegister::A0, internal_args_addr);
                eager_state.regs.write(
                    ContractRegister::Sp,
                    MEMORY_BASE_ADDRESS + MEMORY_SIZE as u64,
                );

                black_box(execute(black_box(&mut eager_state))).unwrap();
            });
        });
    }
    {
        let mut group = c.benchmark_group("ed25519_verify");
        group.throughput(Throughput::Elements(1));

        let signing_key = SigningKey::from([1; _]);
        let public_key = Ed25519PublicKey::from(signing_key.verifying_key());
        let message = [2; _];
        let signature = Ed25519Signature::from(signing_key.sign(&message));

        group.bench_function("native", |b| {
            b.iter(|| {
                black_box(Benchmarks::ed25519_verify(
                    black_box(&public_key),
                    black_box(&signature),
                    black_box(&message),
                ));
            });
        });

        let benchmarks_ed25519_verify_addr = MEMORY_BASE_ADDRESS
            + u64::from(*methods.get("benchmarks_ed25519_verify".as_bytes()).unwrap());

        {
            let internal_args =
                Ed25519VerifyInternalArgs::new(internal_args_addr, public_key, signature, message);
            // SAFETY: Byte representation of `#[repr(C)]` without internal padding
            let internal_args_bytes = unsafe {
                slice::from_raw_parts(
                    ptr::from_ref(&internal_args).cast::<u8>(),
                    size_of::<Ed25519VerifyInternalArgs>(),
                )
            };

            lazy_state
                .memory
                .get_mut_bytes(internal_args_addr, size_of::<Ed25519VerifyInternalArgs>())
                .unwrap()
                .copy_from_slice(internal_args_bytes);
            eager_state
                .memory
                .get_mut_bytes(internal_args_addr, size_of::<Ed25519VerifyInternalArgs>())
                .unwrap()
                .copy_from_slice(internal_args_bytes);
        }

        group.bench_function("interpreter/lazy", |b| {
            b.iter(|| {
                lazy_state
                    .set_pc(benchmarks_ed25519_verify_addr)
                    .unwrap()
                    .continue_ok()
                    .unwrap();
                lazy_state
                    .regs
                    .write(ContractRegister::A0, internal_args_addr);
                lazy_state.regs.write(
                    ContractRegister::Sp,
                    MEMORY_BASE_ADDRESS + MEMORY_SIZE as u64,
                );

                black_box(execute(black_box(&mut lazy_state))).unwrap();
            });
        });

        group.bench_function("interpreter/eager", |b| {
            b.iter(|| {
                eager_state
                    .set_pc(benchmarks_ed25519_verify_addr)
                    .unwrap()
                    .continue_ok()
                    .unwrap();
                eager_state
                    .regs
                    .write(ContractRegister::A0, internal_args_addr);
                eager_state.regs.write(
                    ContractRegister::Sp,
                    MEMORY_BASE_ADDRESS + MEMORY_SIZE as u64,
                );

                black_box(execute(black_box(&mut eager_state))).unwrap();
            });
        });
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
