//! Opaque helpers for RV32 Zknh extension

#[inline(always)]
#[doc(hidden)]
pub fn sha256sig0(x: u32) -> u32 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv32", target_feature = "zknh") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe { core::arch::riscv32::sha256sig0(x) }
        }
        _ => {
            x.rotate_right(7) ^ x.rotate_right(18) ^ (x >> 3)
        }
    }
}

#[inline(always)]
#[doc(hidden)]
pub fn sha256sig1(x: u32) -> u32 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv32", target_feature = "zknh") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe { core::arch::riscv32::sha256sig1(x) }
        }
        _ => {
            x.rotate_right(17) ^ x.rotate_right(19) ^ (x >> 10)
        }
    }
}

#[inline(always)]
#[doc(hidden)]
pub fn sha256sum0(x: u32) -> u32 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv32", target_feature = "zknh") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe { core::arch::riscv32::sha256sum0(x) }
        }
        _ => {
            x.rotate_right(2) ^ x.rotate_right(13) ^ x.rotate_right(22)
        }
    }
}

#[inline(always)]
#[doc(hidden)]
pub fn sha256sum1(x: u32) -> u32 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv32", target_feature = "zknh") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe { core::arch::riscv32::sha256sum1(x) }
        }
        _ => {
            x.rotate_right(6) ^ x.rotate_right(11) ^ x.rotate_right(25)
        }
    }
}

// SHA-512 sigma0: ROR64(x,1) ^ ROR64(x,8) ^ SHR64(x,7)

/// High 32 bits of SHA-512 sigma0. rs1 = HIGH word, rs2 = LOW word.
///
/// ```text
/// ROR64(x,1).hi  = (rs1>>1)  ^ (rs2<<31)
/// ROR64(x,8).hi  = (rs1>>8)  ^ (rs2<<24)
/// SHR64(x,7).hi  =  rs1>>7              <- shift: no rs2 contribution
/// ```
#[inline(always)]
#[doc(hidden)]
pub fn sha512sig0h(rs1: u32, rs2: u32) -> u32 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv32", target_feature = "zknh") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe { core::arch::riscv32::sha512sig0h(rs1, rs2) }
        }
        _ => {
            (rs1 >> 1) ^ (rs2 << 31) ^ (rs1 >> 8) ^ (rs2 << 24) ^ (rs1 >> 7)
        }
    }
}

/// Low 32 bits of SHA-512 sigma0. rs1 = LOW word, rs2 = HIGH word.
///
/// ```text
/// ROR64(x,1).lo  = (rs1>>1)  ^ (rs2<<31)
/// ROR64(x,8).lo  = (rs1>>8)  ^ (rs2<<24)
/// SHR64(x,7).lo  = (rs1>>7)  ^ (rs2<<25)  <- cross-boundary bits from hi
/// ```
#[inline(always)]
#[doc(hidden)]
pub fn sha512sig0l(rs1: u32, rs2: u32) -> u32 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv32", target_feature = "zknh") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe { core::arch::riscv32::sha512sig0l(rs1, rs2) }
        }
        _ => {
            (rs1 >> 1) ^ (rs2 << 31) ^ (rs1 >> 8) ^ (rs2 << 24) ^ (rs1 >> 7) ^ (rs2 << 25)
        }
    }
}

// SHA-512 sigma1: ROR64(x,19) ^ ROR64(x,61) ^ SHR64(x,6)

/// High 32 bits of SHA-512 sigma1. rs1 = HIGH word, rs2 = LOW word.
///
/// ```text
/// ROR64(x,19).hi = (rs1>>19) ^ (rs2<<13)
/// ROR64(x,61).hi = ROR64(x,32+29).hi = (rs2>>29) ^ (rs1<<3)
/// SHR64(x,6).hi  =  rs1>>6              <- shift: no rs2 contribution
/// ```
#[inline(always)]
#[doc(hidden)]
pub fn sha512sig1h(rs1: u32, rs2: u32) -> u32 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv32", target_feature = "zknh") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe { core::arch::riscv32::sha512sig1h(rs1, rs2) }
        }
        _ => {
            (rs1 >> 19) ^ (rs2 << 13) ^ (rs2 >> 29) ^ (rs1 << 3) ^ (rs1 >> 6)
        }
    }
}

