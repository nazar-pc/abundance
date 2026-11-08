#[cfg(test)]
mod tests;

use crate::BlockVerificationError;
use ab_core_primitives::pot::SlotNumber;

/// Slots of a block relative to its parent block, see [`derive_block_slots()`]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) struct BlockSlots {
    /// Block's future slot: the block's slot plus the block authoring delay
    pub(crate) future_slot: SlotNumber,
    /// Parent block's future slot: the parent block's slot plus the block authoring delay, except
    /// for the genesis block (slot zero), whose future slot is its slot
    pub(crate) parent_future_slot: SlotNumber,
    /// Number of slots from the parent block's slot to the block's slot, never zero
    pub(crate) slots_between_blocks: SlotNumber,
}

/// Derive [`BlockSlots`] of a block at `slot` whose parent block is at `parent_slot`.
///
/// Returns [`BlockVerificationError::InvalidSlot`] if `slot` is not above `parent_slot` or its
/// future slot overflows. The rules are the same for all shard kinds.
pub(crate) fn derive_block_slots(
    block_authoring_delay: SlotNumber,
    parent_slot: SlotNumber,
    slot: SlotNumber,
) -> Result<BlockSlots, BlockVerificationError> {
    let slots_between_blocks = slot
        .checked_sub(parent_slot)
        .filter(|&slots_between_blocks| slots_between_blocks != SlotNumber::ZERO)
        .ok_or_else(|| BlockVerificationError::InvalidSlot { slot, parent_slot })?;
    let future_slot = slot
        .checked_add(block_authoring_delay)
        .ok_or_else(|| BlockVerificationError::InvalidSlot { slot, parent_slot })?;
    let parent_future_slot = if parent_slot == SlotNumber::ZERO {
        parent_slot
    } else {
        parent_slot
            .checked_add(block_authoring_delay)
            .ok_or_else(|| BlockVerificationError::InvalidSlot { slot, parent_slot })?
    };

    Ok(BlockSlots {
        future_slot,
        parent_future_slot,
        slots_between_blocks,
    })
}
