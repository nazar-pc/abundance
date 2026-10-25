#![expect(
    clippy::rest_pattern_accessible_field,
    reason = "Intentionally not needing other fields, too verbose otherwise"
)]
#![no_std]

#[cfg(test)]
mod tests;

extern crate alloc;

use ab_aligned_buffer::{OwnedAlignedBuffer, SharedAlignedBuffer};
use ab_core_primitives::address::Address;
use alloc::boxed::Box;
use core::mem::ManuallyDrop;
use replace_with::replace_with_or_abort;
use smallvec::SmallVec;
use tracing::debug;

/// Small number of elements to store without heap allocation in some data structures.
///
/// This is both large enough for many practical use cases and small enough to bring significant
/// performance improvement.
const INLINE_SIZE: usize = 8;
/// It should be rare that more than 2 contracts are created in the same transaction
const NEW_CONTRACTS_INLINE: usize = 2;

/// Key of the slot in [`Slots`]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct SlotKey {
    /// Owner of the slot
    pub owner: Address,
    /// Contract that manages the slot
    pub contract: Address,
}

/// Opaque slot index, used to identify a used slot [`Slots`]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct SlotIndex(usize);

impl From<SlotIndex> for usize {
    #[inline(always)]
    fn from(value: SlotIndex) -> Self {
        value.0
    }
}

#[derive(Debug, Clone)]
pub enum Slot {
    ReadOnly {
        key: SlotKey,
        buffer: SharedAlignedBuffer,
    },
    ReadWrite {
        key: SlotKey,
        buffer: SharedAlignedBuffer,
    },
}

impl Slot {
    fn is_null_contract(&self) -> bool {
        let slot_key = match self {
            Slot::ReadOnly { key, .. } => key,
            Slot::ReadWrite { key, .. } => key,
        };

        slot_key.contract == Address::NULL
    }
}

#[derive(Debug, Clone)]
enum SlotState {
    /// Original slot as given to the execution environment, not accessed yet
    Original(SharedAlignedBuffer),
    /// Original slot as given to the execution environment that is currently being accessed
    OriginalReadOnly(SharedAlignedBuffer),
    /// Previously modified slot
    Modified(SharedAlignedBuffer),
    /// Previously modified slot that is currently being accessed for reads
    ModifiedReadOnly(SharedAlignedBuffer),
    /// Original slot as given to the execution environment that is currently being modified
    OriginalReadWrite {
        buffer: OwnedAlignedBuffer,
        /// What it was in [`Self::Original`] before becoming [`Self::OriginalReadWrite`]
        previous: SharedAlignedBuffer,
    },
    /// Previously modified slot that is currently being modified
    ModifiedReadWrite {
        buffer: OwnedAlignedBuffer,
        /// What it was in [`Self::Modified`] before becoming [`Self::ModifiedReadWrite`]
        previous: SharedAlignedBuffer,
    },
}

#[derive(Debug, Clone)]
enum SlotAccessKind {
    /// Slot is currently accessed for reads
    ReadOnly,
    /// Slot is currently accessed for writes
    ReadWrite,
    /// Modification by nested instances that already integrated their changes into the parent,
    /// with the state of the slot from before the modification, such that the parent can roll it
    /// back
    Integrated { previous: SlotState },
}

#[derive(Debug, Clone)]
struct SlotAccess {
    slot_index: SlotIndex,
    kind: SlotAccessKind,
}

/// Find the active access of a slot: `Some(false)` for read-only and `Some(true)` for read-write.
///
/// A slot can have multiple accesses, but only the latest one can be active.
#[inline(always)]
fn active_access(slot_access: &[SlotAccess], slot_index: SlotIndex) -> Option<bool> {
    let slot_access = slot_access
        .iter()
        .rev()
        .find(|slot_access| slot_access.slot_index == slot_index)?;

    match slot_access.kind {
        SlotAccessKind::ReadOnly => Some(false),
        SlotAccessKind::ReadWrite => Some(true),
        SlotAccessKind::Integrated { .. } => None,
    }
}

