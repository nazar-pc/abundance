//! Subspace chain configurations.

use crate::chain_spec_utils::{chain_spec_properties, get_account_id_from_seed};
use sc_chain_spec::GenericChainSpec;
use sc_service::ChainType;
use sc_telemetry::TelemetryEndpoints;
use sp_core::crypto::Ss58Codec;
use sp_runtime::BoundedVec;
use std::marker::PhantomData;
use std::num::NonZeroU32;
use subspace_core_primitives::pot::PotKey;
use subspace_core_primitives::PublicKey;
use subspace_runtime::{
    AllowAuthoringBy, BalancesConfig, RewardPoint, RewardsConfig, RuntimeConfigsConfig,
    RuntimeGenesisConfig, SubspaceConfig, SudoConfig, SystemConfig, WASM_BINARY,
};
use subspace_runtime_primitives::{AccountId, Balance, SSC};

const SUBSPACE_TELEMETRY_URL: &str = "wss://telemetry.subspace.foundation/submit/";

/// Additional subspace specific genesis parameters.
struct GenesisParams {
    allow_authoring_by: AllowAuthoringBy,
    pot_slot_iterations: NonZeroU32,
    enable_dynamic_cost_of_storage: bool,
    confirmation_depth_k: u32,
    rewards_config: RewardsConfig,
}

pub fn mainnet_compiled() -> Result<GenericChainSpec, String> {
    Ok(GenericChainSpec::builder(
        WASM_BINARY.ok_or_else(|| "Wasm binary must be built for Mainnet".to_string())?,
        None,
    )
    .with_name("Autonomys Mainnet")
    // ID
    .with_id("autonomys_mainnet")
    .with_chain_type(ChainType::Custom("Autonomys Mainnet".to_string()))
    .with_telemetry_endpoints(
        TelemetryEndpoints::new(vec![(SUBSPACE_TELEMETRY_URL.into(), 1)])
            .map_err(|error| error.to_string())?,
    )
    .with_protocol_id("autonomys-mainnet")
    .with_properties({
        let mut properties = chain_spec_properties();
        properties.insert(
            "potExternalEntropy".to_string(),
            serde_json::to_value(None::<PotKey>).expect("Serialization is infallible; qed"),
        );
        properties
    })
    .with_genesis_config({
        let sudo_account =
            AccountId::from_ss58check("5EHHtxGtDEPFX2x2PCVg8uhhg6kDdt9znQLr2oqUA9sYL5n6")
                .expect("Wrong root account address");

        let balances = Vec::new();

        serde_json::to_value(subspace_genesis_config(
            sudo_account.clone(),
            balances,
            GenesisParams {
                allow_authoring_by: AllowAuthoringBy::RootFarmer(PublicKey::from(
                    hex_literal::hex!(
                        "e6a489dab63b650cf475431fc46649f4256167443fea241fca0bb3f86b29837a"
                    ),
                )),
                // TODO: Adjust once we bench PoT on faster hardware
                // About 1s on 6.2 GHz Raptor Lake CPU (14900KS)
                pot_slot_iterations: NonZeroU32::new(206_557_520).expect("Not zero; qed"),
                enable_dynamic_cost_of_storage: false,
                // TODO: Proper value here
                confirmation_depth_k: 100,
                rewards_config: RewardsConfig {
                    remaining_issuance: 350_000_000 * SSC,
                    proposer_subsidy_points: BoundedVec::try_from(vec![
                        RewardPoint {
                            block: 0,
                            subsidy: 454545454545455000,
                        },
                        RewardPoint {
                            block: 10512000,
                            subsidy: 423672207997007000,
                        },
                        RewardPoint {
                            block: 26280000,
                            subsidy: 333635878252228000,
                        },
                        RewardPoint {
                            block: 42048000,
                            subsidy: 262825353875519000,
                        },
                        RewardPoint {
                            block: 57816000,
                            subsidy: 207116053874914000,
                        },
                        RewardPoint {
                            block: 73584000,
                            subsidy: 163272262877830000,
                        },
                        RewardPoint {
                            block: 94608000,
                            subsidy: 118963574070561000,
                        },
                        RewardPoint {
                            block: 120888000,
                            subsidy: 80153245846642200,
                        },
                        RewardPoint {
                            block: 149796000,
                            subsidy: 51971522998131200,
                        },
                        RewardPoint {
                            block: 183960000,
                            subsidy: 31192714495359900,
                        },
                        RewardPoint {
                            block: 220752000,
                            subsidy: 18033114698427300,
                        },
                    ])
                    .expect("Number of elements is below configured MaxRewardPoints; qed"),
                },
            },
        )?)
        .map_err(|error| format!("Failed to serialize genesis config: {error}"))?
    })
    .build())
}

