use crate::{BlockVerification, BlockVerificationError, GenericBody, GenericHeader};
use ab_client_api::{BlockOrigin, ChainInfo, ChainSyncStatus};
use ab_client_archiving::segment_headers_store::SegmentHeadersStore;
use ab_client_consensus_common::ConsensusConstants;
use ab_client_consensus_common::consensus_parameters::{
    DeriveConsensusParametersError, derive_consensus_parameters,
};
use ab_client_proof_of_time::PotNextSlotInput;
use ab_client_proof_of_time::verifier::PotVerifier;
use ab_core_primitives::block::body::{BeaconChainBody, IntermediateShardBlocksInfo};
use ab_core_primitives::block::header::{
    BeaconChainHeader, BlockHeaderConsensusParameters, BlockHeaderPrefix,
    OwnedBlockHeaderConsensusParameters,
};
use ab_core_primitives::block::owned::OwnedBeaconChainBlock;
use ab_core_primitives::block::{BlockNumber, BlockRoot};
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pot::{PotCheckpoints, PotOutput, PotParametersChange, SlotNumber};
use ab_core_primitives::segments::SegmentRoot;
use ab_core_primitives::solutions::{SolutionVerifyError, SolutionVerifyParams};
use ab_proof_of_space::Table;
use rand::prelude::*;
use rayon::prelude::*;
use std::iter;
use std::marker::PhantomData;
use tracing::{debug, trace};

/// Errors for [`BeaconChainBlockVerification`]
#[derive(Debug, thiserror::Error)]
pub enum BeaconChainBlockVerificationError {
    /// Consensus parameters derivation error
    #[error("Consensus parameters derivation error: {error}")]
    ConsensusParametersDerivation {
        /// Consensus parameters derivation error
        #[from]
        error: DeriveConsensusParametersError,
    },
    /// Invalid consensus parameters
    #[error("Invalid consensus parameters")]
    InvalidConsensusParameters,
    /// Invalid PoT checkpoints
    #[error("Invalid PoT checkpoints")]
    InvalidPotCheckpoints,
    /// Invalid proof of time
    #[error("Invalid proof of time")]
    InvalidProofOfTime,
    /// Solution error
    #[error("Solution error: {error}")]
    SolutionError {
        /// Solution error
        #[from]
        error: SolutionVerifyError,
    },
}

impl From<BeaconChainBlockVerificationError> for BlockVerificationError {
    #[inline(always)]
    fn from(error: BeaconChainBlockVerificationError) -> Self {
        Self::Custom {
            error: error.into(),
        }
    }
}

#[derive(Debug)]
pub struct BeaconChainBlockVerification<PosTable, CI, CSS> {
    segment_headers_store: SegmentHeadersStore,
    consensus_constants: ConsensusConstants,
    pot_verifier: PotVerifier,
    chain_info: CI,
    chain_sync_status: CSS,
    _pos_table: PhantomData<PosTable>,
}

impl<PosTable, CI, CSS> BlockVerification<OwnedBeaconChainBlock>
    for BeaconChainBlockVerification<PosTable, CI, CSS>
where
    PosTable: Table,
    CI: ChainInfo<OwnedBeaconChainBlock>,
    CSS: ChainSyncStatus,
{
    #[inline(always)]
    async fn verify(
        &self,
        parent_header: &GenericHeader<'_, OwnedBeaconChainBlock>,
        parent_block_mmr_root: &Blake3Hash,
        header: &GenericHeader<'_, OwnedBeaconChainBlock>,
        body: &GenericBody<'_, OwnedBeaconChainBlock>,
        origin: BlockOrigin,
    ) -> Result<(), BlockVerificationError> {
        self.verify(parent_header, parent_block_mmr_root, header, body, origin)
            .await
    }
}

