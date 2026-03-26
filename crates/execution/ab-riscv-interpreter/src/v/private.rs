/// Trait that constrains supported ELEN + VLEN combinations.
///
/// `ELEN >= 8`, `ELEN <= VLEN`, `VLEN <= 65_536`, both must be a power of 2.
pub trait SupportedElenVlen<const ELEN: u32, const VLEN: u32> {}

impl<T> SupportedElenVlen<8, 8> for T {}
impl<T> SupportedElenVlen<8, 16> for T {}
impl<T> SupportedElenVlen<8, 32> for T {}
impl<T> SupportedElenVlen<8, 64> for T {}
impl<T> SupportedElenVlen<8, 128> for T {}
impl<T> SupportedElenVlen<8, 256> for T {}
impl<T> SupportedElenVlen<8, 512> for T {}
impl<T> SupportedElenVlen<8, 1024> for T {}
impl<T> SupportedElenVlen<8, 2048> for T {}
impl<T> SupportedElenVlen<8, 4096> for T {}
impl<T> SupportedElenVlen<8, 8192> for T {}
impl<T> SupportedElenVlen<8, 16_384> for T {}
impl<T> SupportedElenVlen<8, 32_768> for T {}
impl<T> SupportedElenVlen<8, 65_536> for T {}

impl<T> SupportedElenVlen<16, 16> for T {}
impl<T> SupportedElenVlen<16, 32> for T {}
impl<T> SupportedElenVlen<16, 64> for T {}
impl<T> SupportedElenVlen<16, 128> for T {}
impl<T> SupportedElenVlen<16, 256> for T {}
impl<T> SupportedElenVlen<16, 512> for T {}
impl<T> SupportedElenVlen<16, 1024> for T {}
impl<T> SupportedElenVlen<16, 2048> for T {}
impl<T> SupportedElenVlen<16, 4096> for T {}
impl<T> SupportedElenVlen<16, 8192> for T {}
impl<T> SupportedElenVlen<16, 16_384> for T {}
impl<T> SupportedElenVlen<16, 32_768> for T {}
impl<T> SupportedElenVlen<16, 65_536> for T {}

impl<T> SupportedElenVlen<32, 32> for T {}
impl<T> SupportedElenVlen<32, 64> for T {}
impl<T> SupportedElenVlen<32, 128> for T {}
impl<T> SupportedElenVlen<32, 256> for T {}
impl<T> SupportedElenVlen<32, 512> for T {}
impl<T> SupportedElenVlen<32, 1024> for T {}
impl<T> SupportedElenVlen<32, 2048> for T {}
impl<T> SupportedElenVlen<32, 4096> for T {}
impl<T> SupportedElenVlen<32, 8192> for T {}
impl<T> SupportedElenVlen<32, 16_384> for T {}
impl<T> SupportedElenVlen<32, 32_768> for T {}
impl<T> SupportedElenVlen<32, 65_536> for T {}

impl<T> SupportedElenVlen<64, 64> for T {}
impl<T> SupportedElenVlen<64, 128> for T {}
impl<T> SupportedElenVlen<64, 256> for T {}
impl<T> SupportedElenVlen<64, 512> for T {}
impl<T> SupportedElenVlen<64, 1024> for T {}
impl<T> SupportedElenVlen<64, 2048> for T {}
impl<T> SupportedElenVlen<64, 4096> for T {}
impl<T> SupportedElenVlen<64, 8192> for T {}
impl<T> SupportedElenVlen<64, 16_384> for T {}
impl<T> SupportedElenVlen<64, 32_768> for T {}
impl<T> SupportedElenVlen<64, 65_536> for T {}

impl<T> SupportedElenVlen<128, 128> for T {}
impl<T> SupportedElenVlen<128, 256> for T {}
impl<T> SupportedElenVlen<128, 512> for T {}
impl<T> SupportedElenVlen<128, 1024> for T {}
impl<T> SupportedElenVlen<128, 2048> for T {}
impl<T> SupportedElenVlen<128, 4096> for T {}
impl<T> SupportedElenVlen<128, 8192> for T {}
impl<T> SupportedElenVlen<128, 16_384> for T {}
impl<T> SupportedElenVlen<128, 32_768> for T {}
impl<T> SupportedElenVlen<128, 65_536> for T {}

impl<T> SupportedElenVlen<256, 256> for T {}
impl<T> SupportedElenVlen<256, 512> for T {}
impl<T> SupportedElenVlen<256, 1024> for T {}
impl<T> SupportedElenVlen<256, 2048> for T {}
impl<T> SupportedElenVlen<256, 4096> for T {}
impl<T> SupportedElenVlen<256, 8192> for T {}
impl<T> SupportedElenVlen<256, 16_384> for T {}
impl<T> SupportedElenVlen<256, 32_768> for T {}
impl<T> SupportedElenVlen<256, 65_536> for T {}

impl<T> SupportedElenVlen<512, 512> for T {}
impl<T> SupportedElenVlen<512, 1024> for T {}
impl<T> SupportedElenVlen<512, 2048> for T {}
impl<T> SupportedElenVlen<512, 4096> for T {}
impl<T> SupportedElenVlen<512, 8192> for T {}
impl<T> SupportedElenVlen<512, 16_384> for T {}
impl<T> SupportedElenVlen<512, 32_768> for T {}
impl<T> SupportedElenVlen<512, 65_536> for T {}

impl<T> SupportedElenVlen<1024, 1024> for T {}
impl<T> SupportedElenVlen<1024, 2048> for T {}
impl<T> SupportedElenVlen<1024, 4096> for T {}
impl<T> SupportedElenVlen<1024, 8192> for T {}
impl<T> SupportedElenVlen<1024, 16_384> for T {}
impl<T> SupportedElenVlen<1024, 32_768> for T {}
impl<T> SupportedElenVlen<1024, 65_536> for T {}

impl<T> SupportedElenVlen<2048, 2048> for T {}
impl<T> SupportedElenVlen<2048, 4096> for T {}
impl<T> SupportedElenVlen<2048, 8192> for T {}
impl<T> SupportedElenVlen<2048, 16_384> for T {}
impl<T> SupportedElenVlen<2048, 32_768> for T {}
impl<T> SupportedElenVlen<2048, 65_536> for T {}

impl<T> SupportedElenVlen<4096, 4096> for T {}
impl<T> SupportedElenVlen<4096, 8192> for T {}
impl<T> SupportedElenVlen<4096, 16_384> for T {}
impl<T> SupportedElenVlen<4096, 32_768> for T {}
impl<T> SupportedElenVlen<4096, 65_536> for T {}

impl<T> SupportedElenVlen<8192, 8192> for T {}
impl<T> SupportedElenVlen<8192, 16_384> for T {}
impl<T> SupportedElenVlen<8192, 32_768> for T {}
impl<T> SupportedElenVlen<8192, 65_536> for T {}

impl<T> SupportedElenVlen<16_384, 16_384> for T {}
impl<T> SupportedElenVlen<16_384, 32_768> for T {}
impl<T> SupportedElenVlen<16_384, 65_536> for T {}

impl<T> SupportedElenVlen<32_768, 32_768> for T {}
impl<T> SupportedElenVlen<32_768, 65_536> for T {}

impl<T> SupportedElenVlen<65_536, 65_536> for T {}
