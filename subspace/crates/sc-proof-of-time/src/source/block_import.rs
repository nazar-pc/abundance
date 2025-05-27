use ab_core_primitives::pot::{PotOutput, PotParametersChange, SlotNumber};
use futures::channel::mpsc;
use futures::{SinkExt, StreamExt};
use sc_client_api::BlockchainEvents;
use sp_api::{ApiError, ProvideRuntimeApi};
use sp_blockchain::HeaderBackend;
use sp_consensus_subspace::SubspaceApi;
use sp_consensus_subspace::digests::extract_subspace_digest_items;
use sp_runtime::traits::{Block as BlockT, Header as HeaderT};
use std::marker::PhantomData;
use std::sync::Arc;
use tracing::{debug, error};

/// PoT information of the best block
#[derive(Debug, Copy, Clone)]
pub struct BestBlockPotInfo {
    /// Slot for which PoT output was generated
    pub slot: SlotNumber,
    /// PoT output itself
    pub pot_output: PotOutput,
    /// Change of parameters to apply to PoT chain
    pub pot_parameters_change: Option<PotParametersChange>,
}

/// PoT source using the best imported block
#[derive(Debug)]
#[must_use = "Doesn't do anything unless run() method is called"]
pub struct BestBlockPotSource<Block, Client> {
    client: Arc<Client>,
    block_authoring_delay: SlotNumber,
    best_block_pot_info_sender: mpsc::Sender<BestBlockPotInfo>,
    _block: PhantomData<Block>,
}

impl<Block, Client> BestBlockPotSource<Block, Client>
where
    Block: BlockT,
    Client: BlockchainEvents<Block> + HeaderBackend<Block> + ProvideRuntimeApi<Block>,
    Client::Api: SubspaceApi<Block>,
{
    /// Create a new source instance for the provided client
    pub fn new(client: Arc<Client>) -> Result<(Self, mpsc::Receiver<BestBlockPotInfo>), ApiError> {
        let best_hash = client.info().best_hash;
        let runtime_api = client.runtime_api();
        let chain_constants = runtime_api.chain_constants(best_hash)?;

        let (best_block_pot_info_sender, best_block_pot_info_receiver) = mpsc::channel(1);

        Ok((
            Self {
                client,
                block_authoring_delay: chain_constants.block_authoring_delay(),
                best_block_pot_info_sender,
                _block: PhantomData,
            },
            best_block_pot_info_receiver,
        ))
    }

    /// Run until receiver returned from constructor is dropped
    pub async fn run(self) {
        let Self {
            client,
            block_authoring_delay,
            mut best_block_pot_info_sender,
            _block: _,
        } = self;
        let mut import_notification_stream = client.import_notification_stream();
        drop(client);

        while let Some(import_notification) = import_notification_stream.next().await {
            if !import_notification.is_new_best {
                // Ignore blocks that don't extend the chain
                continue;
            }

            let block_hash = import_notification.hash;
            let header = &import_notification.header;

            let subspace_digest_items = match extract_subspace_digest_items(header) {
                Ok(pre_digest) => pre_digest,
                Err(error) => {
                    error!(
                        %error,
                        block_number = %header.number(),
                        %block_hash,
                        "Failed to extract Subspace digest items from header"
                    );
                    return;
                }
            };

            if let Err(error) = best_block_pot_info_sender
                .send(BestBlockPotInfo {
                    slot: subspace_digest_items.pre_digest.slot + block_authoring_delay,
                    pot_output: subspace_digest_items
                        .pre_digest
                        .pot_info
                        .future_proof_of_time,
                    pot_parameters_change: subspace_digest_items.pot_parameters_change,
                })
                .await
            {
                debug!(%error, "Couldn't send info, the channel is closed");
                break;
            }
        }
    }
}