impl<PosTable, CI, CSS> BeaconChainBlockVerification<PosTable, CI, CSS>
where
    PosTable: Table,
    CI: ChainInfo<OwnedBeaconChainBlock>,
    CSS: ChainSyncStatus,
{
    /// Create a new instance
    #[inline(always)]
    pub fn new(
        segment_headers_store: SegmentHeadersStore,
        consensus_constants: ConsensusConstants,
        pot_verifier: PotVerifier,
        chain_info: CI,
        chain_sync_status: CSS,
    ) -> Self {
        Self {
            segment_headers_store,
            consensus_constants,
            pot_verifier,
            chain_info,
            chain_sync_status,
            _pos_table: PhantomData,
        }
    }

    /// Determine if full proof of time verification is needed for this block number
    fn full_pot_verification(&self, block_number: BlockNumber) -> bool {
        let sync_target_block_number = self.chain_sync_status.target_block_number();
        let Some(diff) = sync_target_block_number.checked_sub(block_number) else {
            return true;
        };
        let diff = diff.as_u64();

        let sample_size = match diff {
            ..=1_581 => {
                return true;
            }
            1_582..=6_234 => 1_581,
            6_235..=63_240 => 3_162 * (diff - 3_162) / (diff - 1),
            63_241..=3_162_000 => 3_162,
            _ => diff / 1_000,
        };

        let n = rand::rng().random_range(0..=diff);

        n < sample_size
    }

    fn check_header_prefix(
        &self,
        parent_header_prefix: &BlockHeaderPrefix,
        parent_block_mmr_root: &Blake3Hash,
        header_prefix: &BlockHeaderPrefix,
    ) -> Result<(), BlockVerificationError> {
        let basic_valid = header_prefix.version == BlockHeaderPrefix::BLOCK_VERSION
            && header_prefix.number == parent_header_prefix.number + BlockNumber::ONE
            && header_prefix.shard_index == parent_header_prefix.shard_index
            && &header_prefix.mmr_root == parent_block_mmr_root;

        if !basic_valid {
            return Err(BlockVerificationError::InvalidHeaderPrefix);
        }

        // TODO: Check Timestamp

        Ok(())
    }

    fn check_consensus_parameters(
        &self,
        parent_block_root: &BlockRoot,
        parent_header: &BeaconChainHeader<'_>,
        header: &BeaconChainHeader<'_>,
    ) -> Result<(), BeaconChainBlockVerificationError> {
        let derived_consensus_parameters = derive_consensus_parameters(
            &self.consensus_constants,
            &self.chain_info,
            parent_block_root,
            parent_header.consensus_parameters(),
            parent_header.consensus_info.slot,
            header.prefix.number,
            header.consensus_info.slot,
        )?;

        let expected_consensus_parameters = OwnedBlockHeaderConsensusParameters {
            fixed_parameters: derived_consensus_parameters.fixed_parameters,
            // TODO: Super segment support
            super_segment_root: None,
            next_solution_range: derived_consensus_parameters.next_solution_range,
            pot_parameters_change: derived_consensus_parameters.pot_parameters_change,
        };

        if header.consensus_parameters() != &expected_consensus_parameters.as_ref() {
            return Err(BeaconChainBlockVerificationError::InvalidConsensusParameters);
        }

        Ok(())
    }

    // TODO: This is a blocking function, but ideally wouldn't be block an executor
    /// Checks current/future proof of time in the consensus info for the slot and corresponding
    /// checkpoints.
    ///
    /// `consensus_parameters` is assumed to be correct and needs to be verified separately.
    ///
    /// When `verify_checkpoints == false` checkpoints are assumed to be correct and verification
    /// for them is skipped.
    #[expect(
        clippy::too_many_arguments,
        reason = "Explicit minimal input for better testability"
    )]
    fn check_proof_of_time(
        pot_verifier: &PotVerifier,
        block_authoring_delay: SlotNumber,
        parent_slot: SlotNumber,
        parent_proof_of_time: PotOutput,
        parent_future_proof_of_time: PotOutput,
        parent_consensus_parameters: &BlockHeaderConsensusParameters<'_>,
        slot: SlotNumber,
        proof_of_time: PotOutput,
        future_proof_of_time: PotOutput,
        consensus_parameters: &BlockHeaderConsensusParameters<'_>,
        checkpoints: &[PotCheckpoints],
        verify_checkpoints: bool,
    ) -> Result<(), BeaconChainBlockVerificationError> {
        let pot_parameters_change = consensus_parameters
            .pot_parameters_change
            .copied()
            .map(PotParametersChange::from);

        let parent_pot_parameters_change = parent_consensus_parameters
            .pot_parameters_change
            .copied()
            .map(PotParametersChange::from);

        // Last checkpoint must be the future proof of time
        if checkpoints.last().map(PotCheckpoints::output) != Some(future_proof_of_time) {
            return Err(BeaconChainBlockVerificationError::InvalidPotCheckpoints);
        }

        let parent_future_slot = if parent_slot == SlotNumber::ZERO {
            parent_slot
        } else {
            parent_slot + block_authoring_delay
        };

        let slots_between_blocks = slot
            .checked_sub(parent_slot)
            .ok_or(BeaconChainBlockVerificationError::InvalidPotCheckpoints)?;
        // Number of checkpoints must match the difference between parent's and this block's
        // future slots. This also implicitly checks that there is a non-zero number of slots
        // between this and parent block because list of checkpoints is already known to be not
        // empty from the check above.
        if slots_between_blocks.as_u64() != checkpoints.len() as u64 {
            return Err(BeaconChainBlockVerificationError::InvalidPotCheckpoints);
        }

        let mut pot_input = if parent_slot == SlotNumber::ZERO {
            PotNextSlotInput {
                slot: parent_slot + SlotNumber::ONE,
                slot_iterations: parent_consensus_parameters.fixed_parameters.slot_iterations,
                seed: pot_verifier.genesis_seed(),
            }
        } else {
            // Calculate slot iterations as of parent future slot
            let slot_iterations = parent_pot_parameters_change
                .and_then(|parameters_change| {
                    (parameters_change.slot <= parent_future_slot)
                        .then_some(parameters_change.slot_iterations)
                })
                .unwrap_or(parent_consensus_parameters.fixed_parameters.slot_iterations);
            // Derive inputs to the slot, which follows parent future slot
            PotNextSlotInput::derive(
                slot_iterations,
                parent_future_slot,
                parent_future_proof_of_time,
                &parent_pot_parameters_change,
            )
        };

        // Collect all the data we will use for verification so we can process it in parallel
        let checkpoints_verification_input = iter::once((
            pot_input,
            *checkpoints
                .first()
                .expect("Not empty, contents was checked above; qed"),
        ));
        let checkpoints_verification_input = checkpoints_verification_input
            .chain(checkpoints.array_windows::<2>().map(|[left, right]| {
                pot_input = PotNextSlotInput::derive(
                    pot_input.slot_iterations,
                    pot_input.slot,
                    left.output(),
                    &pot_parameters_change,
                );

                (pot_input, *right)
            }))
            // TODO: Would be nice to avoid extra allocation here
            .collect::<Vec<_>>();

        // All checkpoints must be valid, search for the first verification failure
        let all_checkpoints_valid =
            checkpoints_verification_input
                .into_par_iter()
                .all(|(pot_input, checkpoints)| {
                    if verify_checkpoints {
                        pot_verifier.verify_checkpoints(
                            pot_input.seed,
                            pot_input.slot_iterations,
                            &checkpoints,
                        )
                    } else {
                        // Store checkpoints as verified when verification is skipped
                        pot_verifier.inject_verified_checkpoints(
                            pot_input.seed,
                            pot_input.slot_iterations,
                            checkpoints,
                        );
                        true
                    }
                });

        if !all_checkpoints_valid {
            return Err(BeaconChainBlockVerificationError::InvalidPotCheckpoints);
        }

        // Make sure proof of time of this block correctly extends proof of time of the parent block
        {
            let pot_input = if parent_slot == SlotNumber::ZERO {
                PotNextSlotInput {
                    slot: parent_slot + SlotNumber::ONE,
                    slot_iterations: parent_consensus_parameters.fixed_parameters.slot_iterations,
                    seed: pot_verifier.genesis_seed(),
                }
            } else {
                // Calculate slot iterations as of parent slot
                let slot_iterations = parent_pot_parameters_change
                    .and_then(|parameters_change| {
                        (parameters_change.slot <= parent_slot)
                            .then_some(parameters_change.slot_iterations)
                    })
                    .unwrap_or(parent_consensus_parameters.fixed_parameters.slot_iterations);
                // Derive inputs to the slot, which follows parent slot
                PotNextSlotInput::derive(
                    slot_iterations,
                    parent_slot,
                    parent_proof_of_time,
                    &parent_pot_parameters_change,
                )
            };

            if pot_verifier.is_output_valid(
                pot_input,
                slots_between_blocks,
                proof_of_time,
                pot_parameters_change,
            ) {
                return Err(BeaconChainBlockVerificationError::InvalidProofOfTime);
            }
        }

        Ok(())
    }

    fn check_body(
        &self,
        block_number: BlockNumber,
        own_segment_roots: &[SegmentRoot],
        _intermediate_shard_blocks: &IntermediateShardBlocksInfo<'_>,
    ) -> Result<(), BlockVerificationError> {
        let expected_segment_headers = self
            .segment_headers_store
            .segment_headers_for_block(block_number);
        let correct_segment_roots = expected_segment_headers
            .iter()
            .map(|segment_header| &segment_header.segment_root)
            .eq(own_segment_roots);
        if !correct_segment_roots {
            return Err(BlockVerificationError::InvalidOwnSegmentRoots {
                expected: expected_segment_headers
                    .iter()
                    .map(|segment_header| segment_header.segment_root)
                    .collect(),
                actual: own_segment_roots.to_vec(),
            });
        }

        // TODO: check intermediate shard blocks

        Ok(())
    }

    async fn verify(
        &self,
        parent_header: &BeaconChainHeader<'_>,
        parent_block_mmr_root: &Blake3Hash,
        header: &BeaconChainHeader<'_>,
        body: &BeaconChainBody<'_>,
        _origin: BlockOrigin,
    ) -> Result<(), BlockVerificationError> {
        trace!(header = ?header, "Verifying");

        let parent_block_root = parent_header.root();

        let block_number = header.prefix.number;
        let consensus_info = header.consensus_info;
        let consensus_parameters = header.consensus_parameters();
        let slot = consensus_info.slot;

        let best_header = self.chain_info.best_header();
        let best_header = best_header.header();
        let best_number = best_header.prefix.number;

        // Reject block below archiving point
        if block_number + self.consensus_constants.confirmation_depth_k < best_number {
            debug!(
                ?header,
                %best_number,
                "Rejecting block below archiving point"
            );

            return Err(BlockVerificationError::BelowArchivingPoint);
        }

        self.check_header_prefix(parent_header.prefix, parent_block_mmr_root, header.prefix)?;

        self.check_consensus_parameters(&parent_block_root, parent_header, header)?;

        if !header.is_sealed_correctly() {
            return Err(BlockVerificationError::InvalidSeal);
        }

        // Verify that solution is valid
        consensus_info
            .solution
            .verify::<PosTable>(
                slot,
                &SolutionVerifyParams {
                    proof_of_time: consensus_info.proof_of_time,
                    solution_range: consensus_parameters.fixed_parameters.solution_range,
                    // TODO: Piece check parameters
                    piece_check_params: None,
                },
            )
            .map_err(BeaconChainBlockVerificationError::from)?;

        Self::check_proof_of_time(
            &self.pot_verifier,
            self.consensus_constants.block_authoring_delay,
            parent_header.consensus_info.slot,
            parent_header.consensus_info.proof_of_time,
            parent_header.consensus_info.future_proof_of_time,
            parent_header.consensus_parameters(),
            consensus_info.slot,
            consensus_info.proof_of_time,
            consensus_info.future_proof_of_time,
            consensus_parameters,
            body.pot_checkpoints(),
            self.full_pot_verification(block_number),
        )?;

        self.check_body(
            block_number,
            body.own_segment_roots(),
            body.intermediate_shard_blocks(),
        )?;

        // TODO: Do something about equivocation?

        Ok(())
    }
}
