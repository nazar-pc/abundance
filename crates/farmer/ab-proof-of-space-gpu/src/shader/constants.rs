// TODO: Replace this constant with usage of `PosProof::K` after
//  https://github.com/Rust-GPU/rust-gpu/pull/249 is merged
pub(super) const K: u8 = 20;
#[cfg(not(target_arch = "spirv"))]
const _: () = {
    assert!(K == ab_core_primitives::pos::PosProof::K);
};
/// PRNG extension parameter to avoid collisions
pub(super) const PARAM_EXT: u8 = 6;
// pub(super) const PARAM_M: u16 = 1 << PARAM_EXT;
// pub(super) const PARAM_B: u16 = 119;
// pub(super) const PARAM_C: u16 = 127;
// pub(super) const PARAM_BC: u16 = PARAM_B * PARAM_C;
