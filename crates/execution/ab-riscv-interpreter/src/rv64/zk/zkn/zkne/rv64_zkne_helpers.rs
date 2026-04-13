//! Opaque helpers for RV64 Zkne extension

cfg_select! {
    all(
        not(miri),
        target_arch = "riscv64",
        target_feature = "zkne"
    ) => {
        // Nothing, calling native intrinsics
    }
    all(target_arch = "x86_64", target_feature = "aes", target_feature = "sse4.1") => {
        /// x86-64 AES-NI implementation
        mod x86_64 {
            use core::arch::x86_64::{
                _mm_aesenclast_si128, _mm_aesenc_si128, _mm_extract_epi64,
                _mm_set_epi64x, _mm_setzero_si128,
            };

            /// `_mm_aesenclast_si128(state, zero)` computes ShiftRows + SubBytes then XORs with
            /// the round key. Zero key -> no-op XOR, matching `aes64es`.
            #[inline]
            #[target_feature(enable = "aes,sse4.1")]
            pub(super) fn aes64es(rs1: u64, rs2: u64) -> u64 {
                let state = _mm_set_epi64x(rs2.cast_signed(), rs1.cast_signed());
                let zero = _mm_setzero_si128();
                let result = _mm_aesenclast_si128(state, zero);
                _mm_extract_epi64::<0>(result).cast_unsigned()
            }

            /// `_mm_aesenc_si128(state, zero)` computes ShiftRows + SubBytes + MixColumns then
            /// XORs with the round key. Zero key -> no-op XOR, matching `aes64esm`.
            #[inline]
            #[target_feature(enable = "aes,sse4.1")]
            pub(super) fn aes64esm(rs1: u64, rs2: u64) -> u64 {
                let state = _mm_set_epi64x(rs2.cast_signed(), rs1.cast_signed());
                let zero = _mm_setzero_si128();
                let result = _mm_aesenc_si128(state, zero);
                _mm_extract_epi64::<0>(result).cast_unsigned()
            }
        }
    }
    all(target_arch = "aarch64", target_feature = "aes") => {
        /// AArch64 AES implementation
        ///
        /// AESE XORs the round key first, then applies SubBytes + ShiftRows (note: ARM ShiftRows
        /// direction matches the forward cipher). With a zero round key the XOR is a no-op,
        /// leaving pure SubBytes + ShiftRows - identical to what `aes64es` requires.
        mod aarch64 {
            use core::arch::aarch64::{
                vaeseq_u8, vaesmcq_u8, vcombine_u64, vcreate_u64, vdupq_n_u8, vgetq_lane_u64,
                vreinterpretq_u8_u64, vreinterpretq_u64_u8,
            };

            #[inline]
            #[target_feature(enable = "aes")]
            pub(super) fn aes64es(rs1: u64, rs2: u64) -> u64 {
                let state = vreinterpretq_u8_u64(vcombine_u64(vcreate_u64(rs1), vcreate_u64(rs2)));
                let zero = vdupq_n_u8(0);
                let result = vaeseq_u8(state, zero);
                vgetq_lane_u64::<0>(vreinterpretq_u64_u8(result))
            }

            /// `vaesmcq_u8(vaeseq_u8(state, zero))` maps exactly to `aes64esm`
            #[inline]
            #[target_feature(enable = "aes")]
            pub(super) fn aes64esm(rs1: u64, rs2: u64) -> u64 {
                let state = vreinterpretq_u8_u64(vcombine_u64(vcreate_u64(rs1), vcreate_u64(rs2)));
                let zero = vdupq_n_u8(0);
                let after_sub_shift = vaeseq_u8(state, zero);
                let result = vaesmcq_u8(after_sub_shift);
                vgetq_lane_u64::<0>(vreinterpretq_u64_u8(result))
            }
        }
    }
    _ => {
        /// Software fallback for aes64es, aes64esm
        mod soft {
            use crate::rv32::zk::zkn::zknd::rv32_zknd_helpers::{SBOX, gmul};

            #[inline(always)]
            fn mix_col(col: u32) -> u32 {
                let s0 = col as u8;
                let s1 = (col >> 8) as u8;
                let s2 = (col >> 16) as u8;
                let s3 = (col >> 24) as u8;
                let r0 = gmul(s0, 0x02) ^ gmul(s1, 0x03) ^ s2 ^ s3;
                let r1 = s0 ^ gmul(s1, 0x02) ^ gmul(s2, 0x03) ^ s3;
                let r2 = s0 ^ s1 ^ gmul(s2, 0x02) ^ gmul(s3, 0x03);
                let r3 = gmul(s0, 0x03) ^ s1 ^ s2 ^ gmul(s3, 0x02);
                (r0 as u32) | ((r1 as u32) << 8) | ((r2 as u32) << 16) | ((r3 as u32) << 24)
            }

            /// Apply ShiftRows + SubBytes to the full 128-bit state `(rs1, rs2)` and return the
            /// low 64-bit half of the result.
            ///
            /// State layout: column-major, little-endian 64-bit halves.
            /// ShiftRows shifts row `r` left by `r` columns (cyclically over 4).
            /// Output low half contains post-transform columns 0 and 1.
            #[inline(always)]
            pub(super) fn aes64es(rs1: u64, rs2: u64) -> u64 {
                let state_byte = |col: usize, row: usize| -> u8 {
                    let word = if col < 2 { rs1 } else { rs2 };
                    (word >> ((col % 2) * 32 + row * 8)) as u8
                };

                let mut out = 0;
                for c in 0..2usize {
                    for r in 0..4usize {
                        let src_col = (c + r) & 3;
                        let b = SBOX[state_byte(src_col, r) as usize];
                        out |= (b as u64) << (c * 32 + r * 8);
                    }
                }
                out
            }

            #[inline(always)]
            pub(super) fn aes64esm(rs1: u64, rs2: u64) -> u64 {
                let lo = aes64es(rs1, rs2);
                let col0 = mix_col(lo as u32);
                let col1 = mix_col((lo >> 32) as u32);
                (col0 as u64) | ((col1 as u64) << 32)
            }
        }
    }
}

#[inline(always)]
#[doc(hidden)]
pub fn aes64es(rs1: u64, rs2: u64) -> u64 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(
            not(miri),
            target_arch = "riscv64",
            target_feature = "zkne"
        ) => {
            // SAFETY: Compile-time checked for supported feature
            unsafe {
                core::arch::riscv64::aes64es(rs1, rs2)
            }
        }
        all(target_arch = "x86_64", target_feature = "aes", target_feature = "sse4.1") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe {
                x86_64::aes64es(rs1, rs2)
            }
        }
        all(target_arch = "aarch64", target_feature = "aes") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe {
                aarch64::aes64es(rs1, rs2)
            }
        }
        _ => { soft::aes64es(rs1, rs2) }
    }
}

#[inline(always)]
#[doc(hidden)]
pub fn aes64esm(rs1: u64, rs2: u64) -> u64 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(
            not(miri),
            target_arch = "riscv64",
            target_feature = "zkne"
        ) => {
            // SAFETY: Compile-time checked for supported feature
            unsafe {
                core::arch::riscv64::aes64esm(rs1, rs2)
            }
        }
        all(target_arch = "x86_64", target_feature = "aes", target_feature = "sse4.1") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe {
                x86_64::aes64esm(rs1, rs2)
            }
        }
        all(target_arch = "aarch64", target_feature = "aes") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe {
                aarch64::aes64esm(rs1, rs2)
            }
        }
        _ => { soft::aes64esm(rs1, rs2) }
    }
}
