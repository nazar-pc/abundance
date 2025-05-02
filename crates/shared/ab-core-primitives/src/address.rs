//! Address-related primitives

use crate::shard::ShardIndex;
use ab_io_type::metadata::IoTypeMetadataKind;
use ab_io_type::trivial_type::TrivialType;
use core::cmp::Ordering;
use core::mem::MaybeUninit;
use core::{fmt, ptr};

/// Logically the same as `u128`, but aligned to `8` bytes instead of `16`.
///
/// Byte layout is the same as `u128`, just alignment is different
#[derive(Default, Copy, Clone, Eq, PartialEq, Hash)]
#[repr(C)]
pub struct Address(u64, u64);

unsafe impl TrivialType for Address {
    const METADATA: &[u8] = &[IoTypeMetadataKind::Address as u8];
}

// Ensure this never mismatches with code in `ab-io-type` despite being in different crate
const _: () = {
    let (type_details, _metadata) = IoTypeMetadataKind::type_details(Address::METADATA)
        .expect("Statically correct metadata; qed");
    assert!(size_of::<Address>() == type_details.recommended_capacity as usize);
    assert!(align_of::<Address>() == type_details.alignment.get() as usize);
};

impl fmt::Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Address").field(&self.as_u128()).finish()
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // TODO: Human-readable formatting rather than a huge number
        self.as_u128().fmt(f)
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
        self.as_u128().cmp(&other.as_u128())
    }
}

impl PartialOrd for Address {
    #[inline(always)]
    fn partial_cmp(&self, other: &Address) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl From<u128> for Address {
    #[inline(always)]
    fn from(value: u128) -> Self {
        Self::new(value)
    }
}

impl From<Address> for u128 {
    #[inline(always)]
    fn from(value: Address) -> Self {
        value.as_u128()
    }
}

// TODO: Method for getting creation shard out of the address
// TODO: There should be a notion of global address
impl Address {
    // TODO: Various system contracts
    /// Sentinel contract address, inaccessible and not owned by anyone
    pub const NULL: Self = Self::new(0);
    /// System contract for managing code of other contracts
    pub const SYSTEM_CODE: Self = Self::new(1);
    /// System contract for managing block state
    pub const SYSTEM_BLOCK: Self = Self::new(2);
    /// System contract for managing state of other contracts
    pub const SYSTEM_STATE: Self = Self::new(3);
    /// System contract for native token
    pub const SYSTEM_NATIVE_TOKEN: Self = Self::new(4);
    /// System simple wallet base contract that can be used by end user wallets
    pub const SYSTEM_SIMPLE_WALLET_BASE: Self = Self::new(10);

    /// Create a value from `u128`
    #[inline(always)]
    const fn new(n: u128) -> Self {
        let mut result = MaybeUninit::<Self>::uninit();
        // SAFETY: correct size, valid pointer, and all bits are valid
        unsafe {
            result.as_mut_ptr().cast::<u128>().write_unaligned(n);
            result.assume_init()
        }
    }

    /// Turn value into `u128`
    #[inline(always)]
    const fn as_u128(self) -> u128 {
        // SAFETY: correct size, valid pointer, and all bits are valid
        unsafe { ptr::from_ref(&self).cast::<u128>().read_unaligned() }
    }

    /// System contract for address allocation on a particular shard index
    #[inline(always)]
    pub const fn system_address_allocator(shard_index: ShardIndex) -> Self {
        // Shard `0` doesn't have its own allocator because there are no user-deployable contracts
        // there, so address `0` is `NULL`, the rest up to `ShardIndex::MAX_SHARD_INDEX` correspond
        // to address allocators of respective shards
        Self::new(shard_index.as_u32() as u128 * ShardIndex::MAX_ADDRESSES_PER_SHARD.get())
    }
}
