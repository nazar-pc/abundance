//! Opaque helpers for RV64 Zknd extension

use ab_riscv_primitives::prelude::*;

/// Key schedule operations shared across all backends.
///
/// Neither `aes64ks1i` nor `aes64ks2` has a hardware mapping on non-riscv64.
#[cfg(not(all(not(miri), target_arch = "riscv64", target_feature = "zknd")))]
#[expect(
    clippy::inline_modules,
    reason = "Small internal API, it is more readable this way"
)]
mod ks {
    use crate::rv32::zk::zkn::zknd::rv32_zknd_helpers::SBOX;
    use ab_riscv_primitives::prelude::*;

    /// AES key schedule step 1.
    ///
    /// Pseudocode (RISC-V Crypto spec Sail source):
    /// ```text
    ///   temp = rs1[63:32]
    ///   if rnum != 0xA: temp = RotWord(temp)
    ///   temp = SubWord(temp)
    ///   if rnum != 0xA: temp ^= RCON[rnum]
    ///   rd = temp | (temp << 32)
    /// ```
    #[inline(always)]
    pub(super) fn aes64ks1i(rs1: u64, rnum: Rv64ZkndKsRnum) -> u64 {
        let w = (rs1 >> 32u8) as u32;

        let rotated = if rnum == Rv64ZkndKsRnum::Final {
            w
        } else {
            w.rotate_right(8)
        };

        let b0 = u32::from(SBOX[(rotated & 0xff) as usize]);
        let b1 = u32::from(SBOX[((rotated >> 8u8) & 0xff) as usize]);
        let b2 = u32::from(SBOX[((rotated >> 16u8) & 0xff) as usize]);
        let b3 = u32::from(SBOX[((rotated >> 24u8) & 0xff) as usize]);
        let subbed = b0 | (b1 << 8u8) | (b2 << 16u8) | (b3 << 24u8);

        let result = if let Some(round_constant) = rnum.constant() {
            subbed ^ u32::from(round_constant)
        } else {
            subbed
        };

        u64::from(result) | (u64::from(result) << 32u8)
    }

    /// AES key schedule step 2.
    ///
    /// Pseudocode (RISC-V Crypto spec):
    /// ```text
    ///   w0 = rs1[63:32] ^ rs2[31:0]
    ///   w1 = rs1[63:32] ^ rs2[31:0] ^ rs2[63:32]
    ///   rd = w0 | (w1 << 32)
    /// ```
    #[inline(always)]
    pub(super) fn aes64ks2(rs1: u64, rs2: u64) -> u64 {
        let w0 = (rs1 >> 32u8) as u32 ^ rs2 as u32;
        let w1 = w0 ^ (rs2 >> 32u8) as u32;
        u64::from(w0) | (u64::from(w1) << 32u8)
    }
}

