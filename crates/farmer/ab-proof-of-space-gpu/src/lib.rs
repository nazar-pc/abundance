//! Proof of space plotting utilities for GPU (Vulkan/Metal).
//!
//! Similarly to `ab-proof-of-space`, max supported `K` within range `15..=24` due to internal data
//! structures used (`ab-proof-of-space` also supports `K=25`, but this crate doesn't for now).

#![cfg_attr(target_arch = "spirv", no_std)]
#![feature(array_windows, bigint_helper_methods, ptr_as_ref_unchecked, step_trait)]
#![cfg_attr(
    all(test, not(miri), not(target_arch = "spirv")),
    feature(
        const_convert,
        const_trait_impl,
        maybe_uninit_fill,
        maybe_uninit_slice,
        new_zeroed_alloc
    )
)]

// This is used for benchmarks of isolated shaders externally, not for general use
#[doc(hidden)]
pub mod shader;

// TODO: Remove gate after https://github.com/Rust-GPU/rust-gpu/pull/249
#[cfg(not(target_arch = "spirv"))]
use ab_core_primitives::pos::PosProof;

// TODO: Remove gate after https://github.com/Rust-GPU/rust-gpu/pull/249
#[cfg(not(target_arch = "spirv"))]
const _: () = {
    assert!(PosProof::K >= 15 && PosProof::K <= 24);
};
