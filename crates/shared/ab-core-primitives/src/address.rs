//! Address-related primitives

use crate::shard::ShardIndex;
use ab_io_type::metadata::IoTypeMetadataKind;
use ab_io_type::trivial_type::TrivialType;
use bech32::primitives::decode::CheckedHrpstring;
use bech32::{Bech32m, ByteIterExt, Fe32IterExt, Hrp};
use core::cmp::Ordering;
use core::mem::MaybeUninit;
use core::ops::Deref;
use core::{fmt, ptr};
use derive_more::Deref;

/// Formatted address
#[derive(Copy, Clone)]
pub struct FormattedAddress {
    buffer:
        [u8; ShortHrp::MAX_HRP_LENGTH + FormattedAddress::MAX_ENCODING_WITHOUT_HRP_WITH_SEPARATOR],
    length: usize,
}

impl fmt::Debug for FormattedAddress {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(f)
    }
}

impl fmt::Display for FormattedAddress {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(f)
    }
}

impl Deref for FormattedAddress {
    type Target = str;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl FormattedAddress {
    const MAX_ENCODING_WITHOUT_HRP_NO_SEPARATOR: usize = 33;
    const MAX_ENCODING_WITHOUT_HRP_WITH_SEPARATOR: usize = 39;

    /// Get internal string representation
    #[inline(always)]
    pub const fn as_str(&self) -> &str {
        // SAFETY: Guaranteed by formatting constructor
        unsafe { str::from_utf8_unchecked(self.buffer.split_at_unchecked(self.length).0) }
    }
}

/// Short human-readable address part
#[derive(Debug, Copy, Clone, Eq, PartialEq, Deref)]
pub struct ShortHrp(Hrp);

impl ShortHrp {
    /// Maximum length of the human-readable part of the address
    pub const MAX_HRP_LENGTH: usize = 5;
    /// Mainnet human-readable part
    pub const MAINNET: Self = Self(Hrp::parse_unchecked("abc"));
    /// Testnet human-readable part
    pub const TESTNET: Self = Self(Hrp::parse_unchecked("xyz"));

    /// Create a new instance.
    ///
    /// Returns `None` if length of human-readable part is longer than [`Self::MAX_HRP_LENGTH`].
    // TODO: `const fn` once `bech32 > 0.12.0` is released
    pub fn new(hrp: Hrp) -> Option<Self> {
        if hrp.len() > Self::MAX_HRP_LENGTH {
            return None;
        }

        Some(Self(hrp))
    }
}

/// Logically the same as `u128`, but aligned to `8` bytes instead of `16`.
///
/// Byte layout is the same as `u128`, just alignment is different.
///
/// The first 20 bits correspond to the shard index (the least significant bits first), and the
/// remaining 108 bits (the most significant bits first) are an address allocated within that shard.
/// This way, an address will have a bunch of zeroes in the middle that is shrinking as more shards
/// are added and more addresses are allocated.
#[derive(Default, Copy, Clone, Eq, PartialEq, Hash)]
#[repr(C)]
pub struct Address(u64, u64);

// SAFETY: Any bit pattern is valid, so it is safe to implement `TrivialType` for this type
unsafe impl TrivialType for Address {
    const METADATA: &[u8] = &[IoTypeMetadataKind::Address as u8];
}

// Ensure this never mismatches with code in `ab-io-type` despite being in a different crate
const {
    let (type_details, _metadata) = IoTypeMetadataKind::type_details(Address::METADATA)
        .expect("Statically correct metadata; qed");
    assert!(size_of::<Address>() == type_details.recommended_capacity as usize);
    assert!(align_of::<Address>() == type_details.alignment.get() as usize);
}

impl fmt::Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Address").field(&u128::from(self)).finish()
    }
}

impl PartialEq<&Address> for Address {
    #[inline(always)]
    fn eq(&self, other: &&Address) -> bool {
        self.0 == other.0
    }
}

impl PartialEq<Address> for &Address {
    #[inline(always)]
    fn eq(&self, other: &Address) -> bool {
        self.0 == other.0
    }
}