cfg_select! {
    all(
        not(miri),
        target_arch = "riscv64",
        target_feature = "zknd"
    ) => {
        // Nothing, calling native intrinsics
    }
    all(target_arch = "x86_64", target_feature = "aes", target_feature = "sse4.1") => {
        /// x86-64 AES-NI implementation
        #[expect(
            clippy::inline_modules,
            reason = "Small internal API, it is more readable this way"
        )]
        mod x86_64 {
            use core::arch::x86_64::{
                _mm_aesdec_si128, _mm_aesdeclast_si128, _mm_aesimc_si128, _mm_extract_epi64,
                _mm_set_epi64x, _mm_setzero_si128,
            };

            /// `_mm_aesdeclast_si128(state, zero)` computes InvShiftRows + InvSubBytes, then XORs
            /// with the round key. Zero key -> no-op XOR, matching `aes64ds`.
            #[inline]
            #[target_feature(enable = "aes,sse4.1")]
            pub(super) fn aes64ds(rs1: u64, rs2: u64) -> u64 {
                let state = _mm_set_epi64x(rs2.cast_signed(), rs1.cast_signed());
                let zero = _mm_setzero_si128();
                let result = _mm_aesdeclast_si128(state, zero);
                _mm_extract_epi64::<0>(result).cast_unsigned()
            }

            /// `_mm_aesdec_si128(state, zero)` computes InvShiftRows + InvSubBytes + InvMixColumns,
            /// then XORs with the round key. Zero key -> no-op XOR.
            #[inline]
            #[target_feature(enable = "aes,sse4.1")]
            pub(super) fn aes64dsm(rs1: u64, rs2: u64) -> u64 {
                let state = _mm_set_epi64x(rs2.cast_signed(), rs1.cast_signed());
                let zero = _mm_setzero_si128();
                let result = _mm_aesdec_si128(state, zero);
                _mm_extract_epi64::<0>(result).cast_unsigned()
            }

            /// `_mm_aesimc_si128` applies InvMixColumns to all four 32-bit columns.
            /// `rs1` is replicated into both halves; we extract the low 64 bits.
            #[inline]
            #[target_feature(enable = "aes,sse4.1")]
            pub(super) fn aes64im(rs1: u64) -> u64 {
                let state = _mm_set_epi64x(rs1.cast_signed(), rs1.cast_signed());
                let result = _mm_aesimc_si128(state);
                _mm_extract_epi64::<0>(result).cast_unsigned()
            }
        }
    }
    all(target_arch = "aarch64", target_feature = "aes") => {
        /// AArch64 AES implementation
        #[expect(
            clippy::inline_modules,
            reason = "Small internal API, it is more readable this way"
        )]
        mod aarch64 {
            use core::arch::aarch64::{
                vaesdq_u8, vaesimcq_u8, vcombine_u64, vcreate_u64, vdupq_n_u8, vgetq_lane_u64,
                vreinterpretq_u8_u64, vreinterpretq_u64_u8,
            };

            /// `vaesdq_u8(state, zero)` computes XOR(zero) then InvShiftRows + InvSubBytes. ARM's
            /// AESD operates in the same byte order as the RISC-V half-state model when
            /// `(rs1, rs2)` is loaded little-endian; no swap needed.
            #[inline]
            #[target_feature(enable = "aes")]
            pub(super) fn aes64ds(rs1: u64, rs2: u64) -> u64 {
                let state = vreinterpretq_u8_u64(vcombine_u64(vcreate_u64(rs1), vcreate_u64(rs2)));
                let zero = vdupq_n_u8(0);
                let result = vaesdq_u8(state, zero);
                vgetq_lane_u64::<0>(vreinterpretq_u64_u8(result))
            }

            /// `vaesimcq_u8(vaesdq_u8(state, zero))` maps exactly to `aes64dsm`
            #[inline]
            #[target_feature(enable = "aes")]
            pub(super) fn aes64dsm(rs1: u64, rs2: u64) -> u64 {
                let state = vreinterpretq_u8_u64(vcombine_u64(vcreate_u64(rs1), vcreate_u64(rs2)));
                let zero = vdupq_n_u8(0);
                let after_sub_shift = vaesdq_u8(state, zero);
                let result = vaesimcq_u8(after_sub_shift);
                vgetq_lane_u64::<0>(vreinterpretq_u64_u8(result))
            }

            #[inline]
            #[target_feature(enable = "aes")]
            pub(super) fn aes64im(rs1: u64) -> u64 {
                let state = vreinterpretq_u8_u64(vcombine_u64(vcreate_u64(rs1), vcreate_u64(rs1)));
                let result = vaesimcq_u8(state);
                vgetq_lane_u64::<0>(vreinterpretq_u64_u8(result))
            }
        }
    }
    _ => {
        /// Software fallback for aes64ds, aes64dsm, aes64im
        #[expect(
            clippy::inline_modules,
            reason = "Small internal API, it is more readable this way"
        )]
        mod soft {
            use crate::rv32::zk::zkn::zknd::rv32_zknd_helpers::{INV_SBOX, gmul};

            #[inline(always)]
            fn inv_mix_col(col: u32) -> u32 {
                let s0 = col as u8;
                let s1 = (col >> 8u8) as u8;
                let s2 = (col >> 16u8) as u8;
                let s3 = (col >> 24u8) as u8;
                let r0 = gmul(s0, 0x0e) ^ gmul(s1, 0x0b) ^ gmul(s2, 0x0d) ^ gmul(s3, 0x09);
                let r1 = gmul(s0, 0x09) ^ gmul(s1, 0x0e) ^ gmul(s2, 0x0b) ^ gmul(s3, 0x0d);
                let r2 = gmul(s0, 0x0d) ^ gmul(s1, 0x09) ^ gmul(s2, 0x0e) ^ gmul(s3, 0x0b);
                let r3 = gmul(s0, 0x0b) ^ gmul(s1, 0x0d) ^ gmul(s2, 0x09) ^ gmul(s3, 0x0e);
                u32::from(r0) | (u32::from(r1) << 8u8) | (u32::from(r2) << 16u8) | (u32::from(r3) << 24u8)
            }

            /// Apply InvShiftRows + InvSubBytes to the full 128-bit state `(rs1, rs2)` and return
            /// the low 64-bit half of the result.
            ///
            /// State layout: column-major, little-endian 64-bit halves.
            /// `byte[col*4 + row]` is at bit `(row*8)` of `rs1` for `col < 2`, or bit `(row*8)` of
            /// `rs2` for `col >= 2`.
            ///
            /// InvShiftRows shifts row `r` right by `r` columns (cyclically over 4).
            /// Output low half contains post-transform columns 0 and 1.
            #[inline(always)]
            pub(super) fn aes64ds(rs1: u64, rs2: u64) -> u64 {
                let state_byte = |col: usize, row: usize| -> u8 {
                    let word = if col < 2 { rs1 } else { rs2 };
                    (word >> ((col % 2) * 32 + row * 8)) as u8
                };

                let mut out = 0;
                for c in 0..2usize {
                    for r in 0..4usize {
                        let src_col = (c + 4 - r) & 3;
                        let b = INV_SBOX[state_byte(src_col, r) as usize];
                        out |= u64::from(b) << (c * 32 + r * 8);
                    }
                }
                out
            }

            #[inline(always)]
            pub(super) fn aes64dsm(rs1: u64, rs2: u64) -> u64 {
                let lo = aes64ds(rs1, rs2);
                let col0 = inv_mix_col(lo as u32);
                let col1 = inv_mix_col((lo >> 32u8) as u32);
                u64::from(col0) | (u64::from(col1) << 32u8)
            }

            #[inline(always)]
            pub(super) fn aes64im(rs1: u64) -> u64 {
                let col0 = inv_mix_col(rs1 as u32);
                let col1 = inv_mix_col((rs1 >> 32u8) as u32);
                u64::from(col0) | (u64::from(col1) << 32u8)
            }
        }
    }
}

