//! Private implementation details of Subspace consensus digests.

use crate::{ConsensusLog, PotParametersChange, SUBSPACE_ENGINE_ID};
use ab_core_primitives::block::BlockNumber;
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pot::{PotOutput, SlotNumber};
use ab_core_primitives::segments::{SegmentIndex, SegmentRoot};
use ab_core_primitives::solutions::{Solution, SolutionRange};
use alloc::collections::btree_map::{BTreeMap, Entry};
use core::fmt;
use core::num::NonZeroU32;
use log::trace;
use parity_scale_codec::{Decode, Encode};
use sp_runtime::DigestItem;
use sp_runtime::traits::{Header as HeaderT, One, Zero};
use subspace_verification::sr25519::RewardSignature;

/// A Subspace pre-runtime digest. This contains all data required to validate a block and for the
/// Subspace runtime module.
#[derive(Debug, Clone, Encode, Decode)]
pub struct PreDigest {
    /// Slot number
    pub slot: SlotNumber,
    /// Solution
    pub solution: Solution,
    /// Proof of time information
    pub pot_info: PreDigestPotInfo,
}

/// Proof of time information in pre-digest
#[derive(Debug, Clone, Encode, Decode)]
pub enum PreDigestPotInfo {
    /// Initial version of proof of time information
    #[codec(index = 0)]
    V0 {
        /// Proof of time for this slot
        proof_of_time: PotOutput,
        /// Future proof of time
        future_proof_of_time: PotOutput,
    },
}

impl PreDigestPotInfo {
    /// Proof of time for this slot
    #[inline]
    pub fn proof_of_time(&self) -> PotOutput {
        let Self::V0 { proof_of_time, .. } = self;
        *proof_of_time
    }

    /// Future proof of time
    #[inline]
    pub fn future_proof_of_time(&self) -> PotOutput {
        let Self::V0 {
            future_proof_of_time,
            ..
        } = self;
        *future_proof_of_time
    }
}

/// A digest item which is usable with Subspace consensus.
pub trait CompatibleDigestItem: Sized {
    /// Construct a digest item which contains a Subspace pre-digest.
    fn subspace_pre_digest(pre_digest: &PreDigest) -> Self;

    /// If this item is an Subspace pre-digest, return it.
    fn as_subspace_pre_digest(&self) -> Option<PreDigest>;

    /// Construct a digest item which contains a Subspace seal.
    fn subspace_seal(signature: RewardSignature) -> Self;

    /// If this item is a Subspace signature, return the signature.
    fn as_subspace_seal(&self) -> Option<RewardSignature>;

    /// Number of iterations for proof of time per slot, corresponds to slot that directly follows
    /// parent block's slot and can change before slot for which block is produced
    fn pot_slot_iterations(pot_slot_iterations: NonZeroU32) -> Self;

    /// If this item is a Subspace proof of time slot iterations, return it.
    fn as_pot_slot_iterations(&self) -> Option<NonZeroU32>;

    /// Construct a digest item which contains a solution range.
    fn solution_range(solution_range: SolutionRange) -> Self;

    /// If this item is a Subspace solution range, return it.
    fn as_solution_range(&self) -> Option<SolutionRange>;

    /// Change of parameters to apply to PoT chain
    fn pot_parameters_change(pot_parameters_change: PotParametersChange) -> Self;

    /// If this item is a Subspace proof of time change of parameters, return it.
    fn as_pot_parameters_change(&self) -> Option<PotParametersChange>;

    /// Construct a digest item which contains next solution range.
    fn next_solution_range(solution_range: SolutionRange) -> Self;

    /// If this item is a Subspace next solution range, return it.
    fn as_next_solution_range(&self) -> Option<SolutionRange>;

    /// Construct a digest item which contains segment root.
    fn segment_root(segment_index: SegmentIndex, segment_root: SegmentRoot) -> Self;

    /// If this item is a Subspace segment root, return it.
    fn as_segment_root(&self) -> Option<(SegmentIndex, SegmentRoot)>;

    /// Construct digest item than indicates enabling of solution range adjustment and override next
    /// solution range.
    fn enable_solution_range_adjustment_and_override(
        override_solution_range: Option<SolutionRange>,
    ) -> Self;

    /// If this item is a Subspace Enable solution range adjustment and override next solution
    /// range, return it.
    fn as_enable_solution_range_adjustment_and_override(&self) -> Option<Option<SolutionRange>>;