impl Ord for Address {
    #[inline(always)]
    fn cmp(&self, other: &Address) -> Ordering {
        u128::from(self).cmp(&u128::from(other))
    }
}

impl PartialOrd for Address {
    #[inline(always)]
    fn partial_cmp(&self, other: &Address) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl const From<u128> for Address {
    #[inline(always)]
    fn from(value: u128) -> Self {
        let mut result = MaybeUninit::<Self>::uninit();
        // SAFETY: correct size, valid pointer, and all bits are valid
        unsafe {
            result.as_mut_ptr().cast::<u128>().write_unaligned(value);
            result.assume_init()
        }
    }
}

impl const From<&Address> for u128 {
    #[inline(always)]
    fn from(value: &Address) -> Self {
        // SAFETY: correct size, valid pointer, and all bits are valid
        unsafe { ptr::from_ref(value).cast::<u128>().read_unaligned() }
    }
}

impl const From<Address> for u128 {
    #[inline(always)]
    fn from(value: Address) -> Self {
        Self::from(&value)
    }
}
// TODO: Method for getting creation shard out of the address
// TODO: There should be a notion of global address
impl Address {
    // TODO: Various system contracts
    /// Sentinel contract address, inaccessible and not owned by anyone
    pub const NULL: Self = Self::from(0);
    /// System contract for managing code of other contracts
    pub const SYSTEM_CODE: Self = Self::from(1);
    /// System contract for managing block state
    pub const SYSTEM_BLOCK: Self = Self::from(2);
    /// System contract for managing state of other contracts
    pub const SYSTEM_STATE: Self = Self::from(3);
    /// System contract for native token
    pub const SYSTEM_NATIVE_TOKEN: Self = Self::from(4);
    /// System simple wallet base contract that can be used by end user wallets
    pub const SYSTEM_SIMPLE_WALLET_BASE: Self = Self::from(10);

    // Formatting-related constants
    const FORMAT_SEPARATOR_INTERVAL: [usize; 7] = [
        // `1` + shard ID
        1 + 4,
        4,
        3,
        4,
        4,
        3,
        4,
    ];
    const FORMAT_SEPARATOR: u8 = b'-';
    const FORMAT_ALL_ZEROES: u8 = b'q';
    const FORMAT_CHECKSUM_LENGTH: usize = 6;

    /// Parse address from a string formatted using [`Self::format()`].
    ///
    /// Returns `None` if the address is formatted incorrectly.
    pub fn parse(s: &str) -> Option<(ShortHrp, Self)> {
        let (hrp, other) = s.split_once('1')?;
        if hrp.len() > ShortHrp::MAX_HRP_LENGTH {
            return None;
        }

        let mut scratch = FormattedAddress {
            buffer: [Self::FORMAT_ALL_ZEROES; _],
            length: 0,
        };

        // Copy human-readable part + `1`
        scratch.buffer[..hrp.len() + 1].copy_from_slice(&s.as_bytes()[..hrp.len() + 1]);
        // Set length to full
        scratch.length = hrp.len() + FormattedAddress::MAX_ENCODING_WITHOUT_HRP_NO_SEPARATOR;

        let mut chunks = other.rsplit(char::from(Self::FORMAT_SEPARATOR));
        // Copy checksum into target location
        {
            let checksum = chunks.next()?;

            if checksum.len() != Self::FORMAT_CHECKSUM_LENGTH {
                return None;
            }
            scratch.buffer[..scratch.length][scratch.length - Self::FORMAT_CHECKSUM_LENGTH..]
                .copy_from_slice(checksum.as_bytes());
        }

        {
            let mut buffer = &mut scratch.buffer[..scratch.length - Self::FORMAT_CHECKSUM_LENGTH];
            let mut iterator = chunks
                .zip(Self::FORMAT_SEPARATOR_INTERVAL.into_iter().rev())
                .peekable();
            while let Some((chunk, max_chunk_length)) = iterator.next() {
                let chunk = chunk.as_bytes();

                if iterator.peek().is_none() {
                    // Finish with shard index
                    buffer[hrp.len() + 1..][..chunk.len()].copy_from_slice(chunk);
                    break;
                }

                if chunk.len() > max_chunk_length {
                    return None;
                }

                let target_chunk;
                (buffer, target_chunk) = buffer.split_at_mut(buffer.len() - max_chunk_length);

                target_chunk[max_chunk_length - chunk.len()..].copy_from_slice(chunk);
            }
        }

        let checked_hrp_string = CheckedHrpstring::new::<Bech32m>(&scratch).ok()?;
        let short_hrp = ShortHrp::new(checked_hrp_string.hrp())?;

        let mut address_bytes = 0u128.to_be_bytes();
        // Must decode the expected number of bytes
        {
            let mut address_bytes = address_bytes.as_mut_slice();
            for byte in checked_hrp_string.byte_iter() {
                if address_bytes.is_empty() {
                    return None;
                }

                address_bytes[0] = byte;

                address_bytes = &mut address_bytes[1..];
            }

            if !address_bytes.is_empty() {
                return None;
            }
        }
        let address = Address::from(u128::from_be_bytes(address_bytes));

        Some((short_hrp, address))
    }

