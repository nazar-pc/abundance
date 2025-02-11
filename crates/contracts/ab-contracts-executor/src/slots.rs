use crate::aligned_buffer::{OwnedAlignedBuffer, SharedAlignedBuffer};
use ab_contracts_common::{Address, ContractError};
use parking_lot::Mutex;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::warn;

#[derive(Debug, Default)]
pub(super) struct Slots {
    // TODO: Think about optimizing locking
    slots: HashMap<Address, HashMap<Address, SharedAlignedBuffer>>,
}

impl Slots {
    pub(super) fn get(&self, owner: &Address, contract: &Address) -> Option<SharedAlignedBuffer> {
        self.slots.get(owner)?.get(contract).cloned()
    }

    pub(super) fn put(&mut self, owner: Address, contract: Address, value: SharedAlignedBuffer) {
        self.slots.entry(owner).or_default().insert(contract, value);
    }

    pub(super) fn remove(&mut self, owner: &Address, contract: &Address) {
        if let Some(owner_slots) = self.slots.get_mut(owner) {
            owner_slots.remove(contract);
        }
    }
}

#[derive(Debug)]
enum SlotAccess {
    ReadOnly {
        counter: usize,
        bytes: SharedAlignedBuffer,
    },
    ReadWrite {
        original_bytes: SharedAlignedBuffer,
        bytes: OwnedAlignedBuffer,
    },
}

impl SlotAccess {
    fn new_ro(bytes: SharedAlignedBuffer) -> Self {
        Self::ReadOnly { counter: 1, bytes }
    }

    fn new_rw(original_bytes: SharedAlignedBuffer, capacity: u32) -> Self {
        let mut bytes = OwnedAlignedBuffer::with_capacity(original_bytes.len().max(capacity));
        bytes.copy_from_slice(&original_bytes);

        Self::ReadWrite {
            original_bytes,
            bytes,
        }
    }

    fn inc_ro(&mut self) -> Result<&SharedAlignedBuffer, ContractError> {
        match self {
            SlotAccess::ReadOnly { counter, bytes } => {
                *counter += 1;
                Ok(bytes)
            }
            SlotAccess::ReadWrite { .. } => Err(ContractError::BadInput),
        }
    }
}

#[derive(Debug, Eq, PartialEq, Hash)]
struct UsedSlot {
    /// Address of the contract whose tree contains the slot
    owner: Address,
    /// Address of the contract that manages the slot under `owner`'s tree
    contract: Address,
}

// TODO: Some notion of branching/generations that allows to persist only some slots
#[derive(Debug)]
pub(super) struct UsedSlots {
    used_slots: HashMap<UsedSlot, SlotAccess>,
    slots: Arc<Mutex<Slots>>,
}

impl UsedSlots {
    pub(super) fn new(slots: Arc<Mutex<Slots>>) -> Self {
        Self {
            used_slots: HashMap::new(),
            slots,
        }
    }

    pub(super) fn use_ro(
        &mut self,
        owner: Address,
        contract: Address,
    ) -> Result<&SharedAlignedBuffer, ContractError> {
        match self.used_slots.entry(UsedSlot { owner, contract }) {
            Entry::Occupied(entry) => entry.into_mut().inc_ro().inspect_err(|_error| {
                warn!(%owner, "Failed to access ro slot");
            }),
            Entry::Vacant(entry) => {
                let bytes = self.slots.lock().get(&owner, &contract).unwrap_or_default();
                let SlotAccess::ReadOnly { bytes, .. } = entry.insert(SlotAccess::new_ro(bytes))
                else {
                    unreachable!("Just inserted `ReadOnly` entry; qed");
                };
                Ok(bytes)
            }
        }
    }

    pub(super) fn use_rw(
        &mut self,
        owner: Address,
        contract: Address,
        capacity: u32,
    ) -> Result<&mut OwnedAlignedBuffer, ContractError> {
        match self.used_slots.entry(UsedSlot { owner, contract }) {
            Entry::Occupied(_entry) => {
                warn!(%owner, "Failed to access rw slot");
                Err(ContractError::BadInput)
            }
            Entry::Vacant(entry) => {
                // TODO: If there were no recursive calls, we could simply remove original bytes and
                //  avoid unnecessary copies in many cases, with recursion we can also do that, but
                //  only on the highest level. For deeper level we need to take special care of
                //  `Slots` because modification of one recursive call doesn't necessarily mean
                //  other recursive calls will fail that may try to modify the same data that failed
                //  call tried
                let bytes = self.slots.lock().get(&owner, &contract).unwrap_or_default();
                let SlotAccess::ReadWrite { bytes, .. } =
                    entry.insert(SlotAccess::new_rw(bytes, capacity))
                else {
                    unreachable!("Just inserted `ReadWrite` entry; qed");
                };
                Ok(bytes)
            }
        }
    }

    /// Persist changes to modified slots
    pub(super) fn persist(self) {
        let mut slots = self.slots.lock();
        for (used_slot, slot_access) in self.used_slots {
            let bytes = match slot_access {
                SlotAccess::ReadOnly { .. } => {
                    continue;
                }
                SlotAccess::ReadWrite {
                    original_bytes,
                    bytes,
                } => {
                    if original_bytes == bytes {
                        continue;
                    }

                    bytes
                }
            };

            let UsedSlot { owner, contract } = used_slot;

            if contract == Address::NULL {
                // Null contact is used implicitly for `#[tmp]` since it is not possible for this
                // contract to write something there directly
                continue;
            }

            if bytes.is_empty() {
                slots.remove(&owner, &contract);
            } else {
                slots.put(owner, contract, bytes.into_shared());
            }
        }
    }
}
