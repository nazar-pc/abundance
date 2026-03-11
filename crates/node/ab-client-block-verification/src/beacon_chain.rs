use crate::{BlockVerification, BlockVerificationError, GenericBody, GenericHeader};
use ab_client_api::{BeaconChainInfo, BlockOrigin, ChainSyncStatus};
use ab_client_consensus_common::ConsensusConstants;
use ab_client_consensus_common::consensus_parameters::{
    DeriveConsensusParametersChainInfo, DeriveConsensusParametersError,
    DeriveSuperSegmentForBlockError, ShardMembershipEntropySourceChainInfo,
    derive_consensus_parameters, derive_super_segments_for_block, shard_membership_entropy_source,
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
use ab_core_primitives::segments::{
    HistorySize, SuperSegment, SuperSegmentIndex, SuperSegmentRoot,
};
use ab_core_primitives::shard::ShardIndex;
use ab_core_primitives::solutions::{
    SolutionVerifyError, SolutionVerifyPieceParams, SolutionVerifyStatelessParams,
};
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
    /// Super segment derivation error
    #[error("Super segment derivation error: {error}")]
    SuperSegmentDerivation {
        /// Super segment derivation error
        #[from]
        error: DeriveSuperSegmentForBlockError,
    },
    /// Invalid consensus parameters
    #[error("Invalid consensus parameters: expected {expected:?}, actual {actual:?}")]
    InvalidConsensusParameters {
        /// Expected consensus parameters
        expected: Box<OwnedBlockHeaderConsensusParameters>,
        /// Actual consensus parameters
        actual: Box<OwnedBlockHeaderConsensusParameters>,
    },
    /// Missing a super segment in the first block
    #[error("Missing super segment in the first block")]
    MissingSuperSegmentInFirstBlock,
    /// Previous super segment header not found
    #[error("Previous super segment header not found")]
    PreviousSuperSegmentHeaderNotFound,
    /// Solution super segment not found
    #[error("Solution super segment {index} not found")]
    SolutionSuperSegmentNotFound {
        /// Expected super segment index
        index: SuperSegmentIndex,
    },
    /// Invalid super segment root
    #[error("Invalid super segment root: expected {expected:?}, actual {actual:?}")]
    InvalidSuperSegmentRoot {
        /// Expected super segment root
        expected: Box<Option<SuperSegmentRoot>>,
        /// Actual super segment root
        actual: Box<Option<SuperSegmentRoot>>,
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

impl<PosTable, CI, CSS> BlockVerification<OwnedBeaconChainBlock, Option<SuperSegment>>
    for BeaconChainBlockVerification<PosTable, CI, CSS>
where
    PosTable: Table,
    CI: BeaconChainInfo,
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
    ) -> Result<Option<SuperSegment>, BlockVerificationError> {
        self.verify_sequential(parent_header, parent_block_mmr_root, header, body, origin)
            .await
    }
}

