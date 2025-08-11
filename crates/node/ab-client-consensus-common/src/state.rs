use ab_aligned_buffer::SharedAlignedBuffer;
use ab_client_api::ContractSlotState;
use ab_core_primitives::address::Address;
use ab_core_primitives::hashes::Blake3Hash;
use ab_merkle_tree::sparse::{Leaf, SparseMerkleTree};
use blake3::hash;
use std::collections::BTreeMap;
use std::num::NonZeroU128;
use std::sync::Arc as StdArc;

type Smt128 = SparseMerkleTree<{ size_of::<Address>() as u8 * u8::BITS as u8 }>;

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
        for system_contract_state in system_contract_states.iter() {
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

    pub fn to_system_contract_states(&self) -> StdArc<[ContractSlotState]> {
        let mut system_contract_states =
            StdArc::<[ContractSlotState]>::new_uninit_slice(self.total_len);

        self.state
            .iter()
            .flat_map(|(owner, state)| {
                state.iter().map(|(contract, contents)| ContractSlotState {
                    owner: *owner,
                    contract: *contract,
                    contents: contents.clone(),
                })
            })
            // SAFETY: A single pointer and a single use
            .zip(unsafe { StdArc::get_mut_unchecked(&mut system_contract_states) })
            .for_each(|(input, output)| {
                output.write(input);
            });

        // SAFETY: Just initialized all entries, internal invariant guarantees that `self.total_len`
        // matches the number of entries deep in the state tree
        unsafe { system_contract_states.assume_init() }
    }

    pub fn root(&self) -> Blake3Hash {
        let mut previous_owner = None;

        let maybe_state_root =
            Smt128::compute_root_only(self.state.iter().flat_map(|(&owner, state)| {
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

                let maybe_owner_root =
                    Smt128::compute_root_only(state.iter().flat_map(|(&contract, contents)| {
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
                            // TODO: Should probably use keyed hash instead
                            leaf: *hash(contents.as_slice()).as_bytes(),
                        }])
                    }));
                let owner_root = maybe_owner_root.expect(
                    "The number of leaves is limited by address space, which is 128-bit; qed",
                );

                skip_leaf
                    .into_iter()
                    .chain([Leaf::OccupiedOwned { leaf: owner_root }])
            }));
        let state_root = maybe_state_root
            .expect("The number of leaves is limited by address space, which is 128-bit; qed");

        Blake3Hash::new(state_root)
    }
}
