#![cfg_attr(not(feature = "std"), no_std)]
#![feature(const_trait_impl, variant_count)]
#![expect(incomplete_features, reason = "generic_const_exprs")]
// TODO: This feature is not actually used in this crate, but is added as a workaround for
//  https://github.com/rust-lang/rust/issues/133199
#![feature(generic_const_exprs)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]
// TODO: remove when upstream issue is fixed
#![allow(
    non_camel_case_types,
    reason = "https://github.com/rust-lang/rust-analyzer/issues/16514"
)]

mod fees;
mod object_mapping;

extern crate alloc;

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

use crate::fees::{OnChargeTransaction, TransactionByteFee};
use crate::object_mapping::extract_block_object_mapping;
use alloc::borrow::Cow;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use core::num::NonZeroU64;
use frame_support::genesis_builder_helper::{build_state, get_preset};
use frame_support::inherent::ProvideInherent;
use frame_support::traits::{ConstU8, ConstU16, ConstU32, ConstU64, Everything};
use frame_support::weights::constants::ParityDbWeight;
use frame_support::weights::{ConstantMultiplier, Weight};
use frame_support::{construct_runtime, parameter_types};
use frame_system::limits::{BlockLength, BlockWeights};
use frame_system::pallet_prelude::RuntimeCallFor;
pub use pallet_subspace::AllowAuthoringBy;
use pallet_subspace::ConsensusConstants;
use sp_api::impl_runtime_apis;
use sp_consensus_subspace::{ChainConstants, PotParameters, SolutionRanges};
use sp_core::OpaqueMetadata;
use sp_runtime::traits::{AccountIdLookup, BlakeTwo256, Block as BlockT};
use sp_runtime::transaction_validity::{TransactionSource, TransactionValidity};
use sp_runtime::type_with_default::TypeWithDefault;
use sp_runtime::{ApplyExtrinsicResult, ExtrinsicInclusionMode, generic};
use sp_version::RuntimeVersion;
use static_assertions::const_assert;
use subspace_core_primitives::block::BlockNumber;
use subspace_core_primitives::hashes::Blake3Hash;
use subspace_core_primitives::objects::BlockObjectMapping;
use subspace_core_primitives::pieces::Piece;
use subspace_core_primitives::pot::{SlotDuration, SlotNumber};
use subspace_core_primitives::segments::{HistorySize, SegmentHeader, SegmentIndex, SegmentRoot};
use subspace_runtime_primitives::utility::{
    DefaultNonceProvider, MaybeNestedCall, MaybeUtilityCall,
};
use subspace_runtime_primitives::{
    AccountId, BLOCK_WEIGHT_FOR_2_SEC, Balance, ConsensusEventSegmentSize, Hash,
    MIN_REPLICATION_FACTOR, Moment, NORMAL_DISPATCH_RATIO, Nonce, SHANNON, SLOT_PROBABILITY,
    Signature, SlowAdjustingFeeUpdate, TargetBlockFullness, maximum_normal_block_length,
};

/// How many pieces one sector is supposed to contain (max)
const MAX_PIECES_IN_SECTOR: u16 = 1000;

// To learn more about runtime versioning and what each of the following value means:
//   https://paritytech.github.io/polkadot-sdk/master/sp_version/struct.RuntimeVersion.html
#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
    spec_name: Cow::Borrowed("subspace"),
    impl_name: Cow::Borrowed("subspace"),
    authoring_version: 0,
    // The spec version can be different on Taurus and Mainnet
    spec_version: 2,
    impl_version: 0,
    apis: RUNTIME_API_VERSIONS,
    transaction_version: 0,
    system_version: 2,
};

// TODO: Many of below constants should probably be updatable but currently they are not

// NOTE: Currently it is not possible to change the slot duration after the chain has started.
//       Attempting to do so will brick block production.
const SLOT_DURATION: SlotDuration = SlotDuration::from_millis(1000);

/// Number of slots between slot arrival and when corresponding block can be produced.
const BLOCK_AUTHORING_DELAY: SlotNumber = SlotNumber::new(4);

/// Interval, in blocks, between blockchain entropy injection into proof of time chain.
const POT_ENTROPY_INJECTION_INTERVAL: BlockNumber = 50;

/// Interval, in entropy injection intervals, where to take entropy for injection from.
const POT_ENTROPY_INJECTION_LOOKBACK_DEPTH: u8 = 2;