pub fn devnet_config_compiled() -> Result<GenericChainSpec, String> {
    Ok(GenericChainSpec::builder(
        WASM_BINARY.ok_or_else(|| "Wasm binary must be built for Devnet".to_string())?,
        None,
    )
    .with_name("Subspace Dev network")
    .with_id("subspace_devnet")
    .with_chain_type(ChainType::Custom("Testnet".to_string()))
    .with_telemetry_endpoints(
        TelemetryEndpoints::new(vec![(SUBSPACE_TELEMETRY_URL.into(), 1)])
            .map_err(|error| error.to_string())?,
    )
    .with_protocol_id("subspace-devnet")
    .with_properties({
        let mut properties = chain_spec_properties();
        properties.insert(
            "potExternalEntropy".to_string(),
            serde_json::to_value(None::<PotKey>).expect("Serialization is infallible; qed"),
        );
        properties
    })
    .with_genesis_config({
        let sudo_account =
            AccountId::from_ss58check("5H6ai5VAt6Sw2qZGkEVGvLvNqTCPv6fZRN2KN2kp5qMQKBUD")
                .expect("Wrong root account address");

        let balances = vec![(sudo_account.clone(), Balance::MAX / 2)];
        serde_json::to_value(subspace_genesis_config(
            sudo_account.clone(),
            balances,
            GenesisParams {
                allow_authoring_by: AllowAuthoringBy::FirstFarmer,
                pot_slot_iterations: NonZeroU32::new(150_000_000).expect("Not zero; qed"),
                enable_dynamic_cost_of_storage: false,
                // TODO: Proper value here
                confirmation_depth_k: 100,
                // TODO: Proper value here
                rewards_config: RewardsConfig {
                    remaining_issuance: 1_000_000_000 * SSC,
                    proposer_subsidy_points: Default::default(),
                },
            },
        )?)
        .map_err(|error| format!("Failed to serialize genesis config: {error}"))?
    })
    .build())
}

pub fn dev_config() -> Result<GenericChainSpec, String> {
    let wasm_binary = WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?;
    let sudo_account = get_account_id_from_seed("Alice");

    Ok(GenericChainSpec::builder(wasm_binary, None)
        .with_name("Subspace development")
        .with_id("subspace_dev")
        .with_chain_type(ChainType::Development)
        .with_properties({
            let mut properties = chain_spec_properties();
            properties.insert(
                "potExternalEntropy".to_string(),
                serde_json::to_value(None::<PotKey>).expect("Serialization is infallible; qed"),
            );
            properties
        })
        .with_genesis_config(
            serde_json::to_value(subspace_genesis_config(
                // Sudo account
                sudo_account.clone(),
                // Pre-funded accounts
                vec![
                    (sudo_account.clone(), Balance::MAX / 2),
                    (get_account_id_from_seed("Bob"), 1_000 * SSC),
                    (get_account_id_from_seed("Alice//stash"), 1_000 * SSC),
                    (get_account_id_from_seed("Bob//stash"), 1_000 * SSC),
                ],
                GenesisParams {
                    allow_authoring_by: AllowAuthoringBy::Anyone,
                    pot_slot_iterations: NonZeroU32::new(100_000_000).expect("Not zero; qed"),
                    enable_dynamic_cost_of_storage: false,
                    confirmation_depth_k: 5,
                    rewards_config: RewardsConfig {
                        remaining_issuance: 1_000_000 * SSC,
                        proposer_subsidy_points: Default::default(),
                    },
                },
            )?)
            .map_err(|error| format!("Failed to serialize genesis config: {error}"))?,
        )
        .build())
}

/// Configure initial storage state for FRAME modules.
fn subspace_genesis_config(
    sudo_account: AccountId,
    balances: Vec<(AccountId, Balance)>,
    genesis_params: GenesisParams,
) -> Result<RuntimeGenesisConfig, String> {
    let GenesisParams {
        allow_authoring_by,
        pot_slot_iterations,
        enable_dynamic_cost_of_storage,
        confirmation_depth_k,
        rewards_config,
    } = genesis_params;

    Ok(RuntimeGenesisConfig {
        system: SystemConfig::default(),
        balances: BalancesConfig { balances },
        transaction_payment: Default::default(),
        sudo: SudoConfig {
            // Assign network admin rights.
            key: Some(sudo_account.clone()),
        },
        subspace: SubspaceConfig {
            allow_authoring_by,
            pot_slot_iterations,
            phantom: PhantomData,
        },
        rewards: rewards_config,
        runtime_configs: RuntimeConfigsConfig {
            enable_dynamic_cost_of_storage,
            confirmation_depth_k,
        },
    })
}
