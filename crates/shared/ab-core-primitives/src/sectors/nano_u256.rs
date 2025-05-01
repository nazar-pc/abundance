use core::ops::Rem;

// Minimal `u256` implementation that is needed for sectors and optimized for producing the
// remainder of division by `u64`
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct NanoU256 {
    lo: u128,
    hi: u128,
}

impl NanoU256 {
    #[inline(always)]
    pub(super) const fn from_le_bytes(bytes: [u8; 32]) -> Self {
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
        if rhs == 0 {
            panic!("division by zero");
        }

        let rhs = rhs as u128;

        // If `hi` is 0, we can directly compute the remainder from `lo`
        if self.hi == 0 {
            return (self.lo % rhs) as u64;
        }

        // Process the high 128 bits first
        let hi_rem = self.hi % rhs;

        // Combine a high remainder with low 128 bits
        // hi_rem * 2^128 + lo
        let combined = (hi_rem << 64) | (self.lo >> 64);
        let combined_rem = combined % rhs;

        // Process the remaining low 64 bits
        let low = self.lo & 0xffffffffffffffff;
        let final_rem = ((combined_rem << 64) | low) % rhs;

        final_rem as u64
    }
}

#[cfg(test)]
mod tests {
    use super::NanoU256;
    use crate::hashes::blake3_hash;

    #[test]
    fn basic() {
        let input = NanoU256::from_le_bytes(*blake3_hash(&[1, 2, 3]));
        let vectors = [
            (749265838, 96295755),
            (4294967296, 468481969),
            (9588891412677391755, 5746309610232603432),
            (3050220159935725727, 1594047135082657684),
            (9163698234407261922, 137811727784537481),
            (8110910621974504463, 772103028532207994),
            (10066003301207900840, 3710011681387425537),
            (6326525170861459176, 4054803448573033593),
            (16971852880362673803, 14857223653279674036),
            (5479364763909636908, 2217175580314974257),
            (14850578606861073142, 5959274540802056661),
            (6477421758110557520, 2913281736886846177),
            (u64::MAX, 11641615165612301982),
        ];

        for (n, rem) in vectors {
            assert_eq!(input % n, rem);
        }
    }

    #[test]
    #[should_panic]
    fn no_division_by_zero() {
        let input = NanoU256::from_le_bytes(*blake3_hash(&[1, 2, 3]));
        let _ = input % 0;
    }
}