/// Delay after block, in slots, when entropy injection takes effect.
const POT_ENTROPY_INJECTION_DELAY: SlotNumber = SlotNumber::new(15);

// Entropy injection interval must be bigger than injection delay or else we may end up in a
// situation where we'll need to do more than one injection at the same slot
const_assert!(POT_ENTROPY_INJECTION_INTERVAL > POT_ENTROPY_INJECTION_DELAY.as_u64());
// Entropy injection delay must be bigger than block authoring delay or else we may include
// invalid future proofs in parent block, +1 ensures we do not have unnecessary reorgs that will
// inevitably happen otherwise
const_assert!(POT_ENTROPY_INJECTION_DELAY.as_u64() > BLOCK_AUTHORING_DELAY.as_u64() + 1);

/// Era duration in blocks.
const ERA_DURATION_IN_BLOCKS: BlockNumber = 2016;

/// Number of latest archived segments that are considered "recent history".
const RECENT_SEGMENTS: HistorySize = HistorySize::new(NonZeroU64::new(5).expect("Not zero; qed"));
/// Fraction of pieces from the "recent history" (`recent_segments`) in each sector.
const RECENT_HISTORY_FRACTION: (HistorySize, HistorySize) = (
    HistorySize::new(NonZeroU64::new(1).expect("Not zero; qed")),
    HistorySize::new(NonZeroU64::new(10).expect("Not zero; qed")),
);
/// Minimum lifetime of a plotted sector, measured in archived segment.
const MIN_SECTOR_LIFETIME: HistorySize =
    HistorySize::new(NonZeroU64::new(4).expect("Not zero; qed"));

parameter_types! {
    pub const Version: RuntimeVersion = VERSION;
    pub const BlockHashCount: BlockNumber = 250;
    /// We allow for 2 seconds of compute with a 6 second average block time.
    pub SubspaceBlockWeights: BlockWeights = BlockWeights::with_sensible_defaults(BLOCK_WEIGHT_FOR_2_SEC, NORMAL_DISPATCH_RATIO);
    /// We allow for 3.75 MiB for `Normal` extrinsic with 5 MiB maximum block length.
    pub SubspaceBlockLength: BlockLength = maximum_normal_block_length();
}

pub type SS58Prefix = ConstU16<6094>;

// Configure FRAME pallets to include in runtime.

impl frame_system::Config for Runtime {
    /// The basic call filter to use in dispatchable.
    type BaseCallFilter = Everything;
    /// Block & extrinsics weights: base values and limits.
    type BlockWeights = SubspaceBlockWeights;
    /// The maximum length of a block (in bytes).
    type BlockLength = SubspaceBlockLength;
    /// The identifier used to distinguish between accounts.
    type AccountId = AccountId;
    /// The aggregated dispatch type that is available for extrinsics.
    type RuntimeCall = RuntimeCall;
    /// The aggregated `RuntimeTask` type.
    type RuntimeTask = RuntimeTask;
    /// The lookup mechanism to get account ID from whatever is passed in dispatchers.
    type Lookup = AccountIdLookup<AccountId, ()>;
    /// The type for storing how many extrinsics an account has signed.
    type Nonce = TypeWithDefault<Nonce, DefaultNonceProvider<System, Nonce>>;
    /// The type for hashing blocks and tries.
    type Hash = Hash;
    /// The hashing algorithm used.
    type Hashing = BlakeTwo256;
    /// The block type.
    type Block = Block;
    /// The ubiquitous event type.
    type RuntimeEvent = RuntimeEvent;
    /// The ubiquitous origin type.
    type RuntimeOrigin = RuntimeOrigin;
    /// Maximum number of block number to block hash mappings to keep (oldest pruned first).
    type BlockHashCount = BlockHashCount;
    /// The weight of database operations that the runtime can invoke.
    type DbWeight = ParityDbWeight;
    /// Version of the runtime.
    type Version = Version;
    /// Converts a module to the index of the module in `construct_runtime!`.
    ///
    /// This type is being generated by `construct_runtime!`.
    type PalletInfo = PalletInfo;
    /// What to do if a new account is created.
    type OnNewAccount = ();
    /// What to do if an account is fully reaped from the system.
    type OnKilledAccount = ();
    /// The data to be stored in an account.
    type AccountData = pallet_balances::AccountData<Balance>;
    /// Weight information for the extrinsics of this pallet.
    type SystemWeightInfo = frame_system::weights::SubstrateWeight<Runtime>;
    /// This is used as an identifier of the chain.
    type SS58Prefix = SS58Prefix;
    /// The set code logic, just the default since we're not a parachain.
    type OnSetCode = ();
    type SingleBlockMigrations = ();
    type MultiBlockMigrator = ();
    type PreInherents = ();
    type PostInherents = ();
    type PostTransactions = ();
    type MaxConsumers = ConstU32<16>;
    type ExtensionsWeightInfo = frame_system::ExtensionsWeight<Runtime>;
    type EventSegmentSize = ConsensusEventSegmentSize;
}