    /// Construct digest item that indicates update of root plot public key.
    fn root_plot_public_key_hash_update(root_plot_public_key: Option<Blake3Hash>) -> Self;

    /// If this item is a Subspace update of root plot public key, return it.
    fn as_root_plot_public_key_hash_update(&self) -> Option<Option<Blake3Hash>>;
}

impl CompatibleDigestItem for DigestItem {
    fn subspace_pre_digest(pre_digest: &PreDigest) -> Self {
        Self::PreRuntime(SUBSPACE_ENGINE_ID, pre_digest.encode())
    }

    fn as_subspace_pre_digest(&self) -> Option<PreDigest> {
        self.pre_runtime_try_to(&SUBSPACE_ENGINE_ID)
    }

    fn subspace_seal(signature: RewardSignature) -> Self {
        Self::Seal(SUBSPACE_ENGINE_ID, signature.encode())
    }

    fn as_subspace_seal(&self) -> Option<RewardSignature> {
        self.seal_try_to(&SUBSPACE_ENGINE_ID)
    }

    fn pot_slot_iterations(pot_slot_iterations: NonZeroU32) -> Self {
        Self::Consensus(
            SUBSPACE_ENGINE_ID,
            ConsensusLog::PotSlotIterations(pot_slot_iterations).encode(),
        )
    }

    fn as_pot_slot_iterations(&self) -> Option<NonZeroU32> {
        self.consensus_try_to(&SUBSPACE_ENGINE_ID).and_then(|c| {
            if let ConsensusLog::PotSlotIterations(pot_slot_iterations) = c {
                Some(pot_slot_iterations)
            } else {
                None
            }
        })
    }

    fn solution_range(solution_range: SolutionRange) -> Self {
        Self::Consensus(
            SUBSPACE_ENGINE_ID,
            ConsensusLog::SolutionRange(solution_range).encode(),
        )
    }

    fn as_solution_range(&self) -> Option<SolutionRange> {
        self.consensus_try_to(&SUBSPACE_ENGINE_ID).and_then(|c| {
            if let ConsensusLog::SolutionRange(solution_range) = c {
                Some(solution_range)
            } else {
                None
            }
        })
    }

    fn pot_parameters_change(pot_parameters_change: PotParametersChange) -> Self {
        Self::Consensus(
            SUBSPACE_ENGINE_ID,
            ConsensusLog::PotParametersChange(pot_parameters_change).encode(),
        )
    }

    fn as_pot_parameters_change(&self) -> Option<PotParametersChange> {
        self.consensus_try_to(&SUBSPACE_ENGINE_ID).and_then(|c| {
            if let ConsensusLog::PotParametersChange(pot_parameters_change) = c {
                Some(pot_parameters_change)
            } else {
                None
            }
        })
    }

    fn next_solution_range(solution_range: SolutionRange) -> Self {
        Self::Consensus(
            SUBSPACE_ENGINE_ID,
            ConsensusLog::NextSolutionRange(solution_range).encode(),
        )
    }

    fn as_next_solution_range(&self) -> Option<SolutionRange> {
        self.consensus_try_to(&SUBSPACE_ENGINE_ID).and_then(|c| {
            if let ConsensusLog::NextSolutionRange(solution_range) = c {
                Some(solution_range)
            } else {
                None
            }
        })
    }

    fn segment_root(segment_index: SegmentIndex, segment_root: SegmentRoot) -> Self {
        Self::Consensus(
            SUBSPACE_ENGINE_ID,
            ConsensusLog::SegmentRoot((segment_index, segment_root)).encode(),
        )
    }

    fn as_segment_root(&self) -> Option<(SegmentIndex, SegmentRoot)> {
        self.consensus_try_to(&SUBSPACE_ENGINE_ID).and_then(|c| {
            if let ConsensusLog::SegmentRoot(segment_root) = c {
                Some(segment_root)
            } else {
                None
            }
        })
    }

    fn enable_solution_range_adjustment_and_override(
        maybe_override_solution_range: Option<SolutionRange>,
    ) -> Self {
        Self::Consensus(
            SUBSPACE_ENGINE_ID,
            ConsensusLog::EnableSolutionRangeAdjustmentAndOverride(maybe_override_solution_range)
                .encode(),
        )
    }

