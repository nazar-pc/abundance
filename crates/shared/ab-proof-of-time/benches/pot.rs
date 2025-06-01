use aes::Aes128;
use aes::cipher::{Array, BlockCipherDecrypt, BlockCipherEncrypt, KeyInit};
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

const NUM_CHECKPOINTS: usize = 8;

fn create_generic(
    seed: [u8; 16],
    key: [u8; 16],
    checkpoint_iterations: u32,
) -> [[u8; 16]; NUM_CHECKPOINTS] {
    let cipher = Aes128::new(&Array::from(key));
    let mut cur_block = Array::from(seed);

    let mut checkpoints = [[0; 16]; NUM_CHECKPOINTS];
    for checkpoint in checkpoints.iter_mut() {
        for _ in 0..checkpoint_iterations {
            // Encrypt in place to produce the next block.
            cipher.encrypt_block(&mut cur_block);
        }
        checkpoint.copy_from_slice(&cur_block);
    }

    checkpoints
}

fn verify_sequential_generic(
    seed: [u8; 16],
    key: [u8; 16],
    checkpoints: &[[u8; 16]; NUM_CHECKPOINTS],
    checkpoint_iterations: u32,
) -> bool {
    let cipher = Aes128::new(&Array::from(key));

    let mut inputs = [[0; 16]; NUM_CHECKPOINTS];
    inputs[0] = seed;
    inputs[1..].copy_from_slice(&checkpoints[..NUM_CHECKPOINTS - 1]);

    let mut outputs = [[0; 16]; NUM_CHECKPOINTS];
    outputs.copy_from_slice(checkpoints);

    for _ in 0..checkpoint_iterations / 2 {
        cipher.encrypt_blocks(Array::cast_slice_from_core_mut(&mut inputs));
        cipher.decrypt_blocks(Array::cast_slice_from_core_mut(&mut outputs));
    }

    inputs == outputs
}

fn criterion_benchmark(c: &mut Criterion) {
    let seed = [1; 16];
    let key = [2; 16];
    // About 1s on 6.0 GHz Raptor Lake CPU (14900K)
    let iterations = 200_032_000;

    let checkpoints = create_generic(seed, key, iterations / NUM_CHECKPOINTS as u32);

    c.bench_function("verify", |b| {
        b.iter(|| {
            black_box(verify_sequential_generic(
                black_box(seed),
                black_box(key),
                black_box(&checkpoints),
                black_box(iterations / NUM_CHECKPOINTS as u32),
            ));
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