impl<PosTable, CI, CSS> BeaconChainBlockVerification<PosTable, CI, CSS>
where
    PosTable: Table,
    CI: BeaconChainInfo,
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
        let diff = u64::from(diff);

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

    fn check_consensus_parameters_concurrent<BCI>(
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
            // TODO: This field is verified separately in the sequential part
            super_segment_root: header.consensus_parameters().super_segment_root.copied(),
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
        if !(u64::from(slots_between_blocks) == checkpoints.len() as u64
            || (parent_slot == SlotNumber::ZERO
                && u64::from(future_slot) == checkpoints.len() as u64))
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

        // TODO: check intermediate shard blocks and all segment roots included in the body

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
        if block_number + self.consensus_constants.block_confirmation_depth < best_number {
            debug!(
                ?header,
                %best_number,
                "Rejecting a block below the archiving point"
            );

            return Err(BlockVerificationError::BelowArchivingPoint);
        }

        self.check_header_prefix(parent_header.prefix, parent_block_mmr_root, header.prefix)?;

        self.check_consensus_parameters_concurrent(
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

        // Verify that the solution is valid (stateless half)
        consensus_info
            .solution
            .verify_stateless::<PosTable>(
                slot,
                &SolutionVerifyStatelessParams {
                    shard_index: ShardIndex::BEACON_CHAIN,
                    proof_of_time: consensus_info.proof_of_time,
                    solution_range: consensus_parameters.fixed_parameters.solution_range,
                    shard_membership_entropy,
                    num_shards: consensus_parameters.fixed_parameters.num_shards,
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
        parent_header: &BeaconChainHeader<'_>,
        // TODO: Probably remove unused arguments
        _parent_block_mmr_root: &Blake3Hash,
        header: &BeaconChainHeader<'_>,
        body: &BeaconChainBody<'_>,
        _origin: &BlockOrigin,
    ) -> Result<Option<SuperSegment>, BlockVerificationError> {
        trace!(header = ?header, "Verify sequential");

        let block_number = header.prefix.number;
        let consensus_info = header.consensus_info;

        let best_header = self.chain_info.best_header();
        let best_header = best_header.header();
        let best_number = best_header.prefix.number;

        // Reject block below archiving point
        if block_number + self.consensus_constants.block_confirmation_depth < best_number {
            debug!(
                ?header,
                %best_number,
                "Rejecting a block below the archiving point"
            );

            return Err(BlockVerificationError::BelowArchivingPoint);
        }

        let maybe_super_segment = derive_super_segments_for_block(
            &self.chain_info,
            parent_header.prefix.number,
            self.consensus_constants.block_confirmation_depth,
            self.consensus_constants.shard_confirmation_depth,
        )
        .map_err(BeaconChainBlockVerificationError::from)?;
        let maybe_super_segment_root = maybe_super_segment
            .as_ref()
            .map(|super_segment| super_segment.header.root);

        if maybe_super_segment_root.as_ref() != header.consensus_parameters().super_segment_root {
            return Err(BlockVerificationError::from(
                BeaconChainBlockVerificationError::InvalidSuperSegmentRoot {
                    expected: Box::new(maybe_super_segment_root),
                    actual: Box::new(header.consensus_parameters().super_segment_root.copied()),
                },
            ));
        }

        // Verify that the solution is valid (piece verification half)
        {
            let (current_history_size, solution_num_segments, solution_super_segment_root) =
                if block_number == BlockNumber::ONE {
                    let latest_super_segment = maybe_super_segment.as_ref().ok_or(
                        BeaconChainBlockVerificationError::MissingSuperSegmentInFirstBlock,
                    )?;

                    (
                        HistorySize::ONE,
                        latest_super_segment.header.num_segments,
                        latest_super_segment.header.root,
                    )
                } else {
                    let max_segment_index = self
                        .chain_info
                        .previous_super_segment_header(block_number)
                        .ok_or(
                            BeaconChainBlockVerificationError::PreviousSuperSegmentHeaderNotFound,
                        )?
                        .max_segment_index
                        .as_inner();
                    let current_history_size = HistorySize::from(max_segment_index);

                    let solution_super_segment_header = self
                        .chain_info
                        .get_super_segment_header(consensus_info.solution.piece_super_segment_index)
                        .ok_or(
                            BeaconChainBlockVerificationError::SolutionSuperSegmentNotFound {
                                index: consensus_info.solution.piece_super_segment_index,
                            },
                        )?;

                    (
                        current_history_size,
                        solution_super_segment_header.num_segments,
                        solution_super_segment_header.root,
                    )
                };
            // TODO: Unlock this once farmer has better access to super segments and replace history
            //  size with super segment index in the solution
            // let sector_expiration_check_segment_root = self
            //     .chain_info
            //     .get_segment_header(
            //         consensus_info
            //             .solution
            //             .history_size
            //             .sector_expiration_check(self.consensus_constants.min_sector_lifetime)
            //             .ok_or(BeaconChainBlockVerificationError::InvalidHistorySize {
            //                 history_size: consensus_info.solution.history_size,
            //                 current_history_size,
            //             })?
            //             .segment_index(),
            //     )
            //     .map(|segment_header| segment_header.segment_root);

            consensus_info
                .solution
                .verify_piece(&SolutionVerifyPieceParams {
                    // TODO: Query it from an actual chain
                    max_pieces_in_sector: 1000,
                    super_segment_root: solution_super_segment_root,
                    num_segments: solution_num_segments,
                    recent_segments: self.consensus_constants.recent_segments,
                    recent_history_fraction: self.consensus_constants.recent_history_fraction,
                    min_sector_lifetime: self.consensus_constants.min_sector_lifetime,
                    current_history_size,
                    // TODO: Expiration check
                    sector_expiration_check_segment_root: None,
                })
                .map_err(BeaconChainBlockVerificationError::from)?;
        }

        self.check_body(
            block_number,
            body.own_segments(),
            body.intermediate_shard_blocks(),
        )?;

        Ok(maybe_super_segment)
    }
}