    fn as_enable_solution_range_adjustment_and_override(&self) -> Option<Option<SolutionRange>> {
        self.consensus_try_to(&SUBSPACE_ENGINE_ID).and_then(|c| {
            if let ConsensusLog::EnableSolutionRangeAdjustmentAndOverride(
                maybe_override_solution_range,
            ) = c
            {
                Some(maybe_override_solution_range)
            } else {
                None
            }
        })
    }

    fn root_plot_public_key_hash_update(root_plot_public_key_hash: Option<Blake3Hash>) -> Self {
        Self::Consensus(
            SUBSPACE_ENGINE_ID,
            ConsensusLog::RootPlotPublicKeyHashUpdate(root_plot_public_key_hash).encode(),
        )
    }

    fn as_root_plot_public_key_hash_update(&self) -> Option<Option<Blake3Hash>> {
        self.consensus_try_to(&SUBSPACE_ENGINE_ID).and_then(|c| {
            if let ConsensusLog::RootPlotPublicKeyHashUpdate(root_plot_public_key_hash) = c {
                Some(root_plot_public_key_hash)
            } else {
                None
            }
        })
    }
}

/// Various kinds of digest types used in errors
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ErrorDigestType {
    /// Pre-digest
    PreDigest,
    /// Seal (signature)
    Seal,
    /// Number of iterations for proof of time per slot
    PotSlotIterations,
    /// Solution range
    SolutionRange,
    /// Change of parameters to apply to PoT chain
    PotParametersChange,
    /// Next solution range
    NextSolutionRange,
    /// Segment root
    SegmentRoot,
    /// Generic consensus
    Consensus,
    /// Enable solution range adjustment and override solution range
    EnableSolutionRangeAdjustmentAndOverride,
    /// Root plot public key was updated
    RootPlotPublicKeyUpdate,
}

impl fmt::Display for ErrorDigestType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorDigestType::PreDigest => {
                write!(f, "PreDigest")
            }
            ErrorDigestType::Seal => {
                write!(f, "Seal")
            }
            ErrorDigestType::PotSlotIterations => {
                write!(f, "PotSlotIterations")
            }
            ErrorDigestType::SolutionRange => {
                write!(f, "SolutionRange")
            }
            ErrorDigestType::PotParametersChange => {
                write!(f, "PotParametersChange")
            }
            ErrorDigestType::NextSolutionRange => {
                write!(f, "NextSolutionRange")
            }
            ErrorDigestType::SegmentRoot => {
                write!(f, "SegmentRoot")
            }
            ErrorDigestType::Consensus => {
                write!(f, "Consensus")
            }
            ErrorDigestType::EnableSolutionRangeAdjustmentAndOverride => {
                write!(f, "EnableSolutionRangeAdjustmentAndOverride")
            }
            ErrorDigestType::RootPlotPublicKeyUpdate => {
                write!(f, "RootPlotPublicKeyUpdate")
            }
        }
    }
}

/// Digest error
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// Subspace digest missing
    #[error("Subspace {0} digest not found")]
    Missing(ErrorDigestType),
    /// Failed to decode Subspace digest
    #[error("Failed to decode Subspace {0} digest: {1}")]
    FailedToDecode(ErrorDigestType, parity_scale_codec::Error),
    /// Duplicate Subspace digests
    #[error("Duplicate Subspace {0} digests, rejecting!")]
    Duplicate(ErrorDigestType),

    /// Error when deriving next digests
    #[error("Failed to derive next {0} digest, rejecting!")]
    NextDigestDerivationError(ErrorDigestType),

    /// Error when verifying next digests
    #[error("Failed to verify next {0} digest, rejecting!")]
    NextDigestVerificationError(ErrorDigestType),
}

#[cfg(feature = "std")]
impl From<Error> for String {
    #[inline]
    fn from(error: Error) -> String {
        error.to_string()
    }
}

/// Digest items extracted from a header into convenient form
#[derive(Debug)]
pub struct SubspaceDigestItems {
    /// Pre-runtime digest
    pub pre_digest: PreDigest,
    /// Signature (seal) if present
    pub signature: Option<RewardSignature>,
    /// Number of iterations for proof of time per slot, corresponds to slot that directly follows
    /// parent block's slot and can change before slot for which block is produced
    pub pot_slot_iterations: NonZeroU32,
    /// Solution range
    pub solution_range: SolutionRange,
    /// Change of parameters to apply to PoT chain
    pub pot_parameters_change: Option<PotParametersChange>,
    /// Next solution range
    pub next_solution_range: Option<SolutionRange>,
    /// Segment roots
    pub segment_roots: BTreeMap<SegmentIndex, SegmentRoot>,
    /// Enable solution range adjustment and Override solution range
    pub enable_solution_range_adjustment_and_override: Option<Option<SolutionRange>>,
    /// Root plot public key was updated
    pub root_plot_public_key_hash_update: Option<Option<Blake3Hash>>,
}

