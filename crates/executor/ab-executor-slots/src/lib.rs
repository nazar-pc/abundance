#![no_std]

extern crate alloc;

use ab_aligned_buffer::{OwnedAlignedBuffer, SharedAlignedBuffer};
use ab_contracts_common::Address;
use alloc::boxed::Box;
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

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct SlotIndex(usize);

impl From<SlotIndex> for usize {
    #[inline(always)]
    fn from(value: SlotIndex) -> Self {
        value.0
    }
}

/// Slot entry
#[derive(Debug, Clone)]
pub enum Slot {
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

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
struct SlotAccess {
    slot_index: SlotIndex,
    /// `false` for read-only and `true` for read-write
    read_write: bool,
}

#[derive(Debug, Clone)]
struct Inner {
    slots: SmallVec<[(SlotKey, Slot); INLINE_SIZE]>,
    slot_access: SmallVec<[SlotAccess; INLINE_SIZE]>,
    /// The list of new addresses that were created during transaction processing and couldn't be
    /// known beforehand.
    ///
    /// Addresses in this list are allowed to create slots for any owner, and other contacts are
    /// allowed to create slots owned by these addresses.
    new_contracts: SmallVec<[Address; NEW_CONTRACTS_INLINE]>,
}

/// Collection of slots, primarily for execution environment
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
        I: IntoIterator<Item = (SlotKey, SharedAlignedBuffer)>,
    {
        let slots = slots
            .into_iter()
            .filter_map(|(slot_key, slot)| {
                // `Address::NULL` is used for `#[tmp]` and is ephemeral. Reads and writes are
                // allowed for any owner, and they will all be thrown away after transaction
                // processing if finished.
                if slot_key.contract == Address::NULL {
                    return None;
                }

                Some((slot_key, Slot::Original(slot)))
            })
            .collect();

        let inner = Inner {
            slots,
            slot_access: SmallVec::new(),
            new_contracts: SmallVec::new(),
        };

        Self(Box::new(inner))
    }

