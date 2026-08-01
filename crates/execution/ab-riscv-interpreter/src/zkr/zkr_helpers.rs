//! Opaque helpers for Zkr extension

use crate::zkr::ZkrSeedPoll;
use ab_riscv_primitives::prelude::*;

/// Encode a [`ZkrSeedPoll`] into the raw bit layout of the `seed` CSR:
/// * bits `[31:30]`: `OPST`
/// * bits `[29:24]`: reserved, always zero
/// * bits `[23:16]`: custom, always zero (unused by this implementation)
/// * bits `[15:0]`: `entropy`, valid only when `OPST=ES16`
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub const fn encode_seed_poll<Reg>(poll: ZkrSeedPoll) -> Reg::Type
where
    Reg: [const] Register,
{
    match poll {
        ZkrSeedPoll::Bist => Reg::Type::from(0b00u8) << 30u8,
        ZkrSeedPoll::Wait => Reg::Type::from(0b01u8) << 30u8,
        ZkrSeedPoll::Es16(entropy) => (Reg::Type::from(0b10u8) << 30u8) | Reg::Type::from(entropy),
        ZkrSeedPoll::Dead => Reg::Type::from(0b11u8) << 30u8,
    }
}