#[derive(Debug, Clone)]
struct Inner {
    slots: SmallVec<[(SlotKey, SlotState); INLINE_SIZE]>,
    /// Accesses of slots, including those of nested instances that already integrated their
    /// changes into the parent, which may still need to roll them back
    slot_access: SmallVec<[SlotAccess; INLINE_SIZE]>,
    /// The list of new addresses that were created during transaction processing and couldn't be
    /// known beforehand.
    ///
    /// Addresses in this list are allowed to create slots for any owner, and other contacts are
    /// allowed to create slots owned by these addresses.
    new_contracts: SmallVec<[Address; NEW_CONTRACTS_INLINE]>,
}

/// Collection of slots, primarily for the execution environment
#[derive(Debug, Clone)]
pub struct Slots(Box<Inner>);

impl Slots {
    /// Create a new instance from a hashmap containing existing slots.
    ///
    /// Only slots that are present in the input can be modified. The only exception is slots for
    /// owners created during runtime and initialized with [`Self::add_new_contract()`].
    ///
    /// "Empty" slots must still have a value in the form of an empty [`SharedAlignedBuffer`].
    #[inline(always)]
    pub fn new<I>(slots: I) -> Self
    where
        I: IntoIterator<Item = Slot>,
    {
        let slots = slots
            .into_iter()
            .filter_map(|slot| {
                // `Address::NULL` is used for `#[tmp]` and is ephemeral. Reads and writes are
                // allowed for any owner, and they will all be thrown away after transaction
                // processing if finished.
                if slot.is_null_contract() {
                    return None;
                }

                Some(match slot {
                    Slot::ReadOnly { key, buffer } => {
                        // Make sure it can't be written to at all
                        (key, SlotState::OriginalReadOnly(buffer))
                    }
                    Slot::ReadWrite { key, buffer } => (key, SlotState::Original(buffer)),
                })
            })
            .collect();

        let inner = Inner {
            slots,
            slot_access: SmallVec::new(),
            new_contracts: SmallVec::new(),
        };

        Self(Box::new(inner))
    }

    /// Create a new read-write [`NestedSlots`] instance.
    ///
    /// Changes of the nested instance are reset when it is dropped, unless they are integrated into
    /// the parent with [`NestedSlots::commit()`] first.
    #[inline(always)]
    pub fn new_nested_rw(&mut self) -> NestedSlots<'_> {
        let parent_slots_len = self.0.slots.len();
        let parent_new_contracts_len = self.0.new_contracts.len();