    /// Format address for presentation purposes
    #[inline]
    pub fn format(&self, hrp: &ShortHrp) -> FormattedAddress {
        let mut scratch = FormattedAddress {
            buffer: [0; _],
            length: 0,
        };

        for char in u128::from(self)
            .to_be_bytes()
            .into_iter()
            .bytes_to_fes()
            .with_checksum::<Bech32m>(hrp)
            .bytes()
        {
            scratch.buffer[scratch.length] = char;
            scratch.length += 1;
        }

        let (prefix_with_shard, other) = scratch
            .as_str()
            .split_at(hrp.len() + Self::FORMAT_SEPARATOR_INTERVAL[0]);
        let (mut address_within_shard, checksum) =
            other.split_at(Self::FORMAT_SEPARATOR_INTERVAL[1..].iter().sum());

        let mut formatted_address = FormattedAddress {
            buffer: [0; _],
            length: 0,
        };

        // Shard index
        {
            let prefix_with_shard = prefix_with_shard
                .trim_end_matches(char::from(Self::FORMAT_ALL_ZEROES))
                .as_bytes();

            formatted_address.buffer[..prefix_with_shard.len()].copy_from_slice(prefix_with_shard);
            formatted_address.length = prefix_with_shard.len();

            formatted_address.buffer[prefix_with_shard.len()] = Self::FORMAT_SEPARATOR;
            formatted_address.length += 1;
        }
        // Address within shard
        {
            let mut finished_trimming = false;

            for &chunk_size in Self::FORMAT_SEPARATOR_INTERVAL[1..].iter() {
                let mut chunk;
                (chunk, address_within_shard) = address_within_shard.split_at(chunk_size);

                if !finished_trimming {
                    chunk = chunk.trim_start_matches(char::from(Self::FORMAT_ALL_ZEROES));

                    if chunk.is_empty() {
                        continue;
                    }

                    finished_trimming = true;
                }

                formatted_address.buffer[formatted_address.length..][..chunk.len()]
                    .copy_from_slice(chunk.as_bytes());
                formatted_address.length += chunk.len();

                formatted_address.buffer[formatted_address.length] = Self::FORMAT_SEPARATOR;
                formatted_address.length += 1;
            }
        }
        // Checksum
        {
            formatted_address.buffer[formatted_address.length..][..checksum.len()]
                .copy_from_slice(checksum.as_bytes());
            formatted_address.length += checksum.len();
        }

        formatted_address
    }

    /// System contract for address allocation on a particular shard index
    #[inline(always)]
    pub const fn system_address_allocator(shard_index: ShardIndex) -> Self {
        // Shard `0` doesn't have its own allocator because there are no user-deployable contracts
        // there, so address `0` is `NULL`, the rest up to `ShardIndex::MAX_SHARD_INDEX` correspond
        // to address allocators of respective shards
        Self::from(u128::from(u32::from(shard_index)).reverse_bits())
    }
}