/// Low 32 bits of SHA-512 sigma1. rs1 = LOW word, rs2 = HIGH word.
///
/// ```text
/// ROR64(x,19).lo = (rs1>>19) ^ (rs2<<13)
/// ROR64(x,61).lo = ROR64(x,32+29).lo = (rs2>>29) ^ (rs1<<3)
/// SHR64(x,6).lo  = (rs1>>6)  ^ (rs2<<26)  <- cross-boundary bits from hi
/// ```
#[inline(always)]
#[doc(hidden)]
pub fn sha512sig1l(rs1: u32, rs2: u32) -> u32 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv32", target_feature = "zknh") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe { core::arch::riscv32::sha512sig1l(rs1, rs2) }
        }
        _ => {
            (rs1 >> 19) ^ (rs2 << 13) ^ (rs2 >> 29) ^ (rs1 << 3) ^ (rs1 >> 6) ^ (rs2 << 26)
        }
    }
}

// SHA-512 Sum0: ROR64(x,28) ^ ROR64(x,34) ^ ROR64(x,39)
//
// Sail: let x = X(rs2) @ X(rs1)  =>  x[63:32] = rs2 (HIGH), x[31:0] = rs1 (LOW)
// sum0r produces the LOW half of the result.
//
// ROR64({hi=rs2, lo=rs1}, 28).lo  = (rs1>>28) ^ (rs2<<4)   [n=28 < 32]
// ROR64({hi=rs2, lo=rs1}, 34).lo  = (rs2>>2)  ^ (rs1<<30)  [n=34 = 32+2]
// ROR64({hi=rs2, lo=rs1}, 39).lo  = (rs2>>7)  ^ (rs1<<25)  [n=39 = 32+7]

/// Low 32 bits of SHA-512 Sum0. rs1 = LOW word, rs2 = HIGH word.
///
/// All three terms are rotations, so no asymmetric shift contribution.
///
/// ```text
/// ROR64(x,28).lo = (rs1>>28) ^ (rs2<<4)
/// ROR64(x,34).lo = ROR64(x,32+2).lo  = (rs2>>2)  ^ (rs1<<30)
/// ROR64(x,39).lo = ROR64(x,32+7).lo  = (rs2>>7)  ^ (rs1<<25)
/// ```
#[inline(always)]
#[doc(hidden)]
pub fn sha512sum0r(rs1: u32, rs2: u32) -> u32 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv32", target_feature = "zknh") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe { core::arch::riscv32::sha512sum0r(rs1, rs2) }
        }
        _ => {
            (rs1 >> 28) ^ (rs2 << 4) ^ (rs2 >> 2) ^ (rs1 << 30) ^ (rs2 >> 7) ^ (rs1 << 25)
        }
    }
}

// SHA-512 Sum1: ROR64(x,14) ^ ROR64(x,18) ^ ROR64(x,41)
//
// Sail: let x = X(rs2) @ X(rs1)  =>  x[63:32] = rs2 (HIGH), x[31:0] = rs1 (LOW)
// sum1r produces the LOW half of the result.
//
// ROR64({hi=rs2, lo=rs1}, 14).lo  = (rs1>>14) ^ (rs2<<18)  [n=14 < 32]
// ROR64({hi=rs2, lo=rs1}, 18).lo  = (rs1>>18) ^ (rs2<<14)  [n=18 < 32]
// ROR64({hi=rs2, lo=rs1}, 41).lo  = (rs2>>9)  ^ (rs1<<23)  [n=41 = 32+9]

/// Low 32 bits of SHA-512 Sum1. rs1 = LOW word, rs2 = HIGH word.
///
/// All three terms are rotations.
///
/// ```text
/// ROR64(x,14).lo = (rs1>>14) ^ (rs2<<18)
/// ROR64(x,18).lo = (rs1>>18) ^ (rs2<<14)
/// ROR64(x,41).lo = ROR64(x,32+9).lo = (rs2>>9)  ^ (rs1<<23)
/// ```
#[inline(always)]
#[doc(hidden)]
pub fn sha512sum1r(rs1: u32, rs2: u32) -> u32 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv32", target_feature = "zknh") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe { core::arch::riscv32::sha512sum1r(rs1, rs2) }
        }
        _ => {
            (rs1 >> 14) ^ (rs2 << 18) ^ (rs1 >> 18) ^ (rs2 << 14) ^ (rs2 >> 9) ^ (rs1 << 23)
        }
    }
}