        NestedSlots(NestedSlotsInner::ReadWrite {
            inner: &mut self.0,
            parent_slot_access_len: 0,
            parent_slots_len,
            parent_new_contracts_len,
            parent_slot_access_start: None,
        })
    }

    /// Create a new read-only [`NestedSlots`] instance
    #[inline(always)]
    pub fn new_nested_ro(&self) -> NestedSlots<'_> {
        NestedSlots(NestedSlotsInner::ReadOnly { inner: &self.0 })
    }

    /// Add a new contract that didn't exist before.
    ///
    /// In contrast to contracts in [`Self::new()`], this contract will be allowed to have any slots
    /// related to it being modified.
    ///
    /// Returns `false` if a contract already exits in a map, which is also considered as an access
    /// violation.
    #[must_use]
    #[inline(always)]
    pub fn add_new_contract(&mut self, owner: Address) -> bool {
        let new_contracts = &mut self.0.new_contracts;

        if new_contracts.contains(&owner) {
            debug!(?owner, "Not adding a new contract duplicate");
            return false;
        }

        new_contracts.push(owner);
        true
    }

    /// Iterate over all slots in the collection
    #[inline]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&SlotKey, &SharedAlignedBuffer)> + '_ {
        self.0.slots.iter().map(|(slot_key, slot)| match slot {
            SlotState::Original(buffer) => (slot_key, buffer),
            SlotState::OriginalReadOnly(_) => {
                unreachable!("Only original and modified slots can exist at the `Slots` level; qed")
            }
            SlotState::Modified(buffer) => (slot_key, buffer),
            SlotState::ModifiedReadOnly(_) => {
                unreachable!("Only original and modified slots can exist at the `Slots` level; qed")
            }
            SlotState::OriginalReadWrite { .. } => {
                unreachable!("Only original and modified slots can exist at the `Slots` level; qed")
            }
            SlotState::ModifiedReadWrite { .. } => {
                unreachable!("Only original and modified slots can exist at the `Slots` level; qed")
            }
        })
    }

    /// Iterate over modified slots in the collection
    #[inline]
    pub fn iter_modified(&self) -> impl Iterator<Item = (&SlotKey, &SharedAlignedBuffer)> + '_ {
        self.0
            .slots
            .iter()
            .filter_map(|(slot_key, slot)| match slot {
                SlotState::Original(_) => None,
                SlotState::OriginalReadOnly(_) => unreachable!(
                    "Only original and modified slots can exist at the `Slots` level; qed"
                ),
                SlotState::Modified(buffer) => Some((slot_key, buffer)),
                SlotState::ModifiedReadOnly(_) => unreachable!(
                    "Only original and modified slots can exist at the `Slots` level; qed"
                ),
                SlotState::OriginalReadWrite { .. } => unreachable!(
                    "Only original and modified slots can exist at the `Slots` level; qed"
                ),
                SlotState::ModifiedReadWrite { .. } => unreachable!(
                    "Only original and modified slots can exist at the `Slots` level; qed"
                ),
            })
    }

    /// Extract all slots in the collection
    #[inline]
    pub fn into_slots(self) -> impl ExactSizeIterator<Item = (SlotKey, SharedAlignedBuffer)> {
        self.0.slots.into_iter().map(|(slot_key, slot)| match slot {
            SlotState::Original(buffer) => (slot_key, buffer),
            SlotState::OriginalReadOnly(_) => {
                unreachable!("Only original and modified slots can exist at the `Slots` level; qed")
            }
            SlotState::Modified(buffer) => (slot_key, buffer),
            SlotState::ModifiedReadOnly(_) => {
                unreachable!("Only original and modified slots can exist at the `Slots` level; qed")
            }
            SlotState::OriginalReadWrite { .. } => {
                unreachable!("Only original and modified slots can exist at the `Slots` level; qed")
            }
            SlotState::ModifiedReadWrite { .. } => {
                unreachable!("Only original and modified slots can exist at the `Slots` level; qed")
            }
        })
    }
}

/// Container for `Slots` just to not expose this enum to the outside
#[derive(Debug)]
enum NestedSlotsInner<'a> {
    /// Similar to [`Self::Original`], but has a parent (another read-write instance or original)
    ReadWrite {
        inner: &'a mut Inner,
        parent_slot_access_len: usize,
        /// Slots added after this length are removed on reset
        parent_slots_len: usize,
        /// New contracts added after this length are removed on reset
        parent_new_contracts_len: usize,
        /// Accesses of the parent start at this index, `None` when the parent is [`Slots`]
        parent_slot_access_start: Option<usize>,
    },
    /// Read-only instance, non-exclusive access to [`Inner`], but not allowed to modify anything
    ReadOnly { inner: &'a Inner },
}

#[derive(Debug)]
pub struct NestedSlots<'a>(NestedSlotsInner<'a>);

impl Drop for NestedSlots<'_> {
    #[inline(always)]
    fn drop(&mut self) {
        // Read-only instances have nothing to reset. They are checked here, so that dropping them
        // doesn't call a cold function.
        if matches!(self.0, NestedSlotsInner::ReadWrite { .. }) {
            self.reset();
        }
    }
}

impl<'a> NestedSlots<'a> {
    #[inline(always)]
    fn inner_ro(&self) -> &Inner {
        match &self.0 {
            NestedSlotsInner::ReadWrite { inner, .. } => inner,
            NestedSlotsInner::ReadOnly { inner } => inner,
        }
    }

    #[inline(always)]
    fn inner_rw(&mut self) -> Option<&mut Inner> {
        match &mut self.0 {
            NestedSlotsInner::ReadWrite { inner, .. } => Some(inner),
            NestedSlotsInner::ReadOnly { .. } => None,
        }
    }

