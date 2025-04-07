// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use jsonrpsee::RpcModule;
use sc_client_api::{BlockchainEvents, UsageProvider};
use sc_network_sync::SyncingService;
use sc_rpc::SubscriptionTaskExecutor;
use sc_service::{
    propagate_transaction_notifications, start_rpc_servers, Configuration, Error, TaskManager,
};
use sc_transaction_pool_api::MaintainedTransactionPool;
use sp_blockchain::{HeaderBackend, HeaderMetadata};
use sp_runtime::traits::Block as BlockT;
use std::sync::Arc;
use tracing::info;

/// Parameters to pass into `build`.
pub struct SpawnTasksParams<'a, TBl: BlockT, TCl, TExPool> {
    /// The service configuration.
    pub config: Configuration,
    /// A shared client returned by `new_full_parts`.
    pub client: Arc<TCl>,
    /// A task manager returned by `new_full_parts`.
    pub task_manager: &'a mut TaskManager,
    /// A shared transaction pool.
    pub transaction_pool: Arc<TExPool>,
    /// Builds additional [`RpcModule`]s that should be added to the server
    pub rpc_builder: Box<dyn Fn(SubscriptionTaskExecutor) -> Result<RpcModule<()>, Error>>,
    /// A shared network instance.
    pub network: Arc<dyn sc_network::service::traits::NetworkService>,
    /// Controller for transactions handlers
    pub tx_handler_controller:
        sc_network_transactions::TransactionsHandlerController<<TBl as BlockT>::Hash>,
    /// Syncing service.
    pub sync_service: Arc<SyncingService<TBl>>,
}

/// Spawn the tasks that are required to run a node.
#[expect(clippy::result_large_err, reason = "Comes from Substrate")]
pub(super) fn spawn_tasks<TBl, TExPool, TCl>(
    params: SpawnTasksParams<TBl, TCl, TExPool>,
) -> Result<(), Error>
where
    TCl: HeaderMetadata<TBl, Error = sp_blockchain::Error>
        + HeaderBackend<TBl>
        + BlockchainEvents<TBl>
        + UsageProvider<TBl>
        + Send
        + 'static,
    TBl: BlockT,
    TExPool: MaintainedTransactionPool<Block = TBl, Hash = TBl::Hash> + 'static,
{
    let SpawnTasksParams {
        // TODO: Stop using `Configuration` once
        //  https://github.com/paritytech/polkadot-sdk/pull/5364 is in our fork
        mut config,
        task_manager,
        client,
        transaction_pool,
        rpc_builder,
        network,
        tx_handler_controller,
        sync_service,
    } = params;

    let chain_info = client.usage_info().chain;

    info!("📦 Highest known block at #{}", chain_info.best_number);

    let spawn_handle = task_manager.spawn_handle();

    // Inform the tx pool about imported and finalized blocks.
    spawn_handle.spawn(
        "txpool-notifications",
        Some("transaction-pool"),
        sc_transaction_pool::notification_future(client.clone(), transaction_pool.clone()),
    );

    spawn_handle.spawn(
        "on-transaction-imported",
        Some("transaction-pool"),
        propagate_transaction_notifications(transaction_pool.clone(), tx_handler_controller, None),
    );

    let rpc_id_provider = config.rpc.id_provider.take();

    let subscription_executor = Arc::new(task_manager.spawn_handle()) as Arc<_>;

    // jsonrpsee RPC
    let rpc_server_handle = start_rpc_servers(
        &config.rpc,
        config.prometheus_registry(),
        &config.tokio_handle,
        || rpc_builder(Arc::clone(&subscription_executor)),
        rpc_id_provider,
    )?;

    // Spawn informant task
    spawn_handle.spawn(
        "informant",
        None,
        sc_informant::build(client.clone(), network, sync_service.clone()),
    );

    task_manager.keep_alive((config.base_path, rpc_server_handle));

    Ok(())
}