    /// Create a new nested read-write slots instance.
    ///
    /// Nested instance will integrate its changes into the parent slot when dropped (or changes can
    /// be reset with [`NestedSlots::reset()`]).
    #[inline(always)]
    pub fn new_nested_rw(&mut self) -> NestedSlots<'_> {
        NestedSlots(NestedSlotsInner::ReadWrite {
            inner: &mut self.0,
            parent_slot_access_len: 0,
            original_parent: true,
        })
    }

    /// Create a new nested read-only slots instance
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
            debug!(%owner, "Not adding new contract duplicate");
            return false;
        }

        new_contracts.push(owner);
        true
    }

    /// Iterate over all slots in the collection
    #[inline]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&SlotKey, &SharedAlignedBuffer)> + '_ {
        self.0.slots.iter().map(|(slot_key, slot)| match slot {
            Slot::Original(buffer) => (slot_key, buffer),
            Slot::OriginalReadOnly(_) => {
                unreachable!("Only original and modified slots can exist at the `Slots` level; qed")
            }
            Slot::Modified(buffer) => (slot_key, buffer),
            Slot::ModifiedReadOnly(_) => {
                unreachable!("Only original and modified slots can exist at the `Slots` level; qed")
            }
            Slot::OriginalReadWrite { .. } => {
                unreachable!("Only original and modified slots can exist at the `Slots` level; qed")
            }
            Slot::ModifiedReadWrite { .. } => {
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
                Slot::Original(_) => None,
                Slot::OriginalReadOnly(_) => unreachable!(
                    "Only original and modified slots can exist at the `Slots` level; qed"
                ),
                Slot::Modified(buffer) => Some((slot_key, buffer)),
                Slot::ModifiedReadOnly(_) => unreachable!(
                    "Only original and modified slots can exist at the `Slots` level; qed"
                ),
                Slot::OriginalReadWrite { .. } => unreachable!(
                    "Only original and modified slots can exist at the `Slots` level; qed"
                ),
                Slot::ModifiedReadWrite { .. } => unreachable!(
                    "Only original and modified slots can exist at the `Slots` level; qed"
                ),
            })
    }

    /// Extract all slots in the collection
    #[inline]
    pub fn into_slots(self) -> impl ExactSizeIterator<Item = (SlotKey, SharedAlignedBuffer)> {
        self.0.slots.into_iter().map(|(slot_key, slot)| match slot {
            Slot::Original(buffer) => (slot_key, buffer),
            Slot::OriginalReadOnly(_) => {
                unreachable!("Only original and modified slots can exist at the `Slots` level; qed")
            }
            Slot::Modified(buffer) => (slot_key, buffer),
            Slot::ModifiedReadOnly(_) => {
                unreachable!("Only original and modified slots can exist at the `Slots` level; qed")
            }
            Slot::OriginalReadWrite { .. } => {
                unreachable!("Only original and modified slots can exist at the `Slots` level; qed")
            }
            Slot::ModifiedReadWrite { .. } => {
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
        original_parent: bool,
    },
    /// Read-only instance, non-exclusive access to [`Inner`], but not allowed to modify anything
    ReadOnly { inner: &'a Inner },
}

#[derive(Debug)]
pub struct NestedSlots<'a>(NestedSlotsInner<'a>);

impl<'a> Drop for NestedSlots<'a> {
    #[inline(always)]
    fn drop(&mut self) {
        let (inner, parent_slot_access_len, original_parent) = match &mut self.0 {
            NestedSlotsInner::ReadWrite {
                inner,
                parent_slot_access_len,
                original_parent,
            } => (&mut **inner, *parent_slot_access_len, *original_parent),
            NestedSlotsInner::ReadOnly { .. } => {
                // No need to integrate changes into the parent
                return;
            }
        };

        let slots = &mut inner.slots;
        let slot_access = &mut inner.slot_access;

        // Fix-up slots that were modified during access
        for slot_access in slot_access.drain(parent_slot_access_len..) {
            let slot = &mut slots
                .get_mut(usize::from(slot_access.slot_index))
                .expect("Accessed slot exists; qed")
                .1;

            take_mut::take(slot, |slot| match slot {
                Slot::Original(_buffer) => {
                    unreachable!("Slot can't be in Original state after being accessed")
                }
                Slot::OriginalReadOnly(buffer) => Slot::Original(buffer),
                Slot::Modified(buffer) => Slot::Modified(buffer),
                Slot::ModifiedReadOnly(buffer) => Slot::Modified(buffer),
                Slot::OriginalReadWrite { buffer, .. } | Slot::ModifiedReadWrite { buffer, .. } => {
                    Slot::Modified(buffer.into_shared())
                }
            })
        }

        if original_parent {
            // Remove temporary values for `Address::NULL` contract, these are used as `#[tmp]`
            // "slots" by convention in the execution environment since there is no code behind
            // `Address::NULL` to possibly use it for anything
            inner
                .slots
                .retain(|(slot_key, _slot)| slot_key.contract != Address::NULL);
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

    /// Create a new nested read-write slots instance.
    ///
    /// Nested instance will integrate its changes into the parent slot when dropped (or changes can
    /// be reset with [`Self::reset()`]).
    ///
    /// Returns `None` when attempted on read-only instance.
    #[inline(always)]
    pub fn new_nested_rw<'b>(&'b mut self) -> Option<NestedSlots<'b>>
    where
        'a: 'b,
    {
        let inner = match &mut self.0 {
            NestedSlotsInner::ReadWrite { inner, .. } => &mut **inner,
            NestedSlotsInner::ReadOnly { .. } => {
                return None;
            }
        };

        let parent_slot_access_len = inner.slot_access.len();

        Some(NestedSlots(NestedSlotsInner::ReadWrite {
            inner,
            parent_slot_access_len,
            original_parent: false,
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
            debug!(%owner, "`add_new_contract` access violation");
            return false;
        };

        let new_contracts = &mut inner.new_contracts;

        if new_contracts.contains(&owner) {
            debug!(%owner, "Not adding new contract duplicate");
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
            debug!(%owner, "`get_code` access violation");
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
        if slot_access
            .iter()
            .any(|slot_access| slot_access.slot_index == slot_index && slot_access.read_write)
        {
            return None;
        }

        let buffer = match &slots
            .get(usize::from(slot_index))
            .expect("Just found; qed")
            .1
        {
            Slot::Original(buffer)
            | Slot::OriginalReadOnly(buffer)
            | Slot::Modified(buffer)
            | Slot::ModifiedReadOnly(buffer) => buffer,
            Slot::OriginalReadWrite { .. } | Slot::ModifiedReadWrite { .. } => {
                return None;
            }
        };

        Some(buffer.clone())
    }

    /// Read-only access to a slot with specified owner and contract, marks it as used.
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
        slots: &'b mut SmallVec<[(SlotKey, Slot); INLINE_SIZE]>,
        slot_access: &mut SmallVec<[SlotAccess; INLINE_SIZE]>,
        new_contracts: &[Address],
    ) -> Option<&'b SharedAlignedBuffer> {
        let maybe_slot_index = slots
            .iter()
            .position(|(slot_key_candidate, _slot)| slot_key_candidate == &slot_key)
            .map(SlotIndex);

        if let Some(slot_index) = maybe_slot_index {
            // Ensure that slot is not currently being written to
            if let Some(read_write) = slot_access.iter().find_map(|slot_access| {
                (slot_access.slot_index == slot_index).then_some(slot_access.read_write)
            }) {
                if read_write {
                    return None;
                }
            } else {
                slot_access.push(SlotAccess {
                    slot_index,
                    read_write: false,
                });
            }

            let slot = &mut slots
                .get_mut(usize::from(slot_index))
                .expect("Just found; qed")
                .1;

            // The slot that is currently being written to is not allowed for read access
            match slot {
                Slot::Original(buffer) => {
                    let buffer = buffer.clone();
                    *slot = Slot::OriginalReadOnly(buffer);
                    let Slot::OriginalReadOnly(buffer) = slot else {
                        unreachable!("Just inserted; qed");
                    };
                    Some(buffer)
                }
                Slot::OriginalReadOnly(buffer) | Slot::ModifiedReadOnly(buffer) => Some(buffer),
                Slot::Modified(buffer) => {
                    let buffer = buffer.clone();
                    *slot = Slot::ModifiedReadOnly(buffer);
                    let Slot::ModifiedReadOnly(buffer) = slot else {
                        unreachable!("Just inserted; qed");
                    };
                    Some(buffer)
                }
                Slot::OriginalReadWrite { .. } | Slot::ModifiedReadWrite { .. } => None,
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
                read_write: false,
            });

            let slot = Slot::OriginalReadOnly(SharedAlignedBuffer::default());
            slots.push((slot_key, slot));
            let slot = &slots.last().expect("Just inserted; qed").1;
            let Slot::OriginalReadOnly(buffer) = slot else {
                unreachable!("Just inserted; qed");
            };

            Some(buffer)
        }
    }

    /// Similar to [`Self::use_ro_internal()`], but for read-only instance
    #[inline(always)]
    fn use_ro_internal_read_only<'b>(
        slot_key: SlotKey,
        slots: &'b SmallVec<[(SlotKey, Slot); INLINE_SIZE]>,
        slot_access: &SmallVec<[SlotAccess; INLINE_SIZE]>,
        new_contracts: &[Address],
    ) -> Option<&'b SharedAlignedBuffer> {
        let maybe_slot_index = slots
            .iter()
            .position(|(slot_key_candidate, _slot)| slot_key_candidate == &slot_key)
            .map(SlotIndex);

        if let Some(slot_index) = maybe_slot_index {
            // Ensure that slot is not currently being written to
            if let Some(read_write) = slot_access.iter().find_map(|slot_access| {
                (slot_access.slot_index == slot_index).then_some(slot_access.read_write)
            }) {
                if read_write {
                    return None;
                }
            }

            let slot = &slots
                .get(usize::from(slot_index))
                .expect("Just found; qed")
                .1;

            // The slot that is currently being written to is not allowed for read access
            match slot {
                Slot::Original(buffer)
                | Slot::OriginalReadOnly(buffer)
                | Slot::ModifiedReadOnly(buffer)
                | Slot::Modified(buffer) => Some(buffer),
                Slot::OriginalReadWrite { .. } | Slot::ModifiedReadWrite { .. } => None,
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

    /// Read-write access to a slot with specified owner and contract, marks it as used.
    ///
    /// The returned slot is no longer accessible through [`Self::use_ro()`] or [`Self::use_rw()`]
    /// during the lifetime of this `Slot` instance (and can be safely turned into a pointer). The
    /// only way to get another mutable reference is to call [`Self::access_used_rw()`].
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
        slots: &'b mut SmallVec<[(SlotKey, Slot); INLINE_SIZE]>,
        slot_access: &mut SmallVec<[SlotAccess; INLINE_SIZE]>,
        new_contracts: &[Address],
    ) -> Option<(SlotIndex, &'b mut OwnedAlignedBuffer)> {
        let maybe_slot_index = slots
            .iter()
            .position(|(slot_key_candidate, _slot)| slot_key_candidate == &slot_key)
            .map(SlotIndex);

        if let Some(slot_index) = maybe_slot_index {
            // Ensure that slot is not accessed right now
            if slot_access
                .iter()
                .any(|slot_access| slot_access.slot_index == slot_index)
            {
                return None;
            }

            slot_access.push(SlotAccess {
                slot_index,
                read_write: true,
            });

            let slot = &mut slots
                .get_mut(usize::from(slot_index))
                .expect("Just found; qed")
                .1;

            // The slot that is currently being accessed to is not allowed for writing
            let buffer = match slot {
                Slot::OriginalReadOnly(_buffer) | Slot::ModifiedReadOnly(_buffer) => {
                    return None;
                }
                Slot::Original(buffer) => {
                    let mut new_buffer =
                        OwnedAlignedBuffer::with_capacity(capacity.max(buffer.len()));
                    new_buffer.copy_from_slice(buffer.as_slice());

                    *slot = Slot::OriginalReadWrite {
                        buffer: new_buffer,
                        previous: buffer.clone(),
                    };
                    let Slot::OriginalReadWrite { buffer, .. } = slot else {
                        unreachable!("Just inserted; qed");
                    };
                    buffer
                }
                Slot::Modified(buffer) => {
                    let mut new_buffer =
                        OwnedAlignedBuffer::with_capacity(capacity.max(buffer.len()));
                    new_buffer.copy_from_slice(buffer.as_slice());

                    *slot = Slot::ModifiedReadWrite {
                        buffer: new_buffer,
                        previous: buffer.clone(),
                    };
                    let Slot::ModifiedReadWrite { buffer, .. } = slot else {
                        unreachable!("Just inserted; qed");
                    };
                    buffer
                }
                Slot::OriginalReadWrite { buffer, .. } | Slot::ModifiedReadWrite { buffer, .. } => {
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
                read_write: true,
            });

            let slot = Slot::OriginalReadWrite {
                buffer: OwnedAlignedBuffer::with_capacity(capacity),
                previous: SharedAlignedBuffer::default(),
            };
            slots.push((slot_key, slot));
            let slot = &mut slots.last_mut().expect("Just inserted; qed").1;
            let Slot::OriginalReadWrite { buffer, .. } = slot else {
                unreachable!("Just inserted; qed");
            };

            Some((slot_index, buffer))
        }
    }

    /// Read-write access to a slot with specified owner and contract, that is currently marked as
    /// used due to earlier call to [`Self::use_rw()`].
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
            Slot::Original(_buffer)
            | Slot::OriginalReadOnly(_buffer)
            | Slot::Modified(_buffer)
            | Slot::ModifiedReadOnly(_buffer) => {
                debug!(?slot_index, "`access_used_rw` access violation (read only)");
                None
            }
            Slot::OriginalReadWrite { buffer, .. } | Slot::ModifiedReadWrite { buffer, .. } => {
                Some(buffer)
            }
        }
    }

    /// Reset any changes that might have been done on this level
    #[cold]
    pub fn reset(&mut self) {
        let (inner, parent_slot_access_len) = match &mut self.0 {
            NestedSlotsInner::ReadWrite {
                inner,
                parent_slot_access_len,
                original_parent: _,
            } => (&mut **inner, parent_slot_access_len),
            NestedSlotsInner::ReadOnly { .. } => {
                // No need to integrate changes into the parent
                return;
            }
        };

        let slots = &mut inner.slots;
        let slot_access = &mut inner.slot_access;

        // Fix-up slots that were modified during access
        for slot_access in slot_access.drain(*parent_slot_access_len..) {
            let slot = &mut slots
                .get_mut(usize::from(slot_access.slot_index))
                .expect("Accessed slot exists; qed")
                .1;
            take_mut::take(slot, |slot| match slot {
                Slot::Original(_buffer) => {
                    unreachable!("Slot can't be in Original state after being accessed")
                }
                Slot::OriginalReadOnly(buffer) => Slot::Original(buffer),
                Slot::Modified(buffer) => Slot::Modified(buffer),
                Slot::ModifiedReadOnly(buffer) => Slot::Modified(buffer),
                Slot::OriginalReadWrite { previous, .. } => Slot::Original(previous),
                Slot::ModifiedReadWrite { previous, .. } => Slot::Modified(previous),
            });
        }

        *parent_slot_access_len = 0;
    }
}
