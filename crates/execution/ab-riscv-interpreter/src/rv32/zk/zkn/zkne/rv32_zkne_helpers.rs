//! Opaque helpers for RV32 Zkne extension

use ab_riscv_primitives::instructions::rv32::zk::zkn::zknd::Rv32AesBs;

/// Software fallback for aes32esi and aes32esmi.
///
/// Both instructions share the same S-box and MixColumn machinery; the only difference is whether
/// forward MixColumns is applied.
#[cfg(not(all(not(miri), target_arch = "riscv32", target_feature = "zkne")))]
pub(in super::super) mod soft {
    use crate::rv32::zk::zkn::zknd::rv32_zknd_helpers::{SBOX, gmul};

    /// Compute the partial forward MixColumns contribution for a single substituted byte `b`.
    ///
    /// This is `aes_mixcolumn_byte_fwd` from the Sail reference:
    /// the four output bytes of MixColumns when the input column has `b` in one position and zeros
    /// elsewhere - packed into a little-endian `u32`.
    ///
    /// Column matrix multiply for MixColumns:
    /// ```text
    /// r0 = 0x02*b
    /// r1 = 0x01*b
    /// r2 = 0x01*b
    /// r3 = 0x03*b
    /// ```
    #[inline(always)]
    pub(super) fn mix_col_byte(b: u8) -> u32 {
        let r0 = u32::from(gmul(b, 0x02));
        let r1 = u32::from(b);
        let r2 = u32::from(b);
        let r3 = u32::from(gmul(b, 0x03));
        r0 | (r1 << 8) | (r2 << 16) | (r3 << 24)
    }

    /// `aes32esi rs1, rs2, bs`
    ///
    /// Pseudocode:
    /// ```text
    /// shamt = bs * 8
    /// si    = (rs2 >> shamt) & 0xff
    /// so    = SBOX[si] as u32
    /// rd    = rs1 ^ rol32(so, shamt)
    /// ```
    #[inline(always)]
    pub(super) fn aes32esi(rs1: u32, rs2: u32, bs: u8) -> u32 {
        let shamt = u32::from(bs) * 8;
        let si = ((rs2 >> shamt) & 0xff) as u8;
        let so = u32::from(SBOX[usize::from(si)]);
        rs1 ^ so.rotate_left(shamt)
    }

    /// `aes32esmi rs1, rs2, bs`
    ///
    /// Pseudocode:
    /// ```text
    /// shamt = bs * 8
    /// si    = (rs2 >> shamt) & 0xff
    /// so    = SBOX[si]
    /// mixed = mix_col_byte(so)
    /// rd    = rs1 ^ rol32(mixed, shamt)
    /// ```
    #[inline(always)]
    pub(super) fn aes32esmi(rs1: u32, rs2: u32, bs: u8) -> u32 {
        let shamt = u32::from(bs) * 8;
        let si = ((rs2 >> shamt) & 0xff) as u8;
        let so = SBOX[usize::from(si)];
        let mixed = mix_col_byte(so);
        rs1 ^ mixed.rotate_left(shamt)
    }
}

#[inline(always)]
#[doc(hidden)]
pub fn aes32esi(rs1: u32, rs2: u32, bs: Rv32AesBs) -> u32 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(
            not(miri),
            target_arch = "riscv32",
            target_feature = "zkne"
        ) => {
            // SAFETY: Compile-time checked for supported feature
            unsafe {
                core::arch::riscv32::aes32esi(rs1, rs2, u8::from(bs))
            }
        }
        _ => { soft::aes32esi(rs1, rs2, u8::from(bs)) }
    }
}

#[inline(always)]
#[doc(hidden)]
pub fn aes32esmi(rs1: u32, rs2: u32, bs: Rv32AesBs) -> u32 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(
            not(miri),
            target_arch = "riscv32",
            target_feature = "zkne"
        ) => {
            // SAFETY: Compile-time checked for supported feature
            unsafe {
                core::arch::riscv32::aes32esmi(rs1, rs2, u8::from(bs))
            }
        }
        _ => { soft::aes32esmi(rs1, rs2, u8::from(bs)) }
    }
}