/// Extract the Subspace global randomness from the given header.
pub fn extract_subspace_digest_items<Header>(header: &Header) -> Result<SubspaceDigestItems, Error>
where
    Header: HeaderT,
{
    let mut maybe_pre_digest = None;
    let mut maybe_seal = None;
    let mut maybe_pot_slot_iterations = None;
    let mut maybe_solution_range = None;
    let mut maybe_pot_parameters_change = None;
    let mut maybe_next_solution_range = None;
    let mut segment_roots = BTreeMap::new();
    let mut maybe_enable_and_override_solution_range = None;
    let mut maybe_root_plot_public_key_hash_update = None;

    for log in header.digest().logs() {
        match log {
            DigestItem::PreRuntime(id, data) => {
                if id != &SUBSPACE_ENGINE_ID {
                    continue;
                }

                let pre_digest = PreDigest::decode(&mut data.as_slice())
                    .map_err(|error| Error::FailedToDecode(ErrorDigestType::PreDigest, error))?;

                match maybe_pre_digest {
                    Some(_) => {
                        return Err(Error::Duplicate(ErrorDigestType::PreDigest));
                    }
                    None => {
                        maybe_pre_digest.replace(pre_digest);
                    }
                }
            }
            DigestItem::Consensus(id, data) => {
                if id != &SUBSPACE_ENGINE_ID {
                    continue;
                }

                let consensus = ConsensusLog::decode(&mut data.as_slice())
                    .map_err(|error| Error::FailedToDecode(ErrorDigestType::Consensus, error))?;

                match consensus {
                    ConsensusLog::PotSlotIterations(pot_slot_iterations) => {
                        match maybe_pot_slot_iterations {
                            Some(_) => {
                                return Err(Error::Duplicate(ErrorDigestType::PotSlotIterations));
                            }
                            None => {
                                maybe_pot_slot_iterations.replace(pot_slot_iterations);
                            }
                        }
                    }
                    ConsensusLog::SolutionRange(solution_range) => match maybe_solution_range {
                        Some(_) => {
                            return Err(Error::Duplicate(ErrorDigestType::SolutionRange));
                        }
                        None => {
                            maybe_solution_range.replace(solution_range);
                        }
                    },
                    ConsensusLog::PotParametersChange(pot_parameters_change) => {
                        match maybe_pot_parameters_change {
                            Some(_) => {
                                return Err(Error::Duplicate(ErrorDigestType::PotParametersChange));
                            }
                            None => {
                                maybe_pot_parameters_change.replace(pot_parameters_change);
                            }
                        }
                    }
                    ConsensusLog::NextSolutionRange(solution_range) => {
                        match maybe_next_solution_range {
                            Some(_) => {
                                return Err(Error::Duplicate(ErrorDigestType::NextSolutionRange));
                            }
                            None => {
                                maybe_next_solution_range.replace(solution_range);
                            }
                        }
                    }
                    ConsensusLog::SegmentRoot((segment_index, segment_root)) => {
                        if let Entry::Vacant(entry) = segment_roots.entry(segment_index) {
                            entry.insert(segment_root);
                        } else {
                            return Err(Error::Duplicate(ErrorDigestType::SegmentRoot));
                        }
                    }
                    ConsensusLog::EnableSolutionRangeAdjustmentAndOverride(
                        override_solution_range,
                    ) => match maybe_enable_and_override_solution_range {
                        Some(_) => {
                            return Err(Error::Duplicate(
                                ErrorDigestType::EnableSolutionRangeAdjustmentAndOverride,
                            ));
                        }
                        None => {
                            maybe_enable_and_override_solution_range
                                .replace(override_solution_range);
                        }
                    },
                    ConsensusLog::RootPlotPublicKeyHashUpdate(root_plot_public_key_update) => {
                        match maybe_root_plot_public_key_hash_update {
                            Some(_) => {
                                return Err(Error::Duplicate(
                                    ErrorDigestType::EnableSolutionRangeAdjustmentAndOverride,
                                ));
                            }
                            None => {
                                maybe_root_plot_public_key_hash_update
                                    .replace(root_plot_public_key_update);
                            }
                        }
                    }
                }
            }
            DigestItem::Seal(id, data) => {
                if id != &SUBSPACE_ENGINE_ID {
                    continue;
                }

                let seal = RewardSignature::decode(&mut data.as_slice())
                    .map_err(|error| Error::FailedToDecode(ErrorDigestType::Seal, error))?;

                match maybe_seal {
                    Some(_) => {
                        return Err(Error::Duplicate(ErrorDigestType::Seal));
                    }
                    None => {
                        maybe_seal.replace(seal);
                    }
                }
            }
            DigestItem::Other(_data) => {
                // Ignore
            }
            DigestItem::RuntimeEnvironmentUpdated => {
                // Ignore
            }
        }
    }

    Ok(SubspaceDigestItems {
        pre_digest: maybe_pre_digest.ok_or(Error::Missing(ErrorDigestType::PreDigest))?,
        signature: maybe_seal,
        pot_slot_iterations: maybe_pot_slot_iterations
            .ok_or(Error::Missing(ErrorDigestType::PotSlotIterations))?,
        solution_range: maybe_solution_range
            .ok_or(Error::Missing(ErrorDigestType::SolutionRange))?,
        pot_parameters_change: maybe_pot_parameters_change,
        next_solution_range: maybe_next_solution_range,
        segment_roots,
        enable_solution_range_adjustment_and_override: maybe_enable_and_override_solution_range,
        root_plot_public_key_hash_update: maybe_root_plot_public_key_hash_update,
    })
}

