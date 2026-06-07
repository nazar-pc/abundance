//! AES related functionality.

#[cfg(target_arch = "aarch64")]
mod aarch64;
#[cfg(test)]
mod tests;
#[cfg(target_arch = "x86_64")]
mod x86_64;

use ab_core_primitives::pot::{PotCheckpoints, PotKey, PotOutput, PotSeed};
use aes::Aes128;
use aes::cipher::array::Array;
use aes::cipher::{BlockCipherDecrypt, BlockCipherEncrypt, KeyInit};

/// Creates an AES-based proof
#[inline(always)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub(crate) fn create(seed: PotSeed, key: PotKey, checkpoint_iterations: u32) -> PotCheckpoints {
    cfg_select! {
        target_arch = "x86_64" => {{
            cpufeatures::new!(has_aes, "aes");
            if has_aes::get() {
                // SAFETY: Checked `aes` feature
                return unsafe {
                    x86_64::create(seed.as_ref(), key.as_ref(), checkpoint_iterations)
                };
            }
        }}
        target_arch = "aarch64" => {{
            cpufeatures::new!(has_aes, "aes");
            if has_aes::get() {
                // SAFETY: Checked `aes` feature
                return unsafe {
                    aarch64::create(seed.as_ref(), key.as_ref(), checkpoint_iterations)
                };
            }
        }}
        _ => {
            // Nothing to do here, fallback will be used
        }
    }

    create_generic(seed, key, checkpoint_iterations)
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
fn create_generic(seed: PotSeed, key: PotKey, checkpoint_iterations: u32) -> PotCheckpoints {
    let key = Array::from(*key);
    let cipher = Aes128::new(&key);
    let mut cur_block = Array::from(*seed);

    let mut checkpoints = PotCheckpoints::default();
    for checkpoint in checkpoints.iter_mut() {
        for _ in 0..checkpoint_iterations {
            // Encrypt in place to produce the next block.
            cipher.encrypt_block(&mut cur_block);
        }
        checkpoint.copy_from_slice(&cur_block);
    }

    checkpoints
}

/// Verifies an AES-based proof sequentially.
///
/// Panics if `checkpoint_iterations` is not a multiple of `2`.
#[inline(always)]
// TODO: Figure out what is wrong with macOS here
#[cfg_attr(
    all(
        feature = "no-panic",
        not(any(target_os = "macos", target_arch = "riscv64"))
    ),
    no_panic::no_panic
)]
pub(crate) fn verify_sequential(
    seed: PotSeed,
    key: PotKey,
    checkpoints: &PotCheckpoints,
    checkpoint_iterations: u32,
) -> bool {
    assert_eq!(checkpoint_iterations % 2, 0);

    cfg_select! {
        target_arch = "x86_64" => {{
            // TODO: Remove this guard once this no longer causes problems for compiler
            #[cfg(not(feature = "no-panic"))]
            {
                cpufeatures::new!(has_avx512f_vaes, "avx512f", "vaes");
                if has_avx512f_vaes::get() {
                    // SAFETY: Checked `avx512f` and `vaes` features
                    return unsafe {
                        x86_64::verify_sequential_avx512f_vaes(
                            &seed,
                            &key,
                            checkpoints,
                            checkpoint_iterations,
                        )
                    };
                }

                cpufeatures::new!(has_avx2_vaes, "avx2", "vaes");
                if has_avx2_vaes::get() {
                    // SAFETY: Checked `avx2` and `vaes` features
                    return unsafe {
                        x86_64::verify_sequential_avx2_vaes(
                            &seed,
                            &key,
                            checkpoints,
                            checkpoint_iterations,
                        )
                    };
                }
            }

            cpufeatures::new!(has_aes_sse41, "aes", "sse4.1");
            if has_aes_sse41::get() {
                // SAFETY: Checked `aes` and `sse4.1` features
                return unsafe {
                    x86_64::verify_sequential_aes_sse41(
                        &seed,
                        &key,
                        checkpoints,
                        checkpoint_iterations,
                    )
                };
            }
        }}
        target_arch = "aarch64" => {
            cpufeatures::new!(has_aes, "aes");
            if has_aes::get() {
                // SAFETY: Checked `aes` feature
                return unsafe {
                    aarch64::verify_sequential_aes(
                        &seed,
                        &key,
                        checkpoints,
                        checkpoint_iterations,
                    )
                };
            }
        }
        _ => {
            // Nothing to do here, fallback will be used
        }
    }

    verify_sequential_generic(seed, key, checkpoints, checkpoint_iterations)
}

// TODO: Figure out what is wrong with macOS here
#[cfg_attr(
    all(
        feature = "no-panic",
        not(any(target_os = "macos", target_arch = "riscv64"))
    ),
    no_panic::no_panic
)]
fn verify_sequential_generic(
    seed: PotSeed,
    key: PotKey,
    checkpoints: &PotCheckpoints,
    checkpoint_iterations: u32,
) -> bool {
    let key = Array::from(*key);
    let cipher = Aes128::new(&key);

    let mut inputs = [[0u8; 16]; PotCheckpoints::NUM_CHECKPOINTS.get() as usize];
    inputs[0] = *seed;
    inputs[1..].copy_from_slice(PotOutput::repr_from_slice(
        &checkpoints[..PotCheckpoints::NUM_CHECKPOINTS.get() as usize - 1],
    ));

    let mut outputs = [[0u8; 16]; PotCheckpoints::NUM_CHECKPOINTS.get() as usize];
    outputs.copy_from_slice(PotOutput::repr_from_slice(checkpoints.as_slice()));

    for _ in 0..checkpoint_iterations / 2 {
        cipher.encrypt_blocks(Array::cast_slice_from_core_mut(&mut inputs));
        cipher.decrypt_blocks(Array::cast_slice_from_core_mut(&mut outputs));
    }

    inputs == outputs
}
