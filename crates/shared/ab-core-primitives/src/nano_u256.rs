#[cfg(test)]
mod tests;

use core::ops::Rem;

// Minimal `u256` implementation that is needed for sectors and optimized for producing the
// remainder of division by `u64`
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct NanoU256 {
    lo: u128,
    hi: u128,
}

impl NanoU256 {
    #[inline(always)]
    pub(crate) const fn from_le_bytes(bytes: [u8; 32]) -> Self {
        let lo = u128::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
        ]);
        let hi = u128::from_le_bytes([
            bytes[16], bytes[17], bytes[18], bytes[19], bytes[20], bytes[21], bytes[22], bytes[23],
            bytes[24], bytes[25], bytes[26], bytes[27], bytes[28], bytes[29], bytes[30], bytes[31],
        ]);

        Self { lo, hi }
    }
}

impl Rem<u64> for NanoU256 {
    type Output = u64;

    #[inline(always)]
    fn rem(self, rhs: u64) -> u64 {
        assert_ne!(rhs, 0, "division by zero");

        let rhs = u128::from(rhs);

        // If `hi` is 0, we can directly compute the remainder from `lo`
        if self.hi == 0 {
            return (self.lo % rhs) as u64;
        }

        // Process the high 128 bits first
        let hi_rem = self.hi % rhs;

        // Combine a high remainder with low 128 bits
        // hi_rem * 2^128 + lo
        let combined = (hi_rem << 64u8) | (self.lo >> 64u8);
        let combined_rem = combined % rhs;

        // Process the remaining low 64 bits
        let low = self.lo & 0xffff_ffff_ffff_ffff;
        let final_rem = ((combined_rem << 64u8) | low) % rhs;

        final_rem as u64
    }
}
