use ab_blake3::CHUNK_LEN;
use ab_contract_file::ContractFile;
use ab_core_primitives::ed25519::{Ed25519PublicKey, Ed25519Signature};
use ab_riscv_benchmarks::Benchmarks;
use ab_riscv_benchmarks::host_utils::{
    Blake3HashChunkInternalArgs, EagerTestInstructionHandler, Ed25519VerifyInternalArgs,
    LazyTestInstructionHandler, RISCV_CONTRACT_BYTES, TestMemory,
};
use ab_riscv_interpreter::execute_rv64mbzbc;
use ab_riscv_primitives::instruction::{GenericBaseInstruction, Rv64MBZbcInstruction};
use ab_riscv_primitives::registers::{EReg64, ERegisters64, GenericRegisters64};
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use ed25519_zebra::SigningKey;
use std::collections::HashMap;
use std::hint::black_box;
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
                    instructions.push(Rv64MBZbcInstruction::<EReg64>::decode(instruction));
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

    let mut regs = ERegisters64::default();
    let internal_args_addr = (MEMORY_BASE_ADDRESS + contract_memory_size as u64)
        .next_multiple_of(size_of::<u128>() as u64);
    let mut lazy_handler = LazyTestInstructionHandler::<TRAP_ADDRESS>;
    let mut eager_handler = EagerTestInstructionHandler::<TRAP_ADDRESS, _>::new(
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
                GenericBaseInstruction::decode(instruction)
            })
            .collect(),
        MEMORY_BASE_ADDRESS + contract_file.header().read_only_section_memory_size as u64,
    );

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

            memory
                .get_mut_bytes(internal_args_addr, size_of::<Blake3HashChunkInternalArgs>())
                .unwrap()
                .copy_from_slice(internal_args_bytes);
        }

        group.bench_function("interpreter/lazy", |b| {
            b.iter(|| {
                let mut pc = benchmarks_blake3_hash_chunk_addr;
                regs.write(EReg64::A0, internal_args_addr);
                regs.write(EReg64::Sp, MEMORY_BASE_ADDRESS + MEMORY_SIZE as u64);

                black_box(execute_rv64mbzbc(
                    black_box(&mut regs),
                    black_box(&mut memory),
                    black_box(&mut pc),
                    black_box(&mut lazy_handler),
                ))
                .unwrap();
            });
        });

        group.bench_function("interpreter/eager", |b| {
            b.iter(|| {
                let mut pc = benchmarks_blake3_hash_chunk_addr;
                regs.write(EReg64::A0, internal_args_addr);
                regs.write(EReg64::Sp, MEMORY_BASE_ADDRESS + MEMORY_SIZE as u64);

                black_box(execute_rv64mbzbc(
                    black_box(&mut regs),
                    black_box(&mut memory),
                    black_box(&mut pc),
                    black_box(&mut eager_handler),
                ))
                .unwrap();
            });
        });
    }
    {
        let mut group = c.benchmark_group("ed25519_verify");
        group.throughput(Throughput::Elements(1));

        let signing_key = SigningKey::from([1; _]);
        let public_key = Ed25519PublicKey::from(signing_key.verification_key());
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

            memory
                .get_mut_bytes(internal_args_addr, size_of::<Ed25519VerifyInternalArgs>())
                .unwrap()
                .copy_from_slice(internal_args_bytes);
        }

        group.bench_function("interpreter/lazy", |b| {
            b.iter(|| {
                let mut pc = benchmarks_ed25519_verify_addr;
                regs.write(EReg64::A0, internal_args_addr);
                regs.write(EReg64::Sp, MEMORY_BASE_ADDRESS + MEMORY_SIZE as u64);

                black_box(execute_rv64mbzbc(
                    black_box(&mut regs),
                    black_box(&mut memory),
                    black_box(&mut pc),
                    black_box(&mut lazy_handler),
                ))
                .unwrap();
            });
        });

        group.bench_function("interpreter/eager", |b| {
            b.iter(|| {
                let mut pc = benchmarks_ed25519_verify_addr;
                regs.write(EReg64::A0, internal_args_addr);
                regs.write(EReg64::Sp, MEMORY_BASE_ADDRESS + MEMORY_SIZE as u64);

                black_box(execute_rv64mbzbc(
                    black_box(&mut regs),
                    black_box(&mut memory),
                    black_box(&mut pc),
                    black_box(&mut eager_handler),
                ))
                .unwrap();
            });
        });
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
