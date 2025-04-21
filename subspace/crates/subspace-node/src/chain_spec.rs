//! Subspace chain configurations.

use crate::chain_spec_utils::{chain_spec_properties, get_account_id_from_seed};
use sc_chain_spec::GenericChainSpec;
use sc_service::ChainType;
use sp_core::crypto::Ss58Codec;
use std::marker::PhantomData;
use std::num::NonZeroU32;
use subspace_core_primitives::pot::PotKey;
use subspace_core_primitives::solutions::SolutionRange;
use subspace_core_primitives::PublicKey;
use subspace_runtime::{
    AllowAuthoringBy, BalancesConfig, RuntimeConfigsConfig, RuntimeGenesisConfig, SubspaceConfig,
    SudoConfig, SystemConfig, WASM_BINARY,
};
use subspace_runtime_primitives::{AccountId, Balance, SLOT_PROBABILITY, SSC};

// We assume initial plot size starts with a single sector.
const INITIAL_SOLUTION_RANGE: SolutionRange = SolutionRange::from_pieces(1000, SLOT_PROBABILITY);

/// Additional subspace specific genesis parameters.
struct GenesisParams {
    allow_authoring_by: AllowAuthoringBy,
    pot_slot_iterations: NonZeroU32,
    enable_dynamic_cost_of_storage: bool,
    confirmation_depth_k: u32,
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
            initial_solution_range: INITIAL_SOLUTION_RANGE,
            phantom: PhantomData,
        },
        runtime_configs: RuntimeConfigsConfig {
            enable_dynamic_cost_of_storage,
            confirmation_depth_k,
        },
    })
}