parameter_types! {
    pub const RuntimeConsensusConstants: ConsensusConstants<BlockNumber> = ConsensusConstants {
        pot_entropy_injection_interval: POT_ENTROPY_INJECTION_INTERVAL,
        pot_entropy_injection_lookback_depth: POT_ENTROPY_INJECTION_LOOKBACK_DEPTH,
        pot_entropy_injection_delay: POT_ENTROPY_INJECTION_DELAY,
        era_duration: ERA_DURATION_IN_BLOCKS,
        slot_probability: SLOT_PROBABILITY,
    };
}

impl pallet_subspace::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type SubspaceOrigin = pallet_subspace::EnsureSubspaceOrigin;
    type ConsensusConstants = RuntimeConsensusConstants;
    type WeightInfo = pallet_subspace::weights::SubstrateWeight<Runtime>;
    type ExtensionWeightInfo = pallet_subspace::extensions::weights::SubstrateWeight<Runtime>;
}

impl pallet_timestamp::Config for Runtime {
    /// A timestamp: milliseconds since the unix epoch.
    type Moment = Moment;
    type OnTimestampSet = ();
    type MinimumPeriod = ConstU64<{ SLOT_DURATION.as_millis() as u64 / 2 }>;
    type WeightInfo = ();
}

parameter_types! {
    // Computed as ED = Account data size * Price per byte, where
    // Price per byte = Min Number of validators * Storage duration (years) * Storage cost per year
    // Account data size (80 bytes)
    // Min Number of redundant validators (100) - For a stable and redundant blockchain we need at least a certain number of full nodes/collators.
    // Storage duration (1 year) - It is theoretically unlimited, accounts will stay around while the chain is alive.
    // Storage cost per year of (12 * 1e-9 * 0.1 ) - SSD storage on cloud hosting costs about 0.1 USD per Gb per month
    pub const ExistentialDeposit: Balance = 10_000_000_000_000 * SHANNON;
}

impl pallet_balances::Config for Runtime {
    type RuntimeFreezeReason = RuntimeFreezeReason;
    type MaxLocks = ConstU32<50>;
    type MaxReserves = ();
    type ReserveIdentifier = [u8; 8];
    /// The type for recording an account's balance.
    type Balance = Balance;
    /// The ubiquitous event type.
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = pallet_balances::weights::SubstrateWeight<Runtime>;
    type FreezeIdentifier = ();
    type MaxFreezes = ();
    type RuntimeHoldReason = ();
    type DoneSlashHandler = ();
}

parameter_types! {
    pub CreditSupply: Balance = Balances::total_issuance();
    pub TotalSpacePledged: u128 = {
        let pieces = Subspace::solution_ranges().current.to_pieces(SLOT_PROBABILITY);
        pieces as u128 * Piece::SIZE as u128
    };
    pub BlockchainHistorySize: u128 = u128::from(Subspace::archived_history_size());
    pub DynamicCostOfStorage: bool = RuntimeConfigs::enable_dynamic_cost_of_storage();
    pub TransactionWeightFee: Balance = 100_000 * SHANNON;
}

impl pallet_transaction_fees::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type MinReplicationFactor = ConstU16<MIN_REPLICATION_FACTOR>;
    type CreditSupply = CreditSupply;
    type TotalSpacePledged = TotalSpacePledged;
    type BlockchainHistorySize = BlockchainHistorySize;
    type Currency = Balances;
    type DynamicCostOfStorage = DynamicCostOfStorage;
    type WeightInfo = pallet_transaction_fees::weights::SubstrateWeight<Runtime>;
}

impl pallet_transaction_payment::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OnChargeTransaction = OnChargeTransaction;
    type OperationalFeeMultiplier = ConstU8<5>;
    type WeightToFee = ConstantMultiplier<Balance, TransactionWeightFee>;
    type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
    type FeeMultiplierUpdate = SlowAdjustingFeeUpdate<Runtime, TargetBlockFullness>;
    type WeightInfo = pallet_transaction_payment::weights::SubstrateWeight<Runtime>;
}

