use ab_aligned_buffer::SharedAlignedBuffer;
use ab_client_api::ContractSlotState;
use ab_core_primitives::address::Address;
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::trees::{StateOwnerContractsSparseMerkleTree, StateOwnersSparseMerkleTree};
use ab_merkle_tree::sparse::Leaf;
use blake3::hash;
use std::collections::BTreeMap;
use std::mem::MaybeUninit;
use std::num::NonZeroU128;
use triomphe::{Arc, UniqueArc};

// TODO: This is very inefficient, but will do for now
#[derive(Debug, Clone)]
pub struct GlobalState {
    /// Map from the owner address to a map from the management contract to the corresponding state
    state: BTreeMap<Address, BTreeMap<Address, SharedAlignedBuffer>>,
    total_len: usize,
}

impl GlobalState {
    pub fn new(system_contract_states: &[ContractSlotState]) -> Self {
        let mut state = BTreeMap::<Address, BTreeMap<_, _>>::new();
        for system_contract_state in system_contract_states {
            state
                .entry(system_contract_state.owner)
                .or_default()
                .insert(
                    system_contract_state.contract,
                    system_contract_state.contents.clone(),
                );
        }

        Self {
            state,
            total_len: system_contract_states.len(),
        }
    }

    pub fn to_system_contract_states(&self) -> Arc<[ContractSlotState]> {
        let mut system_contract_states =
            UniqueArc::<[MaybeUninit<ContractSlotState>]>::new_uninit_slice(self.total_len);

        self.state
            .iter()
            .flat_map(|(owner, state)| {
                state.iter().map(|(contract, contents)| ContractSlotState {
                    owner: *owner,
                    contract: *contract,
                    contents: contents.clone(),
                })
            })
            .zip(system_contract_states.iter_mut())
            .for_each(|(input, output)| {
                output.write(input);
            });

        // SAFETY: Just initialized all entries, internal invariant guarantees that `self.total_len`
        // matches the number of entries deep in the state tree
        unsafe { UniqueArc::assume_init_slice(system_contract_states) }.shareable()
    }

    pub fn root(&self) -> Blake3Hash {
        let mut previous_owner = None;

        let maybe_state_root = StateOwnersSparseMerkleTree::compute_root_only(
            self.state.iter().flat_map(|(&owner, state)| {
                let owner = u128::from(owner);
                let skip_leaf = if let Some(previous_owner) = previous_owner
                    && previous_owner + 1 != owner
                {
                    let skip_count = NonZeroU128::new(owner - previous_owner).expect(
                        "Owner is a larger number due to BTreeMap, hence the difference is more \
                        than zero; qed",
                    );
                    Some(Leaf::Empty { skip_count })
                } else {
                    None
                };
                previous_owner.replace(owner);

                let mut previous_contract = None;

                let maybe_owner_root = StateOwnerContractsSparseMerkleTree::compute_root_only(
                    state.iter().flat_map(|(&contract, contents)| {
                        let contract = u128::from(contract);
                        let skip_leaf = if let Some(previous_contract) = previous_contract
                            && previous_contract + 1 != contract
                        {
                            let skip_count = NonZeroU128::new(contract - previous_contract).expect(
                                "Contract is a larger number due to BTreeMap, hence the difference \
                                is more than zero; qed",
                            );
                            Some(Leaf::Empty { skip_count })
                        } else {
                            None
                        };
                        previous_contract.replace(contract);

                        skip_leaf.into_iter().chain([Leaf::OccupiedOwned {
                            leaf: *hash(contents.as_slice()).as_bytes(),
                        }])
                    }),
                );
                let owner_root = maybe_owner_root.expect(
                    "The number of leaves is limited by address space, which is 128-bit; qed",
                );

                skip_leaf
                    .into_iter()
                    .chain([Leaf::OccupiedOwned { leaf: owner_root }])
            }),
        );
        let state_root = maybe_state_root
            .expect("The number of leaves is limited by address space, which is 128-bit; qed");

        Blake3Hash::new(state_root)
    }
}