/// Extract the Subspace pre digest from the given header. Pre-runtime digests are mandatory, the
/// function will return `Err` if none is found.
pub fn extract_pre_digest<Header>(header: &Header) -> Result<PreDigest, Error>
where
    Header: HeaderT,
{
    // genesis block doesn't contain a pre digest so let's generate a
    // dummy one to not break any invariants in the rest of the code
    if header.number().is_zero() {
        return Ok(PreDigest {
            slot: SlotNumber::ZERO,
            solution: Solution::genesis_solution(),
            pot_info: PreDigestPotInfo::V0 {
                proof_of_time: Default::default(),
                future_proof_of_time: Default::default(),
            },
        });
    }

    let mut pre_digest = None;
    for log in header.digest().logs() {
        trace!(target: "subspace", "Checking log {:?}, looking for pre runtime digest", log);
        match (log.as_subspace_pre_digest(), pre_digest.is_some()) {
            (Some(_), true) => return Err(Error::Duplicate(ErrorDigestType::PreDigest)),
            (None, _) => trace!(target: "subspace", "Ignoring digest not meant for us"),
            (s, false) => pre_digest = s,
        }
    }
    pre_digest.ok_or(Error::Missing(ErrorDigestType::PreDigest))
}

type NumberOf<T> = <T as HeaderT>::Number;

/// Params used to derive the next solution range.
pub struct DeriveNextSolutionRangeParams<Header: HeaderT> {
    /// Current number of the block.
    pub number: NumberOf<Header>,
    /// Era duration of the chain.
    pub era_duration: NumberOf<Header>,
    /// Slot probability at which a block is produced.
    pub slot_probability: (u64, u64),
    /// Current slot of the block.
    pub current_slot: SlotNumber,
    /// Current solution range of the block.
    pub current_solution_range: SolutionRange,
    /// Slot at which era has begun.
    pub era_start_slot: SlotNumber,
    /// Flag to check if the next solution range should be adjusted.
    pub should_adjust_solution_range: bool,
    /// Solution range override that should be used instead of deriving from current.
    pub maybe_next_solution_range_override: Option<SolutionRange>,
}