impl pallet_utility::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type PalletsOrigin = OriginCaller;
    type WeightInfo = pallet_utility::weights::SubstrateWeight<Runtime>;
}

impl MaybeUtilityCall<Runtime> for RuntimeCall {
    /// If this call is a `pallet_utility::Call<Runtime>` call, returns the inner call.
    fn maybe_utility_call(&self) -> Option<&pallet_utility::Call<Runtime>> {
        match self {
            RuntimeCall::Utility(call) => Some(call),
            _ => None,
        }
    }
}

impl MaybeNestedCall<Runtime> for RuntimeCall {
    /// If this call is a nested runtime call, returns the inner call(s).
    ///
    /// Ignored calls (such as `pallet_utility::Call::__Ignore`) should be yielded themsevles, but
    /// their contents should not be yielded.
    fn maybe_nested_call(&self) -> Option<Vec<&RuntimeCallFor<Runtime>>> {
        // We currently ignore privileged calls, because privileged users can already change
        // runtime code. This includes sudo, collective, and scheduler nested `RuntimeCall`s,
        // and democracy nested `BoundedCall`s.

        // It is ok to return early, because each call can only belong to one pallet.
        let calls = self.maybe_nested_utility_calls();
        if calls.is_some() {
            return calls;
        }

        None
    }
}

impl pallet_sudo::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type WeightInfo = pallet_sudo::weights::SubstrateWeight<Runtime>;
}

impl<C> frame_system::offchain::CreateTransactionBase<C> for Runtime
where
    RuntimeCall: From<C>,
{
    type Extrinsic = UncheckedExtrinsic;
    type RuntimeCall = RuntimeCall;
}

impl<C> frame_system::offchain::CreateInherent<C> for Runtime
where
    RuntimeCall: From<C>,
{
    fn create_inherent(call: Self::RuntimeCall) -> Self::Extrinsic {
        UncheckedExtrinsic::new_bare(call)
    }
}

impl<C> subspace_runtime_primitives::CreateUnsigned<C> for Runtime
where
    RuntimeCall: From<C>,
{
    fn create_unsigned(call: Self::RuntimeCall) -> Self::Extrinsic {
        create_unsigned_general_extrinsic(call)
    }
}

impl pallet_runtime_configs::Config for Runtime {
    type WeightInfo = pallet_runtime_configs::weights::SubstrateWeight<Runtime>;
}

construct_runtime!(
    pub struct Runtime {
        System: frame_system = 0,
        Timestamp: pallet_timestamp = 1,

        Subspace: pallet_subspace = 3,

        Balances: pallet_balances = 5,
        TransactionFees: pallet_transaction_fees = 6,
        TransactionPayment: pallet_transaction_payment = 7,
        Utility: pallet_utility = 8,

        RuntimeConfigs: pallet_runtime_configs = 14,

        // Reserve some room for other pallets as we'll remove sudo pallet eventually.
        Sudo: pallet_sudo = 100,
    }
);

/// The address format for describing accounts.
pub type Address = sp_runtime::MultiAddress<AccountId, ()>;
/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;

/// The SignedExtension to the basic transaction logic.
pub type SignedExtra = (
    frame_system::CheckNonZeroSender<Runtime>,
    frame_system::CheckSpecVersion<Runtime>,
    frame_system::CheckTxVersion<Runtime>,
    frame_system::CheckGenesis<Runtime>,
    frame_system::CheckMortality<Runtime>,
    frame_system::CheckNonce<Runtime>,
    frame_system::CheckWeight<Runtime>,
    pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
    pallet_subspace::extensions::SubspaceExtension<Runtime>,
);
/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
    generic::UncheckedExtrinsic<Address, RuntimeCall, Signature, SignedExtra>;

/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
    Runtime,
    Block,
    frame_system::ChainContext<Runtime>,
    Runtime,
    AllPalletsWithSystem,
>;

impl pallet_subspace::extensions::MaybeSubspaceCall<Runtime> for RuntimeCall {
    fn maybe_subspace_call(&self) -> Option<&pallet_subspace::Call<Runtime>> {
        match self {
            RuntimeCall::Subspace(call) => Some(call),
            _ => None,
        }
    }
}

fn extract_segment_headers(ext: &UncheckedExtrinsic) -> Option<Vec<SegmentHeader>> {
    match &ext.function {
        RuntimeCall::Subspace(pallet_subspace::Call::store_segment_headers { segment_headers }) => {
            Some(segment_headers.clone())
        }
        _ => None,
    }
}

