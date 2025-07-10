//! Proof of space plotting utilities for GPU (Vulkan/Metal).
//!
//! Just like in `ab-proof-of-space`, max supported `K` within range `15..=25` due to internal data
//! structures used.

#![cfg_attr(target_arch = "spirv", no_std)]
#![feature(array_chunks, bigint_helper_methods)]

// This is used for benchmarks of isolated shaders externally, not for general use
#[doc(hidden)]
pub mod shader;

// TODO: Remove gate after https://github.com/Rust-GPU/rust-gpu/pull/249
#[cfg(not(target_arch = "spirv"))]
use ab_core_primitives::pos::PosProof;

// TODO: Remove gate after https://github.com/Rust-GPU/rust-gpu/pull/249
#[cfg(not(target_arch = "spirv"))]
const _: () = {
    assert!(PosProof::K >= 15 && PosProof::K <= 25);
};
