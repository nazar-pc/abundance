use crate::aligned_buffer::{OwnedAlignedBuffer, SharedAlignedBuffer};
use ab_contracts_common::Address;
use parking_lot::Mutex;
use smallvec::SmallVec;
use std::mem;
use std::sync::{Arc, Weak};
use tracing::debug;

/// Small number of elements to store without heap allocation in some data structures.
///
/// This is both large enough for many practical use cases and small enough to bring significant
/// performance improvement.
const INLINE_SIZE: usize = 8;
/// It should be rare that more than 2 contracts are created in the same transaction
const NEW_CONTRACTS_INLINE: usize = 2;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub(super) struct SlotKey {
    pub(super) owner: Address,
    pub(super) contract: Address,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub(super) struct SlotIndex(usize);

impl From<SlotIndex> for usize {
    fn from(value: SlotIndex) -> Self {
        value.0
    }
}

#[derive(Debug, Clone)]
enum Slot {
    /// Original slot as given to the execution environment, not accessed yet
    Original(SharedAlignedBuffer),
    /// Original slot as given to the execution environment that is currently being accessed
    OriginalAccessed(SharedAlignedBuffer),
    /// Previously modified slot
    Modified(SharedAlignedBuffer),
    /// Previously modified slot that is currently being accessed for reads
    ModifiedAccessed(SharedAlignedBuffer),
    /// Original slot as given to the execution environment that is currently being modified
    ReadWriteOriginal {
        buffer: OwnedAlignedBuffer,
        /// What it was in [`Self::Original`] before becoming [`Self::ReadWriteOriginal`]
        previous: SharedAlignedBuffer,
    },
    /// Previously modified slot that is currently being modified
    ReadWriteModified {
        buffer: OwnedAlignedBuffer,
        /// What it was in [`Self::ReadOnlyModified`] before becoming [`Self::ReadWriteModified`]
        previous: SharedAlignedBuffer,
    },
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
struct SlotAccess {
    slot_index: SlotIndex,
    /// `false` for read-only and `true` for read-write
    read_write: bool,
}

#[derive(Debug)]
struct SlotsParent {
    parent_slot_access_len: usize,
    parent: Arc<Mutex<Slots>>,
}

#[derive(Debug)]
pub(super) struct Slots {
    slots: SmallVec<[(SlotKey, Slot); INLINE_SIZE]>,
    slot_access: SmallVec<[SlotAccess; INLINE_SIZE]>,
    /// The list of new addresses that were created during transaction processing and couldn't be
    /// known beforehand.
    ///
    /// Addresses in this list are allowed to create slots for any owner, and other contacts are
    /// allowed to create slots owned by these addresses.
    new_contracts: SmallVec<[Address; NEW_CONTRACTS_INLINE]>,
    access_violation: bool,
    read_only: bool,
    weak: Weak<Mutex<Self>>,
    parent: Option<SlotsParent>,
}

impl Drop for Slots {
    fn drop(&mut self) {
        let Some(SlotsParent {
            parent_slot_access_len,
            parent,
        }) = self.parent.take()
        else {
            return;
        };

        let parent = &mut *parent.lock();

        parent.access_violation = self.access_violation;
        if self.access_violation {
            return;
        }
        mem::swap(&mut parent.slots, &mut self.slots);
        mem::swap(&mut parent.slot_access, &mut self.slot_access);
        mem::swap(&mut parent.new_contracts, &mut self.new_contracts);

        // Fix-up slots that were modified during access
        for slot_access in parent.slot_access.drain(parent_slot_access_len..) {
            let slot = &mut parent
                .slots
                .get_mut(usize::from(slot_access.slot_index))
                .expect("Accessed slot exists; qed")
                .1;
            take_mut::take(slot, |slot| match slot {
                Slot::Original(_buffer) => {
                    unreachable!("Slot can't be in Original state after being accessed")
                }
                Slot::OriginalAccessed(buffer) => Slot::Original(buffer),
                Slot::Modified(buffer) => Slot::Modified(buffer),
                Slot::ModifiedAccessed(buffer) => Slot::Modified(buffer),
                Slot::ReadWriteOriginal { buffer, .. } | Slot::ReadWriteModified { buffer, .. } => {
                    Slot::Modified(buffer.into_shared())
                }
            })
        }
    }
}

// TODO: Method for getting all slots and modified slots, take `#[tmp]` into consideration
impl Slots {
    /// Create a new instance from a hashmap containing existing slots.
    ///
    /// Only slots that are present in the input can be modified. The only exception is slots for
    /// owners created during runtime and initialized with [`Self::add_new_contract()`].
    ///
    /// "Empty" slots must still have a value in the form of an empty [`SharedAlignedBuffer`].
    pub(super) fn new<I>(slots: I) -> Arc<Mutex<Self>>
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
        Arc::new_cyclic(|weak| {
            Mutex::new(Self {
                slots,
                slot_access: SmallVec::new(),
                new_contracts: SmallVec::new(),
                access_violation: false,
                read_only: false,
                weak: weak.clone(),
                parent: None,
            })
        })
    }

    /// Create a new nested slots instance.
    ///
    /// Nested instance will integrate its changes into the parent slot when dropped.
    ///
    /// *Only one nested instance can exist at the same time*, if more than one exists, it will
    /// cause slot contents corruption.
    ///
    /// *Make sure to release lock on parent instance before nested instance if dropped.*
    pub(super) fn new_nested(&mut self, read_only: bool) -> Arc<Mutex<Self>> {
        Arc::new_cyclic(|weak| {
            let parent_slot_access_len = self.slot_access.len();
            Mutex::new(Self {
                // Steal the value, will be re-integrated back into the parent when nested instance
                // is dropped
                slots: mem::take(&mut self.slots),
                // Steal the value, will be re-integrated back into the parent when nested instance
                // is dropped
                slot_access: mem::take(&mut self.slot_access),
                // Steal the value, will be re-integrated back into the parent when nested instance
                // is dropped
                new_contracts: mem::take(&mut self.new_contracts),
                access_violation: self.access_violation,
                read_only,
                weak: weak.clone(),
                parent: Some(SlotsParent {
                    parent_slot_access_len,
                    parent: self
                        .weak
                        .upgrade()
                        .expect("Called from within an instance itself; qed"),
                }),
            })
        })
    }

    /// Check if there was an access violation up until this point
    pub(super) fn access_violation(&self) -> bool {
        self.access_violation
    }

    /// Mark slot as used, such that execution environment can read/write from/to it
    pub(super) fn use_slot(&mut self, slot_key: SlotKey) {
        if !self
            .slots
            .iter()
            .any(|(slot_key_candidate, _slot)| slot_key_candidate == &slot_key)
        {
            self.slots
                .push((slot_key, Slot::Original(SharedAlignedBuffer::default())));
        }
    }

    /// Add a new contract that didn't exist before.
    ///
    /// In contrast to contracts in [`Self::new()`], this contract will be allowed to have any of its
    /// slots modified.
    ///
    /// Returns `false` if a contract already exits in a map, this is also considered as an access
    /// violation.
    #[must_use]
    pub(super) fn add_new_contract(&mut self, owner: Address) -> bool {
        if self.new_contracts.contains(&owner) {
            debug!(%owner, "Not adding new contract duplicate");
            self.access_violation = true;
            return false;
        }

        self.new_contracts.push(owner);
        true
    }

    /// Get code for `owner`.
    ///
    /// The biggest difference from [`Self::use_ro()`] is that the slot is not marked as used,
    /// instead the current code is cloned and returned.
    ///
    /// Returns `None` in case of access violation or if code is missing.
    pub(super) fn get_code(&mut self, owner: Address) -> Option<SharedAlignedBuffer> {
        let result = Self::get_code_internal(owner, &self.slots, &self.slot_access);

        if result.is_none() {
            debug!(%owner, "`get_code` state access violation");
            self.access_violation = true;
        }

        result
    }

    fn get_code_internal(
        owner: Address,
        slots: &SmallVec<[(SlotKey, Slot); INLINE_SIZE]>,
        slot_access: &[SlotAccess],
    ) -> Option<SharedAlignedBuffer> {
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
            | Slot::OriginalAccessed(buffer)
            | Slot::Modified(buffer)
            | Slot::ModifiedAccessed(buffer) => buffer,
            Slot::ReadWriteOriginal { .. } | Slot::ReadWriteModified { .. } => {
                return None;
            }
        };

        Some(buffer.clone())
    }

    /// Read-only access to a slot with specified owner and contract, marks it as used.
    ///
    /// Returns `None` in case of access violation.
    pub(super) fn use_ro(&mut self, slot_key: SlotKey) -> Option<&SharedAlignedBuffer> {
        let result = if self.read_only {
            // Simplified version that doesn't do access tracking
            Self::use_ro_internal_read_only(
                slot_key,
                &mut self.slots,
                &mut self.slot_access,
                &self.new_contracts,
            )
        } else {
            Self::use_ro_internal(
                slot_key,
                &mut self.slots,
                &mut self.slot_access,
                &self.new_contracts,
            )
        };

        if result.is_none() {
            debug!(?slot_key, "`use_ro` state access violation");
            self.access_violation = true;
        }

        result
    }

    fn use_ro_internal<'a>(
        slot_key: SlotKey,
        slots: &'a mut SmallVec<[(SlotKey, Slot); INLINE_SIZE]>,
        slot_access: &mut SmallVec<[SlotAccess; INLINE_SIZE]>,
        new_contracts: &[Address],
    ) -> Option<&'a SharedAlignedBuffer> {
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
                    *slot = Slot::OriginalAccessed(buffer);
                    let Slot::OriginalAccessed(buffer) = slot else {
                        unreachable!("Just inserted; qed");
                    };
                    Some(buffer)
                }
                Slot::OriginalAccessed(buffer) | Slot::ModifiedAccessed(buffer) => Some(buffer),
                Slot::Modified(buffer) => {
                    let buffer = buffer.clone();
                    *slot = Slot::ModifiedAccessed(buffer);
                    let Slot::ModifiedAccessed(buffer) = slot else {
                        unreachable!("Just inserted; qed");
                    };
                    Some(buffer)
                }
                Slot::ReadWriteOriginal { .. } | Slot::ReadWriteModified { .. } => None,
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

            slot_access.push(SlotAccess {
                slot_index: SlotIndex(slots.len()),
                read_write: false,
            });

            let slot = Slot::OriginalAccessed(SharedAlignedBuffer::default());
            slots.push((slot_key, slot));
            let slot = &slots.last().expect("Just inserted; qed").1;
            let Slot::OriginalAccessed(buffer) = slot else {
                unreachable!("Just inserted; qed");
            };

            Some(buffer)
        }
    }

    fn use_ro_internal_read_only<'a>(
        slot_key: SlotKey,
        slots: &'a mut SmallVec<[(SlotKey, Slot); INLINE_SIZE]>,
        slot_access: &mut SmallVec<[SlotAccess; INLINE_SIZE]>,
        new_contracts: &[Address],
    ) -> Option<&'a SharedAlignedBuffer> {
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

            let slot = &mut slots
                .get_mut(usize::from(slot_index))
                .expect("Just found; qed")
                .1;

            // The slot that is currently being written to is not allowed for read access
            match slot {
                Slot::Original(buffer)
                | Slot::OriginalAccessed(buffer)
                | Slot::ModifiedAccessed(buffer)
                | Slot::Modified(buffer) => Some(buffer),
                Slot::ReadWriteOriginal { .. } | Slot::ReadWriteModified { .. } => None,
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

            let slot = Slot::OriginalAccessed(SharedAlignedBuffer::default());
            slots.push((slot_key, slot));
            let slot = &slots.last().expect("Just inserted; qed").1;
            let Slot::OriginalAccessed(buffer) = slot else {
                unreachable!("Just inserted; qed");
            };

            Some(buffer)
        }
    }

    /// Read-write access to a slot with specified owner and contract, marks it as used.
    ///
    /// Returns `None` in case of access violation.
    pub(super) fn use_rw(
        &mut self,
        slot_key: SlotKey,
        capacity: u32,
    ) -> Option<(SlotIndex, &mut OwnedAlignedBuffer)> {
        let result = Self::use_rw_internal(
            slot_key,
            capacity,
            &mut self.slots,
            &mut self.slot_access,
            &self.new_contracts,
        );

        if self.read_only || result.is_none() {
            debug!(?slot_key, "`use_rw` state access violation");
            self.access_violation = true;
        }

        result
    }

    fn use_rw_internal<'a>(
        slot_key: SlotKey,
        capacity: u32,
        slots: &'a mut SmallVec<[(SlotKey, Slot); INLINE_SIZE]>,
        slot_access: &mut SmallVec<[SlotAccess; INLINE_SIZE]>,
        new_contracts: &[Address],
    ) -> Option<(SlotIndex, &'a mut OwnedAlignedBuffer)> {
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
                Slot::OriginalAccessed(_buffer) | Slot::ModifiedAccessed(_buffer) => {
                    return None;
                }
                Slot::Original(buffer) => {
                    let mut new_buffer =
                        OwnedAlignedBuffer::with_capacity(capacity.max(buffer.len()));
                    new_buffer.copy_from_slice(buffer.as_slice());

                    *slot = Slot::ReadWriteOriginal {
                        buffer: new_buffer,
                        previous: buffer.clone(),
                    };
                    let Slot::ReadWriteOriginal { buffer, .. } = slot else {
                        unreachable!("Just inserted; qed");
                    };
                    buffer
                }
                Slot::Modified(buffer) => {
                    let mut new_buffer =
                        OwnedAlignedBuffer::with_capacity(capacity.max(buffer.len()));
                    new_buffer.copy_from_slice(buffer.as_slice());

                    *slot = Slot::ReadWriteModified {
                        buffer: new_buffer,
                        previous: buffer.clone(),
                    };
                    let Slot::ReadWriteModified { buffer, .. } = slot else {
                        unreachable!("Just inserted; qed");
                    };
                    buffer
                }
                Slot::ReadWriteOriginal { buffer, .. } | Slot::ReadWriteModified { buffer, .. } => {
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

            let slot = Slot::ReadWriteOriginal {
                buffer: OwnedAlignedBuffer::with_capacity(capacity),
                previous: SharedAlignedBuffer::default(),
            };
            slots.push((slot_key, slot));
            let slot = &mut slots.last_mut().expect("Just inserted; qed").1;
            let Slot::ReadWriteOriginal { buffer, .. } = slot else {
                unreachable!("Just inserted; qed");
            };

            Some((slot_index, buffer))
        }
    }

    /// Read-write access to a slot with specified owner and contract, that is currently marked as
    /// used due to earlier call to [`Self::use_rw()`].
    ///
    /// Returns `None` in case of access violation.
    pub(super) fn access_used_rw(
        &mut self,
        slot_index: SlotIndex,
    ) -> Option<&mut OwnedAlignedBuffer> {
        let maybe_slot = self
            .slots
            .get_mut(usize::from(slot_index))
            .map(|(_slot_key, slot)| slot);

        let Some(slot) = maybe_slot else {
            debug!(
                ?slot_index,
                "`access_used_rw` state access violation (not found)"
            );
            self.access_violation = true;
            return None;
        };

        // Must be currently accessed for writing
        match slot {
            Slot::Original(_buffer)
            | Slot::OriginalAccessed(_buffer)
            | Slot::Modified(_buffer)
            | Slot::ModifiedAccessed(_buffer) => {
                debug!(
                    ?slot_index,
                    "`access_used_rw` state access violation (read only)"
                );
                self.access_violation = true;
                None
            }
            Slot::ReadWriteOriginal { buffer, .. } | Slot::ReadWriteModified { buffer, .. } => {
                Some(buffer)
            }
        }
    }

    /// Reset any changes that might have been done on this level
    pub(super) fn reset(mut self) {
        let Some(SlotsParent {
            parent_slot_access_len,
            parent,
        }) = self.parent.take()
        else {
            return;
        };

        let parent = &mut *parent.lock();

        parent.access_violation = self.access_violation;
        if self.access_violation {
            return;
        }
        mem::swap(&mut parent.slots, &mut self.slots);
        mem::swap(&mut parent.slot_access, &mut self.slot_access);
        mem::swap(&mut parent.new_contracts, &mut self.new_contracts);

        // Fix-up slots that were modified during access
        for slot_access in parent.slot_access.drain(parent_slot_access_len..) {
            let slot = &mut parent
                .slots
                .get_mut(usize::from(slot_access.slot_index))
                .expect("Accessed slot exists; qed")
                .1;
            take_mut::take(slot, |slot| match slot {
                Slot::Original(_buffer) => {
                    unreachable!("Slot can't be in Original state after being accessed")
                }
                Slot::OriginalAccessed(buffer) => Slot::Original(buffer),
                Slot::Modified(buffer) => Slot::Modified(buffer),
                Slot::ModifiedAccessed(buffer) => Slot::Modified(buffer),
                Slot::ReadWriteOriginal { previous, .. } => Slot::Original(previous),
                Slot::ReadWriteModified { previous, .. } => Slot::Modified(previous),
            });
        }
    }
}
