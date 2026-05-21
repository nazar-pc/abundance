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

#[cfg(test)]
mod tests {
    use super::NanoU256;

    #[test]
    fn basic() {
        let input = NanoU256::from_le_bytes(blake3::hash(&[1, 2, 3]).into());
        let vectors = [
            (749_265_838, 96_295_755),
            (4_294_967_296, 468_481_969),
            (9_588_891_412_677_391_755, 5_746_309_610_232_603_432),
            (3_050_220_159_935_725_727, 1_594_047_135_082_657_684),
            (9_163_698_234_407_261_922, 137_811_727_784_537_481),
            (8_110_910_621_974_504_463, 772_103_028_532_207_994),
            (10_066_003_301_207_900_840, 3_710_011_681_387_425_537),
            (6_326_525_170_861_459_176, 4_054_803_448_573_033_593),
            (16_971_852_880_362_673_803, 14_857_223_653_279_674_036),
            (5_479_364_763_909_636_908, 2_217_175_580_314_974_257),
            (14_850_578_606_861_073_142, 5_959_274_540_802_056_661),
            (6_477_421_758_110_557_520, 2_913_281_736_886_846_177),
            (u64::MAX, 11_641_615_165_612_301_982),
        ];

        for (n, rem) in vectors {
            assert_eq!(input % n, rem);
        }
    }

    #[test]
    #[should_panic]
    fn no_division_by_zero() {
        let input = NanoU256::from_le_bytes(blake3::hash(&[1, 2, 3]).into());
        let _: u64 = input % 0;
    }
}