#[inline(always)]
#[doc(hidden)]
pub fn aes64ds(rs1: u64, rs2: u64) -> u64 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(
            not(miri),
            target_arch = "riscv64",
            target_feature = "zknd"
        ) => {
            // SAFETY: Compile-time checked for supported feature
            unsafe {
                core::arch::riscv64::aes64ds(rs1, rs2)
            }
        }
        all(target_arch = "x86_64", target_feature = "aes", target_feature = "sse4.1") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe {
                x86_64::aes64ds(rs1, rs2)
            }
        }
        all(target_arch = "aarch64", target_feature = "aes") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe {
                aarch64::aes64ds(rs1, rs2)
            }
        }
        _ => { soft::aes64ds(rs1, rs2) }
    }
}

#[inline(always)]
#[doc(hidden)]
pub fn aes64dsm(rs1: u64, rs2: u64) -> u64 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(
            not(miri),
            target_arch = "riscv64",
            target_feature = "zknd"
        ) => {
            // SAFETY: Compile-time checked for supported feature
            unsafe {
                core::arch::riscv64::aes64dsm(rs1, rs2)
            }
        }
        all(target_arch = "x86_64", target_feature = "aes", target_feature = "sse4.1") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe {
                x86_64::aes64dsm(rs1, rs2)
            }
        }
        all(target_arch = "aarch64", target_feature = "aes") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe {
                aarch64::aes64dsm(rs1, rs2)
            }
        }
        _ => { soft::aes64dsm(rs1, rs2) }
    }
}

#[inline(always)]
#[doc(hidden)]
pub fn aes64im(rs1: u64) -> u64 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(
            not(miri),
            target_arch = "riscv64",
            target_feature = "zknd"
        ) => {
            // SAFETY: Compile-time checked for supported feature
            unsafe {
                core::arch::riscv64::aes64im(rs1)
            }
        }
        all(target_arch = "x86_64", target_feature = "aes", target_feature = "sse4.1") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe {
                x86_64::aes64im(rs1)
            }
        }
        all(target_arch = "aarch64", target_feature = "aes") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe {
                aarch64::aes64im(rs1)
            }
        }
        _ => { soft::aes64im(rs1) }
    }
}

#[inline(always)]
#[doc(hidden)]
pub fn aes64ks1i(rs1: u64, rnum: Rv64ZkndKsRnum) -> u64 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(
            not(miri),
            target_arch = "riscv64",
            target_feature = "zknd"
        ) => {
            // SAFETY: Compile-time checked for supported feature
            unsafe {
                core::arch::riscv64::aes64ks1i(rs1, rnum as u8)
            }
        }
        _ => { ks::aes64ks1i(rs1, rnum) }
    }
}

#[inline(always)]
#[doc(hidden)]
pub fn aes64ks2(rs1: u64, rs2: u64) -> u64 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(
            not(miri),
            target_arch = "riscv64",
            target_feature = "zknd"
        ) => {
            // SAFETY: Compile-time checked for supported feature
            unsafe {
                core::arch::riscv64::aes64ks2(rs1, rs2)
            }
        }
        _ => { ks::aes64ks2(rs1, rs2) }
    }
}