/// Derives next solution range if era duration interval has met.
pub fn derive_next_solution_range<Header: HeaderT>(
    params: DeriveNextSolutionRangeParams<Header>,
) -> Result<Option<SolutionRange>, Error> {
    let DeriveNextSolutionRangeParams {
        number,
        era_duration,
        slot_probability,
        current_slot,
        current_solution_range,
        era_start_slot,
        should_adjust_solution_range,
        maybe_next_solution_range_override,
    } = params;

    if number.is_zero() || number % era_duration != Zero::zero() {
        return Ok(None);
    }

    // if the solution range should not be adjusted, return the current solution range
    let next_solution_range = if !should_adjust_solution_range {
        current_solution_range
    } else if let Some(solution_range_override) = maybe_next_solution_range_override {
        // era has change so take this override and reset it
        solution_range_override
    } else {
        current_solution_range.derive_next(
            era_start_slot,
            current_slot,
            slot_probability,
            BlockNumber::new(
                <Header::Number as TryInto<u64>>::try_into(era_duration)
                    .unwrap_or_else(|_| panic!("Era duration is always within u64; qed")),
            ),
        )
    };

    Ok(Some(next_solution_range))
}

/// Type that holds the parameters to derive and verify next digest items.
pub struct NextDigestsVerificationParams<'a, Header: HeaderT> {
    /// Header number for which we are verifying the digests.
    pub number: NumberOf<Header>,
    /// Digests present in the header that corresponds to number above.
    pub header_digests: &'a SubspaceDigestItems,
    /// Era duration at which solution range is updated.
    pub era_duration: NumberOf<Header>,
    /// Slot probability.
    pub slot_probability: (u64, u64),
    /// Current Era start slot.
    pub era_start_slot: SlotNumber,
    /// Should the solution range be adjusted on era change.
    /// If the digest logs indicate that solution range adjustment has been enabled, value is updated.
    pub should_adjust_solution_range: &'a mut bool,
    /// Next Solution range override.
    /// If the digest logs indicate that solution range override is provided, value is updated.
    pub maybe_next_solution_range_override: &'a mut Option<SolutionRange>,
    /// Root plot public key.
    /// Value is updated when digest items contain an update.
    pub maybe_root_plot_public_key_hash: &'a mut Option<Blake3Hash>,
}

/// Derives and verifies next digest items based on their respective intervals.
pub fn verify_next_digests<Header: HeaderT>(
    params: NextDigestsVerificationParams<Header>,
) -> Result<(), Error> {
    let NextDigestsVerificationParams {
        number,
        header_digests,
        era_duration,
        slot_probability,
        era_start_slot,
        should_adjust_solution_range,
        maybe_next_solution_range_override,
        maybe_root_plot_public_key_hash: root_plot_public_key_hash,
    } = params;

    // verify solution range adjustment and override
    // if the adjustment is already enabled, then error out
    if *should_adjust_solution_range
        && header_digests
            .enable_solution_range_adjustment_and_override
            .is_some()
    {
        return Err(Error::NextDigestVerificationError(
            ErrorDigestType::EnableSolutionRangeAdjustmentAndOverride,
        ));
    }

    if let Some(solution_range_override) =
        header_digests.enable_solution_range_adjustment_and_override
    {
        *should_adjust_solution_range = true;
        *maybe_next_solution_range_override = solution_range_override;
    }

    // verify if the solution range should be derived at this block header
    let expected_next_solution_range =
        derive_next_solution_range::<Header>(DeriveNextSolutionRangeParams {
            number,
            era_duration,
            slot_probability,
            current_slot: header_digests.pre_digest.slot,
            current_solution_range: header_digests.solution_range,
            era_start_slot,
            should_adjust_solution_range: *should_adjust_solution_range,
            maybe_next_solution_range_override: *maybe_next_solution_range_override,
        })?;

    if expected_next_solution_range.is_some() {
        // Whatever override we had, it is no longer necessary
        maybe_next_solution_range_override.take();
    }
    if expected_next_solution_range != header_digests.next_solution_range {
        return Err(Error::NextDigestVerificationError(
            ErrorDigestType::NextSolutionRange,
        ));
    }

    if let Some(updated_root_plot_public_key_hash) = header_digests.root_plot_public_key_hash_update
    {
        match updated_root_plot_public_key_hash {
            Some(updated_root_plot_public_key_hash) => {
                if number.is_one()
                    && root_plot_public_key_hash.is_none()
                    && header_digests.pre_digest.solution.public_key_hash
                        == updated_root_plot_public_key_hash
                {
                    root_plot_public_key_hash.replace(updated_root_plot_public_key_hash);
                } else {
                    return Err(Error::NextDigestVerificationError(
                        ErrorDigestType::RootPlotPublicKeyUpdate,
                    ));
                }
            }
            None => {
                root_plot_public_key_hash.take();
            }
        }
    }

    Ok(())
}