    /// Create a new read-write [`NestedSlots`] instance.
    ///
    /// Changes of the nested instance are reset when it is dropped, unless they are integrated into
    /// the parent with [`Self::commit()`] first.
    ///
    /// Returns `None` when attempted on a read-only instance.
    #[inline(always)]
    pub fn new_nested_rw<'b>(&'b mut self) -> Option<NestedSlots<'b>>
    where
        'a: 'b,
    {
        let (inner, parent_slot_access_start) = match &mut self.0 {
            NestedSlotsInner::ReadWrite {
                inner,
                parent_slot_access_len,
                ..
            } => (&mut **inner, *parent_slot_access_len),
            NestedSlotsInner::ReadOnly { .. } => {
                return None;
            }
        };

        let parent_slot_access_len = inner.slot_access.len();
        let parent_slots_len = inner.slots.len();
        let parent_new_contracts_len = inner.new_contracts.len();

        Some(NestedSlots(NestedSlotsInner::ReadWrite {
            inner,
            parent_slot_access_len,
            parent_slots_len,
            parent_new_contracts_len,
            parent_slot_access_start: Some(parent_slot_access_start),
        }))
    }

    /// Create a new nested read-only slots instance
    #[inline(always)]
    pub fn new_nested_ro<'b>(&'b self) -> NestedSlots<'b>
    where
        'a: 'b,
    {
        let inner = match &self.0 {
            NestedSlotsInner::ReadWrite { inner, .. } => &**inner,
            NestedSlotsInner::ReadOnly { inner } => &**inner,
        };

        NestedSlots(NestedSlotsInner::ReadOnly { inner })
    }

    /// Add a new contract that didn't exist before.
    ///
    /// In contrast to contracts in [`Slots::new()`], this contract will be allowed to have any
    /// slots related to it being modified.
    ///
    /// Returns `false` if a contract already exits in a map, which is also considered as an access
    /// violation.
    #[must_use]
    #[inline(always)]
    pub fn add_new_contract(&mut self, owner: Address) -> bool {
        let Some(inner) = self.inner_rw() else {
            debug!(?owner, "`add_new_contract` access violation");
            return false;
        };

        let new_contracts = &mut inner.new_contracts;

        if new_contracts.contains(&owner) {
            debug!(?owner, "Not adding a new contract duplicate");
            return false;
        }

        new_contracts.push(owner);
        true
    }

    /// Get code for `owner`.
    ///
    /// The biggest difference from [`Self::use_ro()`] is that the slot is not marked as used,
    /// instead the current code is cloned and returned.
    ///
    /// Returns `None` in case of access violation or if code is missing.
    #[inline(always)]
    pub fn get_code(&self, owner: Address) -> Option<SharedAlignedBuffer> {
        let result = self.get_code_internal(owner);

        if result.is_none() {
            debug!(?owner, "`get_code` access violation");
        }

        result
    }

    #[inline(always)]
    fn get_code_internal(&self, owner: Address) -> Option<SharedAlignedBuffer> {
        let inner = self.inner_ro();
        let slots = &inner.slots;
        let slot_access = &inner.slot_access;

        let contract = Address::SYSTEM_CODE;

        let slot_index = slots.iter().position(|(slot_key, _slot)| {
            slot_key.owner == owner && slot_key.contract == contract
        })?;
        let slot_index = SlotIndex(slot_index);

        // Ensure code is not currently being written to
        if active_access(slot_access, slot_index) == Some(true) {
            return None;
        }

        let buffer = match &slots
            .get(usize::from(slot_index))
            .expect("Just found; qed")
            .1
        {
            SlotState::Original(buffer)
            | SlotState::OriginalReadOnly(buffer)
            | SlotState::Modified(buffer)
            | SlotState::ModifiedReadOnly(buffer) => buffer,
            SlotState::OriginalReadWrite { .. } | SlotState::ModifiedReadWrite { .. } => {
                return None;
            }
        };

        Some(buffer.clone())
    }

    /// Read-only access to a slot with a specified owner and contract, marks it as used.
    ///
    /// Returns `None` in case of access violation.
    #[inline(always)]
    pub fn use_ro(&mut self, slot_key: SlotKey) -> Option<&SharedAlignedBuffer> {
        let inner_rw = match &mut self.0 {
            NestedSlotsInner::ReadWrite { inner, .. } => &mut **inner,
            NestedSlotsInner::ReadOnly { inner } => {
                // Simplified version that doesn't do access tracking
                let result = Self::use_ro_internal_read_only(
                    slot_key,
                    &inner.slots,
                    &inner.slot_access,
                    &inner.new_contracts,
                );

                if result.is_none() {
                    debug!(?slot_key, "`use_ro` access violation");
                }

                return result;
            }
        };

        let result = Self::use_ro_internal(
            slot_key,
            &mut inner_rw.slots,
            &mut inner_rw.slot_access,
            &inner_rw.new_contracts,
        );

        if result.is_none() {
            debug!(?slot_key, "`use_ro` access violation");
        }

        result
    }

    #[inline(always)]
    fn use_ro_internal<'b>(
        slot_key: SlotKey,
        slots: &'b mut SmallVec<[(SlotKey, SlotState); INLINE_SIZE]>,
        slot_access: &mut SmallVec<[SlotAccess; INLINE_SIZE]>,
        new_contracts: &[Address],
    ) -> Option<&'b SharedAlignedBuffer> {
        let maybe_slot_index = slots
            .iter()
            .position(|(slot_key_candidate, _slot)| slot_key_candidate == &slot_key)
            .map(SlotIndex);

        if let Some(slot_index) = maybe_slot_index {
            // Ensure that the slot is not currently being written to
            if let Some(read_write) = active_access(slot_access, slot_index) {
                if read_write {
                    return None;
                }
            } else {
                slot_access.push(SlotAccess {
                    slot_index,
                    kind: SlotAccessKind::ReadOnly,
                });
            }

            let slot = &mut slots
                .get_mut(usize::from(slot_index))
                .expect("Just found; qed")
                .1;

            // The slot that is currently being written to is not allowed for read access
            match slot {
                SlotState::Original(buffer) => {
                    let buffer = buffer.clone();
                    *slot = SlotState::OriginalReadOnly(buffer);
                    let SlotState::OriginalReadOnly(buffer) = slot else {
                        unreachable!("Just inserted; qed");
                    };
                    Some(buffer)
                }
                SlotState::OriginalReadOnly(buffer) | SlotState::ModifiedReadOnly(buffer) => {
                    Some(buffer)
                }
                SlotState::Modified(buffer) => {
                    let buffer = buffer.clone();
                    *slot = SlotState::ModifiedReadOnly(buffer);
                    let SlotState::ModifiedReadOnly(buffer) = slot else {
                        unreachable!("Just inserted; qed");
                    };
                    Some(buffer)
                }
                SlotState::OriginalReadWrite { .. } | SlotState::ModifiedReadWrite { .. } => None,
            }
        } else {
            // `Address::NULL` is used for `#[tmp]` and is ephemeral. Reads and writes are allowed
            // for any owner, and they will all be thrown away after transaction processing if
            // finished.
            if !(slot_key.contract == Address::NULL
                || new_contracts
                    .iter()
                    .any(|candidate| candidate == slot_key.owner || candidate == slot_key.contract))
            {
                return None;
            }

            slot_access.push(SlotAccess {
                slot_index: SlotIndex(slots.len()),
                kind: SlotAccessKind::ReadOnly,
            });

            let slot = SlotState::OriginalReadOnly(SharedAlignedBuffer::default());
            slots.push((slot_key, slot));
            let slot = &slots.last().expect("Just inserted; qed").1;
            let SlotState::OriginalReadOnly(buffer) = slot else {
                unreachable!("Just inserted; qed");
            };

            Some(buffer)
        }
    }

    /// Similar to [`Self::use_ro_internal()`], but for read-only instance
    #[inline(always)]
    fn use_ro_internal_read_only<'b>(
        slot_key: SlotKey,
        slots: &'b SmallVec<[(SlotKey, SlotState); INLINE_SIZE]>,
        slot_access: &SmallVec<[SlotAccess; INLINE_SIZE]>,
        new_contracts: &[Address],
    ) -> Option<&'b SharedAlignedBuffer> {
        let maybe_slot_index = slots
            .iter()
            .position(|(slot_key_candidate, _slot)| slot_key_candidate == &slot_key)
            .map(SlotIndex);

        if let Some(slot_index) = maybe_slot_index {
            // Ensure that the slot is not currently being written to
            if active_access(slot_access, slot_index) == Some(true) {
                return None;
            }

            let slot = &slots
                .get(usize::from(slot_index))
                .expect("Just found; qed")
                .1;

            // The slot that is currently being written to is not allowed for read access
            match slot {
                SlotState::Original(buffer)
                | SlotState::OriginalReadOnly(buffer)
                | SlotState::ModifiedReadOnly(buffer)
                | SlotState::Modified(buffer) => Some(buffer),
                SlotState::OriginalReadWrite { .. } | SlotState::ModifiedReadWrite { .. } => None,
            }
        } else {
            // `Address::NULL` is used for `#[tmp]` and is ephemeral. Reads and writes are
            // allowed for any owner, and they will all be thrown away after transaction
            // processing if finished.
            if !(slot_key.contract == Address::NULL
                || new_contracts
                    .iter()
                    .any(|candidate| candidate == slot_key.owner || candidate == slot_key.contract))
            {
                return None;
            }

            Some(SharedAlignedBuffer::empty_ref())
        }
    }

    /// Read-write access to a slot with a specified owner and contract, marks it as used.
    ///
    /// The returned slot is no longer accessible through [`Self::use_ro()`] or [`Self::use_rw()`]
    /// during the lifetime of this `NestedSlots` instance (and can be safely turned into a
    /// pointer). The only way to get another mutable reference is to call
    /// [`Self::access_used_rw()`].
    ///
    /// Returns `None` in case of access violation.
    #[inline(always)]
    pub fn use_rw(
        &mut self,
        slot_key: SlotKey,
        capacity: u32,
    ) -> Option<(SlotIndex, &mut OwnedAlignedBuffer)> {
        let inner = self.inner_rw()?;
        let slots = &mut inner.slots;
        let slot_access = &mut inner.slot_access;
        let new_contracts = &inner.new_contracts;

        let result = Self::use_rw_internal(slot_key, capacity, slots, slot_access, new_contracts);

        if result.is_none() {
            debug!(?slot_key, "`use_rw` access violation");
        }

        result
    }

    #[inline(always)]
    fn use_rw_internal<'b>(
        slot_key: SlotKey,
        capacity: u32,
        slots: &'b mut SmallVec<[(SlotKey, SlotState); INLINE_SIZE]>,
        slot_access: &mut SmallVec<[SlotAccess; INLINE_SIZE]>,
        new_contracts: &[Address],
    ) -> Option<(SlotIndex, &'b mut OwnedAlignedBuffer)> {
        let maybe_slot_index = slots
            .iter()
            .position(|(slot_key_candidate, _slot)| slot_key_candidate == &slot_key)
            .map(SlotIndex);

        if let Some(slot_index) = maybe_slot_index {
            // Ensure that slot is not accessed right now
            if active_access(slot_access, slot_index).is_some() {
                return None;
            }

            slot_access.push(SlotAccess {
                slot_index,
                kind: SlotAccessKind::ReadWrite,
            });

            let slot = &mut slots
                .get_mut(usize::from(slot_index))
                .expect("Just found; qed")
                .1;

            // The slot that is currently being accessed to is not allowed for writing
            let buffer = match slot {
                SlotState::OriginalReadOnly(_buffer) | SlotState::ModifiedReadOnly(_buffer) => {
                    return None;
                }
                SlotState::Original(buffer) => {
                    let mut new_buffer =
                        OwnedAlignedBuffer::with_capacity(capacity.max(buffer.len()));
                    new_buffer.copy_from_slice(buffer.as_slice());

                    *slot = SlotState::OriginalReadWrite {
                        buffer: new_buffer,
                        previous: buffer.clone(),
                    };
                    let SlotState::OriginalReadWrite { buffer, .. } = slot else {
                        unreachable!("Just inserted; qed");
                    };
                    buffer
                }
                SlotState::Modified(buffer) => {
                    let mut new_buffer =
                        OwnedAlignedBuffer::with_capacity(capacity.max(buffer.len()));
                    new_buffer.copy_from_slice(buffer.as_slice());

                    *slot = SlotState::ModifiedReadWrite {
                        buffer: new_buffer,
                        previous: buffer.clone(),
                    };
                    let SlotState::ModifiedReadWrite { buffer, .. } = slot else {
                        unreachable!("Just inserted; qed");
                    };
                    buffer
                }
                SlotState::OriginalReadWrite { buffer, .. }
                | SlotState::ModifiedReadWrite { buffer, .. } => {
                    buffer.ensure_capacity(capacity);
                    buffer
                }
            };

            Some((slot_index, buffer))
        } else {
            // `Address::NULL` is used for `#[tmp]` and is ephemeral. Reads and writes are allowed
            // for any owner, and they will all be thrown away after transaction processing if
            // finished.
            if !(slot_key.contract == Address::NULL
                || new_contracts
                    .iter()
                    .any(|candidate| candidate == slot_key.owner || candidate == slot_key.contract))
            {
                return None;
            }

            let slot_index = SlotIndex(slots.len());
            slot_access.push(SlotAccess {
                slot_index,
                kind: SlotAccessKind::ReadWrite,
            });

            let slot = SlotState::OriginalReadWrite {
                buffer: OwnedAlignedBuffer::with_capacity(capacity),
                previous: SharedAlignedBuffer::default(),
            };
            slots.push((slot_key, slot));
            let slot = &mut slots.last_mut().expect("Just inserted; qed").1;
            let SlotState::OriginalReadWrite { buffer, .. } = slot else {
                unreachable!("Just inserted; qed");
            };

            Some((slot_index, buffer))
        }
    }

    /// Read-write access to a slot with a specified owner and contract that is currently marked as
    /// used due to an earlier call to [`Self::use_rw()`].
    ///
    /// NOTE: Calling this method means that any pointers that might have been stored to the result
    /// of [`Self::use_rw()`] call are now invalid!
    ///
    /// Returns `None` in case of access violation.
    pub fn access_used_rw(&mut self, slot_index: SlotIndex) -> Option<&mut OwnedAlignedBuffer> {
        let maybe_slot = self
            .inner_rw()?
            .slots
            .get_mut(usize::from(slot_index))
            .map(|(_slot_key, slot)| slot);

        let Some(slot) = maybe_slot else {
            debug!(?slot_index, "`access_used_rw` access violation (not found)");
            return None;
        };

        // Must be currently accessed for writing
        match slot {
            SlotState::Original(_buffer)
            | SlotState::OriginalReadOnly(_buffer)
            | SlotState::Modified(_buffer)
            | SlotState::ModifiedReadOnly(_buffer) => {
                debug!(?slot_index, "`access_used_rw` access violation (read only)");
                None
            }
            SlotState::OriginalReadWrite { buffer, .. }
            | SlotState::ModifiedReadWrite { buffer, .. } => Some(buffer),
        }
    }

    /// Integrate changes into the parent.
    ///
    /// Without this, changes are reset when the instance is dropped.
    #[inline(always)]
    pub fn commit(self) {
        // Changes are integrated here instead of being reset on drop
        let mut this = ManuallyDrop::new(self);

        let (inner, parent_slot_access_len, parent_slot_access_start) = match &mut this.0 {
            NestedSlotsInner::ReadWrite {
                inner,
                parent_slot_access_len,
                parent_slot_access_start,
                ..
            } => (
                &mut **inner,
                *parent_slot_access_len,
                *parent_slot_access_start,
            ),
            NestedSlotsInner::ReadOnly { .. } => {
                // No need to integrate changes into the parent
                return;
            }
        };

        let slots = &mut inner.slots;
        let slot_access = &mut inner.slot_access;

        // Fix-up slots that were accessed by this instance. Accesses that modified a slot become
        // modifications with the previous state of the slot, such that the parent can roll them
        // back later.
        for slot_access in slot_access.iter_mut().skip(parent_slot_access_len) {
            if matches!(slot_access.kind, SlotAccessKind::Integrated { .. }) {
                // Modification by a nested instance that already integrated its changes
                continue;
            }

            let slot = &mut slots
                .get_mut(usize::from(slot_access.slot_index))
                .expect("Accessed slot exists; qed")
                .1;

            let mut previous = None;
            replace_with_or_abort(slot, |slot| match slot {
                SlotState::Original(_buffer) => {
                    unreachable!("Slot can't be in `Original` state after being accessed; qed")
                }
                SlotState::OriginalReadOnly(buffer) => SlotState::Original(buffer),
                SlotState::Modified(buffer) => SlotState::Modified(buffer),
                SlotState::ModifiedReadOnly(buffer) => SlotState::Modified(buffer),
                SlotState::OriginalReadWrite {
                    buffer,
                    previous: previous_buffer,
                } => {
                    previous = Some(SlotState::Original(previous_buffer));
                    SlotState::Modified(buffer.into_shared())
                }
                SlotState::ModifiedReadWrite {
                    buffer,
                    previous: previous_buffer,
                } => {
                    previous = Some(SlotState::Modified(previous_buffer));
                    SlotState::Modified(buffer.into_shared())
                }
            });
            if let Some(previous) = previous {
                slot_access.kind = SlotAccessKind::Integrated { previous };
            }
        }

        let Some(parent_slot_access_start) = parent_slot_access_start else {
            // There is no parent to roll back changes
            slot_access.clear();

            // Remove temporary values for `Address::NULL` contract, these are used as `#[tmp]`
            // "slots" by convention in the execution environment since there is no code behind
            // `Address::NULL` to possibly use it for anything
            slots.retain(|(slot_key, _slot)| slot_key.contract != Address::NULL);
            return;
        };

        // Keep only what the parent needs to roll back changes: one modification per slot, the
        // earliest one, which has the oldest state of the slot
        let accesses = slot_access
            .drain(parent_slot_access_len..)
            .collect::<SmallVec<[_; INLINE_SIZE]>>();
        for access in accesses {
            if !matches!(access.kind, SlotAccessKind::Integrated { .. }) {
                // Accesses that didn't modify the slot have nothing to roll back
                continue;
            }

            if slot_access
                .iter()
                .skip(parent_slot_access_start)
                .any(|slot_access| slot_access.slot_index == access.slot_index)
            {
                // The parent already has an earlier modification of the slot with an older state
                continue;
            }

            slot_access.push(access);
        }
    }

    /// Reset any changes that might have been done on this level, including changes of nested
    /// instances that already integrated them
    #[cold]
    fn reset(&mut self) {
        let (inner, parent_slot_access_len, parent_slots_len, parent_new_contracts_len) =
            match &mut self.0 {
                NestedSlotsInner::ReadWrite {
                    inner,
                    parent_slot_access_len,
                    parent_slots_len,
                    parent_new_contracts_len,
                    parent_slot_access_start: _,
                } => (
                    &mut **inner,
                    *parent_slot_access_len,
                    *parent_slots_len,
                    *parent_new_contracts_len,
                ),
                NestedSlotsInner::ReadOnly { .. } => {
                    // Read-only instances have nothing to reset
                    return;
                }
            };

        let slots = &mut inner.slots;
        let slot_access = &mut inner.slot_access;

        // Fix-up slots that were modified during access. Accesses are rolled back from the latest
        // to the earliest, such that each slot ends up in the state from before the earliest one.
        for slot_access in slot_access.drain(parent_slot_access_len..).rev() {
            let slot = &mut slots
                .get_mut(usize::from(slot_access.slot_index))
                .expect("Accessed slot exists; qed")
                .1;

            if let SlotAccessKind::Integrated { previous } = slot_access.kind {
                *slot = previous;
                continue;
            }

            replace_with_or_abort(slot, |slot| match slot {
                SlotState::Original(_buffer) => {
                    unreachable!("Slot can't be in `Original` state after being accessed; qed");
                }
                SlotState::OriginalReadOnly(buffer) => SlotState::Original(buffer),
                SlotState::Modified(buffer) => SlotState::Modified(buffer),
                SlotState::ModifiedReadOnly(buffer) => SlotState::Modified(buffer),
                SlotState::OriginalReadWrite { previous, .. } => SlotState::Original(previous),
                SlotState::ModifiedReadWrite { previous, .. } => SlotState::Modified(previous),
            });
        }

        // Slots and contracts that didn't exist before can't be accessed by the parent anymore
        slots.truncate(parent_slots_len);
        inner.new_contracts.truncate(parent_new_contracts_len);
    }
}
