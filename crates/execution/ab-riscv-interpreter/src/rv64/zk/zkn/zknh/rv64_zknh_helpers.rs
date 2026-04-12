//! Opaque helpers for RV64 Zknh extension

#[inline(always)]
#[doc(hidden)]
pub fn sha256sig0(x: u32) -> u32 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv64", target_feature = "zknh") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe { core::arch::riscv64::sha256sig0(x) }
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
        all(not(miri), target_arch = "riscv64", target_feature = "zknh") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe { core::arch::riscv64::sha256sig1(x) }
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
        all(not(miri), target_arch = "riscv64", target_feature = "zknh") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe { core::arch::riscv64::sha256sum0(x) }
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
        all(not(miri), target_arch = "riscv64", target_feature = "zknh") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe { core::arch::riscv64::sha256sum1(x) }
        }
        _ => {
            x.rotate_right(6) ^ x.rotate_right(11) ^ x.rotate_right(25)
        }
    }
}

#[inline(always)]
#[doc(hidden)]
pub fn sha512sig0(x: u64) -> u64 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv64", target_feature = "zknh") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe { core::arch::riscv64::sha512sig0(x) }
        }
        _ => {
            x.rotate_right(1) ^ x.rotate_right(8) ^ (x >> 7)
        }
    }
}

#[inline(always)]
#[doc(hidden)]
pub fn sha512sig1(x: u64) -> u64 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv64", target_feature = "zknh") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe { core::arch::riscv64::sha512sig1(x) }
        }
        _ => {
            x.rotate_right(19) ^ x.rotate_right(61) ^ (x >> 6)
        }
    }
}

#[inline(always)]
#[doc(hidden)]
pub fn sha512sum0(x: u64) -> u64 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv64", target_feature = "zknh") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe { core::arch::riscv64::sha512sum0(x) }
        }
        _ => {
            x.rotate_right(28) ^ x.rotate_right(34) ^ x.rotate_right(39)
        }
    }
}

#[inline(always)]
#[doc(hidden)]
pub fn sha512sum1(x: u64) -> u64 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv64", target_feature = "zknh") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe { core::arch::riscv64::sha512sum1(x) }
        }
        _ => {
            x.rotate_right(14) ^ x.rotate_right(18) ^ x.rotate_right(41)
        }
    }
}
