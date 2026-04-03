//! Opaque helpers for RV32 Zknh extension

// Each function implements the exact 32-bit formula from the RISC-V scalar crypto specification
// (Volume I, Zknh section), expressed directly in terms of the rs1 and rs2 register values as
// the executor passes them.
//
// The spec uses asymmetric register conventions across the six instructions. For all six the
// 64-bit operand is assembled from the two source registers, but the high/low assignment
// differs by group:
//
//   sha512sig0l, sha512sig1l : rs1 = LOW 32 bits,  rs2 = HIGH 32 bits
//   sha512sig0h, sha512sig1h : rs1 = HIGH 32 bits, rs2 = LOW 32 bits
//   sha512sum0r, sha512sum1r : rs1 = LOW 32 bits,  rs2 = HIGH 32 bits
//
// For sum0r/sum1r the Sail pseudocode reads:
//   let x : bits(64) = X(rs2) @ X(rs1);   -- rs2 is the HIGH half, rs1 is the LOW half
//   rd = result[31..0];
//
// The _l and _sum*r instructions place the "primary" (low) operand half in rs1 and produce
// the low result half. The _h instructions place the high-operand half in rs1 and produce
// the high-result half.
//
// For pure rotation terms ROR64(x, n) the half formulas are (hi = high word, lo = low word):
//   n < 32:  .hi = (hi>>n) ^ (lo<<(32-n))   .lo = (lo>>n) ^ (hi<<(32-n))
//   n >= 32: .hi = (lo>>(n-32)) ^ (hi<<(64-n))  .lo = (hi>>(n-32)) ^ (lo<<(64-n))
//
// For SHR64(x, n) with n < 32 the low half picks up cross-boundary bits from hi:
//   .hi = hi>>n   (no lo contribution)
//   .lo = (lo>>n) ^ (hi<<(32-n))

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
    (rs1 >> 1) ^ (rs2 << 31) ^ (rs1 >> 8) ^ (rs2 << 24) ^ (rs1 >> 7)
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
    (rs1 >> 1) ^ (rs2 << 31) ^ (rs1 >> 8) ^ (rs2 << 24) ^ (rs1 >> 7) ^ (rs2 << 25)
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
    (rs1 >> 19) ^ (rs2 << 13) ^ (rs2 >> 29) ^ (rs1 << 3) ^ (rs1 >> 6)
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
    (rs1 >> 19) ^ (rs2 << 13) ^ (rs2 >> 29) ^ (rs1 << 3) ^ (rs1 >> 6) ^ (rs2 << 26)
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
    (rs1 >> 28) ^ (rs2 << 4) ^ (rs2 >> 2) ^ (rs1 << 30) ^ (rs2 >> 7) ^ (rs1 << 25)
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
    (rs1 >> 14) ^ (rs2 << 18) ^ (rs1 >> 18) ^ (rs2 << 14) ^ (rs2 >> 9) ^ (rs1 << 23)
}

/// Only here to prevent compiler warnings about unused `zknh_helpers` module.
#[doc(hidden)]
pub const PLACEHOLDER: () = ();