fn create_unsigned_general_extrinsic(call: RuntimeCall) -> UncheckedExtrinsic {
    let extra: SignedExtra = (
        frame_system::CheckNonZeroSender::<Runtime>::new(),
        frame_system::CheckSpecVersion::<Runtime>::new(),
        frame_system::CheckTxVersion::<Runtime>::new(),
        frame_system::CheckGenesis::<Runtime>::new(),
        frame_system::CheckMortality::<Runtime>::from(generic::Era::Immortal),
        // for unsigned extrinsic, nonce check will be skipped
        // so set a default value
        frame_system::CheckNonce::<Runtime>::from(0u32.into()),
        frame_system::CheckWeight::<Runtime>::new(),
        // for unsigned extrinsic, transaction fee check will be skipped
        // so set a default value
        pallet_transaction_payment::ChargeTransactionPayment::<Runtime>::from(0u128),
        pallet_subspace::extensions::SubspaceExtension::<Runtime>::new(),
    );

    UncheckedExtrinsic::new_transaction(call, extra)
}

#[cfg(feature = "runtime-benchmarks")]
mod benches {
    frame_benchmarking::define_benchmarks!(
        [frame_benchmarking, BaselineBench::<Runtime>]
        [frame_system, SystemBench::<Runtime>]
        [pallet_balances, Balances]
        [pallet_runtime_configs, RuntimeConfigs]
        [pallet_timestamp, Timestamp]
    );
}

#[cfg(feature = "runtime-benchmarks")]
impl frame_system_benchmarking::Config for Runtime {}

#[cfg(feature = "runtime-benchmarks")]
impl frame_benchmarking::baseline::Config for Runtime {}

