//! Primitives for the farmer

use ab_core_primitives::block::BlockRoot;
use ab_core_primitives::block::header::OwnedBlockHeaderSeal;
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pot::SlotNumber;
use ab_core_primitives::segments::HistorySize;
use ab_core_primitives::shard::NumShards;
use ab_core_primitives::solutions::{ShardMembershipEntropy, Solution, SolutionRange};
use ab_farmer_components::FarmerProtocolInfo;
use ab_networking::libp2p::Multiaddr;
use parity_scale_codec::{Decode, Encode, EncodeLike, Input, Output};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Defines a limit for number of segments that can be requested over RPC
pub const MAX_SEGMENT_HEADERS_PER_REQUEST: usize = 1000;
// TODO: This is a workaround for https://github.com/paritytech/jsonrpsee/issues/1617 and should be
//  removed once that issue is resolved
/// Shard membership expiration
pub const SHARD_MEMBERSHIP_EXPIRATION: Duration = Duration::from_mins(1);

/// Information necessary for farmer application
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FarmerAppInfo {
    /// Genesis root of the beacon chain
    pub genesis_root: BlockRoot,
    /// Bootstrap nodes for DSN
    pub dsn_bootstrap_nodes: Vec<Multiaddr>,
    /// Whether node is syncing right now
    pub syncing: bool,
    /// How much time farmer has to audit sectors and generate a solution
    pub farming_timeout: Duration,
    /// Protocol info for farmer
    pub protocol_info: FarmerProtocolInfo,
}

impl Encode for FarmerAppInfo {
    fn size_hint(&self) -> usize {
        0_usize
            .saturating_add(Encode::size_hint(&self.genesis_root))
            .saturating_add(Encode::size_hint(
                &self
                    .dsn_bootstrap_nodes
                    .iter()
                    .map(|addr| addr.as_ref())
                    .collect::<Vec<_>>(),
            ))
            .saturating_add(Encode::size_hint(&self.syncing))
            .saturating_add(Encode::size_hint(&self.farming_timeout))
            .saturating_add(Encode::size_hint(&self.protocol_info))
    }

    fn encode_to<O: Output + ?Sized>(&self, output: &mut O) {
        Encode::encode_to(&self.genesis_root, output);
        Encode::encode_to(
            &self
                .dsn_bootstrap_nodes
                .iter()
                .map(|addr| addr.as_ref())
                .collect::<Vec<_>>(),
            output,
        );
        Encode::encode_to(&self.syncing, output);
        Encode::encode_to(&self.farming_timeout, output);
        Encode::encode_to(&self.protocol_info, output);
    }
}

impl EncodeLike for FarmerAppInfo {}

impl Decode for FarmerAppInfo {
    fn decode<I: Input>(input: &mut I) -> Result<Self, parity_scale_codec::Error> {
        Ok(FarmerAppInfo {
            genesis_root: BlockRoot::decode(input)
                .map_err(|error| error.chain("Could not decode `FarmerAppInfo::genesis_root`"))?,
            dsn_bootstrap_nodes: Vec::<Vec<u8>>::decode(input)
                .map_err(|error| {
                    error.chain("Could not decode `FarmerAppInfo::dsn_bootstrap_nodes`")
                })?
                .into_iter()
                .map(Multiaddr::try_from)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| {
                    parity_scale_codec::Error::from("Failed to decode bytes as Multiaddr")
                        .chain(error.to_string())
                        .chain("Could not decode `FarmerAppInfo::dsn_bootstrap_nodes`")
                })?,
            syncing: bool::decode(input)
                .map_err(|error| error.chain("Could not decode `FarmerAppInfo::syncing`"))?,
            farming_timeout: Duration::decode(input).map_err(|error| {
                error.chain("Could not decode `FarmerAppInfo::farming_timeout`")
            })?,
            protocol_info: FarmerProtocolInfo::decode(input)
                .map_err(|error| error.chain("Could not decode `FarmerAppInfo::protocol_info`"))?,
        })
    }
}

/// Information about new slot that just arrived
#[derive(Debug, Copy, Clone, Eq, PartialEq, Encode, Decode, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotInfo {
    /// Slot number
    pub slot: SlotNumber,
    /// Global slot challenge
    pub global_challenge: Blake3Hash,
    /// Acceptable solution range for block authoring
    pub solution_range: SolutionRange,
    /// Current shard membership entropy
    pub entropy: ShardMembershipEntropy,
    /// The number of shards in the network
    pub num_shards: NumShards,
}

/// Response of a slot challenge consisting of an optional solution and
/// the submitter(farmer)'s secret key for block signing.
#[derive(Clone, Debug, Encode, Decode, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolutionResponse {
    /// Slot number.
    pub slot_number: SlotNumber,
    /// Solution farmer has for the challenge.
    ///
    /// Corresponds to `slot_number` above.
    pub solution: Solution,
}

/// Block sealing info
#[derive(Clone, Copy, Debug, Encode, Decode, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockSealInfo {
    /// Block pre-seal hash to be signed
    pub pre_seal_hash: Blake3Hash,
    /// Public key hash of the plot identity that should create signature
    pub public_key_hash: Blake3Hash,
}

/// Block sealing response
#[derive(Clone, Copy, Debug, Encode, Decode, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockSealResponse {
    /// Block pre-seal hash that was signed
    pub pre_seal_hash: Blake3Hash,
    /// The seal itself
    pub seal: OwnedBlockHeaderSeal,
}

/// Farmer shard membership info
#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FarmerShardMembershipInfo {
    /// Public key hash of the plot identity
    pub public_key_hash: Blake3Hash,
    /// Seed used to derive the shard commitment (typically a hash of the private key)
    pub shard_commitment_seed: Blake3Hash,
    /// History sizes
    pub history_sizes: Vec<HistorySize>,
}
