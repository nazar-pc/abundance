use crate::{BlockVerification, BlockVerificationError, GenericBody, GenericHeader};
use ab_client_api::{BlockOrigin, ChainInfo, ChainSyncStatus};
use ab_client_consensus_common::ConsensusConstants;
use ab_client_consensus_common::consensus_parameters::{
    DeriveConsensusParametersChainInfo, DeriveConsensusParametersError,
    ShardMembershipEntropySourceChainInfo, derive_consensus_parameters,
    shard_membership_entropy_source,
};
use ab_client_proof_of_time::PotNextSlotInput;
use ab_client_proof_of_time::verifier::PotVerifier;
use ab_core_primitives::block::body::{BeaconChainBody, IntermediateShardBlocksInfo, OwnSegments};
use ab_core_primitives::block::header::{
    BeaconChainHeader, BlockHeaderConsensusParameters, BlockHeaderPrefix,
    OwnedBlockHeaderConsensusParameters,
};
use ab_core_primitives::block::owned::OwnedBeaconChainBlock;
use ab_core_primitives::block::{BlockNumber, BlockRoot, BlockTimestamp};
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pot::{PotCheckpoints, PotOutput, PotParametersChange, SlotNumber};
use ab_core_primitives::shard::ShardIndex;
use ab_core_primitives::solutions::{SolutionVerifyError, SolutionVerifyParams};
use ab_proof_of_space::Table;
use rand::prelude::*;
use rayon::prelude::*;
use std::iter;
use std::marker::PhantomData;
use std::time::SystemTime;
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
    #[error("Invalid consensus parameters: expected {expected:?}, actual {actual:?}")]
    InvalidConsensusParameters {
        /// Expected consensus parameters
        expected: Box<OwnedBlockHeaderConsensusParameters>,
        /// Actual consensus parameters
        actual: Box<OwnedBlockHeaderConsensusParameters>,
    },
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
    async fn verify_concurrent<BCI>(
        &self,
        parent_header: &GenericHeader<'_, OwnedBeaconChainBlock>,
        parent_block_mmr_root: &Blake3Hash,
        header: &GenericHeader<'_, OwnedBeaconChainBlock>,
        body: &GenericBody<'_, OwnedBeaconChainBlock>,
        origin: &BlockOrigin,
        beacon_chain_info: &BCI,
    ) -> Result<(), BlockVerificationError>
    where
        BCI: DeriveConsensusParametersChainInfo + ShardMembershipEntropySourceChainInfo,
    {
        self.verify_concurrent(
            parent_header,
            parent_block_mmr_root,
            header,
            body,
            origin,
            beacon_chain_info,
        )
        .await
    }

    #[inline(always)]
    async fn verify_sequential(
        &self,
        parent_header: &GenericHeader<'_, OwnedBeaconChainBlock>,
        parent_block_mmr_root: &Blake3Hash,
        header: &GenericHeader<'_, OwnedBeaconChainBlock>,
        body: &GenericBody<'_, OwnedBeaconChainBlock>,
        origin: &BlockOrigin,
    ) -> Result<(), BlockVerificationError> {
        self.verify_sequential(parent_header, parent_block_mmr_root, header, body, origin)
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
        consensus_constants: ConsensusConstants,
        pot_verifier: PotVerifier,
        chain_info: CI,
        chain_sync_status: CSS,
    ) -> Self {
        Self {
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
        let basic_valid = header_prefix.number == parent_header_prefix.number + BlockNumber::ONE
            && header_prefix.shard_index == parent_header_prefix.shard_index
            && &header_prefix.mmr_root == parent_block_mmr_root
            && header_prefix.timestamp > parent_header_prefix.timestamp;

        if !basic_valid {
            return Err(BlockVerificationError::InvalidHeaderPrefix);
        }

        let timestamp_now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let timestamp_now =
            BlockTimestamp::from_millis(u64::try_from(timestamp_now).unwrap_or(u64::MAX));

        if header_prefix.timestamp
            > timestamp_now.saturating_add(self.consensus_constants.max_block_timestamp_drift)
        {
            return Err(BlockVerificationError::TimestampTooFarInTheFuture);
        }

        Ok(())
    }

    fn check_consensus_parameters<BCI>(
        &self,
        parent_block_root: &BlockRoot,
        parent_header: &BeaconChainHeader<'_>,
        header: &BeaconChainHeader<'_>,
        beacon_chain_info: &BCI,
    ) -> Result<(), BeaconChainBlockVerificationError>
    where
        BCI: DeriveConsensusParametersChainInfo,
    {
        let derived_consensus_parameters = derive_consensus_parameters(
            &self.consensus_constants,
            beacon_chain_info,
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
            return Err(
                BeaconChainBlockVerificationError::InvalidConsensusParameters {
                    expected: Box::new(expected_consensus_parameters),
                    actual: Box::new(OwnedBlockHeaderConsensusParameters {
                        fixed_parameters: header.consensus_parameters().fixed_parameters,
                        super_segment_root: header
                            .consensus_parameters()
                            .super_segment_root
                            .copied(),
                        next_solution_range: header.consensus_parameters().next_solution_range,
                        pot_parameters_change: header
                            .consensus_parameters()
                            .pot_parameters_change
                            .copied(),
                    }),
                },
            );
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
        checkpoints: &[PotCheckpoints],
        verify_checkpoints: bool,
    ) -> Result<(), BeaconChainBlockVerificationError> {
        let parent_pot_parameters_change = parent_consensus_parameters
            .pot_parameters_change
            .copied()
            .map(PotParametersChange::from);

        // The last checkpoint must be the future proof of time
        if checkpoints.last().map(PotCheckpoints::output) != Some(future_proof_of_time) {
            return Err(BeaconChainBlockVerificationError::InvalidPotCheckpoints);
        }

        let future_slot = slot + block_authoring_delay;
        let parent_future_slot = if parent_slot == SlotNumber::ZERO {
            parent_slot
        } else {
            parent_slot + block_authoring_delay
        };

        let slots_between_blocks = slot
            .checked_sub(parent_slot)
            .ok_or(BeaconChainBlockVerificationError::InvalidPotCheckpoints)?;
        // The number of checkpoints must match the difference between parent's and this block's
        // future slots. This also implicitly checks that there is a non-zero number of slots
        // between this and parent block because the list of checkpoints is already known to be not
        // empty from the check above.
        //
        // The first block after genesis is a special case and is handled separately here.
        if !(slots_between_blocks.as_u64() == checkpoints.len() as u64
            || (parent_slot == SlotNumber::ZERO
                && future_slot.as_u64() == checkpoints.len() as u64))
        {
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
            // Derive inputs to the slot, which follows the parent future slot
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
                    &parent_pot_parameters_change,
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
                // Calculate slot iterations as of the parent slot
                let slot_iterations = parent_pot_parameters_change
                    .and_then(|parameters_change| {
                        (parameters_change.slot <= parent_slot)
                            .then_some(parameters_change.slot_iterations)
                    })
                    .unwrap_or(parent_consensus_parameters.fixed_parameters.slot_iterations);
                // Derive inputs to the slot, which follows the parent slot
                PotNextSlotInput::derive(
                    slot_iterations,
                    parent_slot,
                    parent_proof_of_time,
                    &parent_pot_parameters_change,
                )
            };

            if !pot_verifier.is_output_valid(
                pot_input,
                slots_between_blocks,
                proof_of_time,
                parent_pot_parameters_change,
            ) {
                return Err(BeaconChainBlockVerificationError::InvalidProofOfTime);
            }
        }

        Ok(())
    }

    fn check_body(
        &self,
        block_number: BlockNumber,
        own_segments: Option<OwnSegments<'_>>,
        _intermediate_shard_blocks: &IntermediateShardBlocksInfo<'_>,
    ) -> Result<(), BlockVerificationError> {
        let expected_segment_headers = self.chain_info.segment_headers_for_block(block_number);
        let expected_first_local_segment_index = expected_segment_headers
            .first()
            .map(|segment_header| segment_header.segment_index.as_inner());
        let correct_first_local_segment_index = expected_first_local_segment_index
            == own_segments
                .as_ref()
                .map(|own_segments| own_segments.first_local_segment_index);
        let correct_segment_roots = expected_segment_headers
            .iter()
            .map(|segment_header| &segment_header.segment_root)
            .eq(own_segments
                .as_ref()
                .map(|own_segments| own_segments.segment_roots)
                .unwrap_or_default());
        if !(correct_first_local_segment_index && correct_segment_roots) {
            return Err(BlockVerificationError::InvalidOwnSegments {
                expected_first_local_segment_index,
                expected_segment_roots: expected_segment_headers
                    .iter()
                    .map(|segment_header| segment_header.segment_root)
                    .collect(),
                actual_first_local_segment_index: own_segments
                    .as_ref()
                    .map(|own_segments| own_segments.first_local_segment_index),
                actual_segment_roots: own_segments
                    .as_ref()
                    .map(|own_segments| own_segments.segment_roots.to_vec())
                    .unwrap_or_default(),
            });
        }

        // TODO: check intermediate shard blocks

        Ok(())
    }

    async fn verify_concurrent<BCI>(
        &self,
        parent_header: &BeaconChainHeader<'_>,
        parent_block_mmr_root: &Blake3Hash,
        header: &BeaconChainHeader<'_>,
        body: &BeaconChainBody<'_>,
        _origin: &BlockOrigin,
        beacon_chain_info: &BCI,
    ) -> Result<(), BlockVerificationError>
    where
        BCI: DeriveConsensusParametersChainInfo + ShardMembershipEntropySourceChainInfo,
    {
        trace!(header = ?header, "Verify concurrent");

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
                "Rejecting a block below the archiving point"
            );

            return Err(BlockVerificationError::BelowArchivingPoint);
        }

        self.check_header_prefix(parent_header.prefix, parent_block_mmr_root, header.prefix)?;

        self.check_consensus_parameters(
            &parent_block_root,
            parent_header,
            header,
            beacon_chain_info,
        )?;

        if !header.is_sealed_correctly() {
            return Err(BlockVerificationError::InvalidSeal);
        }

        // Find shard membership entropy for the slot
        let shard_membership_entropy = shard_membership_entropy_source(
            header.prefix.number,
            best_header,
            self.consensus_constants.shard_rotation_interval,
            self.consensus_constants.shard_rotation_delay,
            beacon_chain_info,
        )?;

        // Verify that the solution is valid
        consensus_info
            .solution
            .verify::<PosTable>(
                slot,
                &SolutionVerifyParams {
                    shard_index: ShardIndex::BEACON_CHAIN,
                    proof_of_time: consensus_info.proof_of_time,
                    solution_range: consensus_parameters.fixed_parameters.solution_range,
                    shard_membership_entropy,
                    num_shards: consensus_parameters.fixed_parameters.num_shards,
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
            body.pot_checkpoints(),
            self.full_pot_verification(block_number),
        )?;

        // TODO: Do something about equivocation?

        Ok(())
    }

    async fn verify_sequential(
        &self,
        // TODO: Probable remove these unused arguments
        _parent_header: &BeaconChainHeader<'_>,
        _parent_block_mmr_root: &Blake3Hash,
        header: &BeaconChainHeader<'_>,
        body: &BeaconChainBody<'_>,
        _origin: &BlockOrigin,
    ) -> Result<(), BlockVerificationError> {
        trace!(header = ?header, "Verify sequential");

        let block_number = header.prefix.number;

        let best_header = self.chain_info.best_header();
        let best_header = best_header.header();
        let best_number = best_header.prefix.number;

        // Reject block below archiving point
        if block_number + self.consensus_constants.confirmation_depth_k < best_number {
            debug!(
                ?header,
                %best_number,
                "Rejecting a block below the archiving point"
            );

            return Err(BlockVerificationError::BelowArchivingPoint);
        }

        self.check_body(
            block_number,
            body.own_segments(),
            body.intermediate_shard_blocks(),
        )?;

        Ok(())
    }
}