impl_runtime_apis! {
    impl sp_api::Core<Block> for Runtime {
        fn version() -> RuntimeVersion {
            VERSION
        }

        fn execute_block(block: Block) {
            Executive::execute_block(block);
        }

        fn initialize_block(header: &<Block as BlockT>::Header) -> ExtrinsicInclusionMode {
            Executive::initialize_block(header)
        }
    }

    impl sp_api::Metadata<Block> for Runtime {
        fn metadata() -> OpaqueMetadata {
            OpaqueMetadata::new(Runtime::metadata().into())
        }

        fn metadata_at_version(version: u32) -> Option<OpaqueMetadata> {
            Runtime::metadata_at_version(version)
        }

        fn metadata_versions() -> Vec<u32> {
            Runtime::metadata_versions()
        }
    }

    impl sp_block_builder::BlockBuilder<Block> for Runtime {
        fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
            Executive::apply_extrinsic(extrinsic)
        }

        fn finalize_block() -> <Block as BlockT>::Header {
            Executive::finalize_block()
        }

        fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
            data.create_extrinsics()
        }

        fn check_inherents(
            block: Block,
            data: sp_inherents::InherentData,
        ) -> sp_inherents::CheckInherentsResult {
            data.check_extrinsics(&block)
        }
    }

    impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
        fn validate_transaction(
            source: TransactionSource,
            tx: <Block as BlockT>::Extrinsic,
            block_hash: <Block as BlockT>::Hash,
        ) -> TransactionValidity {
            Executive::validate_transaction(source, tx, block_hash)
        }
    }

    impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
        fn offchain_worker(header: &<Block as BlockT>::Header) {
            Executive::offchain_worker(header)
        }
    }

    impl sp_objects::ObjectsApi<Block> for Runtime {
        fn extract_block_object_mapping(block: Block) -> BlockObjectMapping {
            extract_block_object_mapping(block)
        }
    }

    impl sp_consensus_subspace::SubspaceApi<Block> for Runtime {
        fn pot_parameters() -> PotParameters {
            Subspace::pot_parameters()
        }

        fn solution_ranges() -> SolutionRanges {
            Subspace::solution_ranges()
        }

        fn history_size() -> HistorySize {
            <pallet_subspace::Pallet<Runtime>>::history_size()
        }

        fn max_pieces_in_sector() -> u16 {
            MAX_PIECES_IN_SECTOR
        }

        fn segment_root(segment_index: SegmentIndex) -> Option<SegmentRoot> {
            Subspace::segment_root(segment_index)
        }

        fn extract_segment_headers(ext: &<Block as BlockT>::Extrinsic) -> Option<Vec<SegmentHeader >> {
            extract_segment_headers(ext)
        }

        fn is_inherent(ext: &<Block as BlockT>::Extrinsic) -> bool {
            match &ext.function {
                RuntimeCall::Subspace(call) => Subspace::is_inherent(call),
                RuntimeCall::Timestamp(call) => Timestamp::is_inherent(call),
                _ => false,
            }
        }

        fn root_plot_public_key_hash() -> Option<Blake3Hash> {
            Subspace::root_plot_public_key_hash()
        }

        fn should_adjust_solution_range() -> bool {
            Subspace::should_adjust_solution_range()
        }

        fn chain_constants() -> ChainConstants {
            ChainConstants::V0 {
                confirmation_depth_k: pallet_runtime_configs::ConfirmationDepthK::<Runtime>::get(),
                block_authoring_delay: BLOCK_AUTHORING_DELAY,
                era_duration: ERA_DURATION_IN_BLOCKS,
                slot_probability: SLOT_PROBABILITY,
                slot_duration: SLOT_DURATION,
                recent_segments: RECENT_SEGMENTS,
                recent_history_fraction: RECENT_HISTORY_FRACTION,
                min_sector_lifetime: MIN_SECTOR_LIFETIME,
            }
        }
    }

    impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce> for Runtime {
        fn account_nonce(account: AccountId) -> Nonce {
            *System::account_nonce(account)
        }
    }

    impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance> for Runtime {
        fn query_info(
            uxt: <Block as BlockT>::Extrinsic,
            len: u32,
        ) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
            TransactionPayment::query_info(uxt, len)
        }
        fn query_fee_details(
            uxt: <Block as BlockT>::Extrinsic,
            len: u32,
        ) -> pallet_transaction_payment::FeeDetails<Balance> {
            TransactionPayment::query_fee_details(uxt, len)
        }
        fn query_weight_to_fee(weight: Weight) -> Balance {
            TransactionPayment::weight_to_fee(weight)
        }
        fn query_length_to_fee(length: u32) -> Balance {
            TransactionPayment::length_to_fee(length)
        }
    }

    impl sp_genesis_builder::GenesisBuilder<Block> for Runtime {
        fn build_state(config: Vec<u8>) -> sp_genesis_builder::Result {
            build_state::<RuntimeGenesisConfig>(config)
        }

        fn get_preset(_id: &Option<sp_genesis_builder::PresetId>) -> Option<Vec<u8>> {
            // By passing `None` the upstream `get_preset` will return the default value of `RuntimeGenesisConfig`
            get_preset::<RuntimeGenesisConfig>(&None, |_| None)
        }

        fn preset_names() -> Vec<sp_genesis_builder::PresetId> {
            Vec::new()
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    impl frame_benchmarking::Benchmark<Block> for Runtime {
        fn benchmark_metadata(extra: bool) -> (
            Vec<frame_benchmarking::BenchmarkList>,
            Vec<frame_support::traits::StorageInfo>,
        ) {
            use frame_benchmarking::{baseline, Benchmarking, BenchmarkList};
            use frame_support::traits::StorageInfoTrait;
            use frame_system_benchmarking::Pallet as SystemBench;
            use baseline::Pallet as BaselineBench;

            let mut list = Vec::<BenchmarkList>::new();
            list_benchmarks!(list, extra);

            let storage_info = AllPalletsWithSystem::storage_info();

            (list, storage_info)
        }

        fn dispatch_benchmark(
            config: frame_benchmarking::BenchmarkConfig
        ) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, alloc::string::String> {
            use frame_benchmarking::{baseline, Benchmarking, BenchmarkBatch};
            use sp_core::storage::TrackedStorageKey;

            use frame_system_benchmarking::Pallet as SystemBench;
            use baseline::Pallet as BaselineBench;

            use frame_support::traits::WhitelistedStorageKeys;
            let whitelist: Vec<TrackedStorageKey> = AllPalletsWithSystem::whitelisted_storage_keys();

            let mut batches = Vec::<BenchmarkBatch>::new();
            let params = (&config, &whitelist);
            add_benchmarks!(params, batches);

            Ok(batches)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Runtime, SubspaceBlockWeights as BlockWeights};
    use subspace_runtime_primitives::tests_utils::FeeMultiplierUtils;

    #[test]
    fn multiplier_can_grow_from_zero() {
        FeeMultiplierUtils::<Runtime, BlockWeights>::multiplier_can_grow_from_zero()
    }
}
