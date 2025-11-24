use crate::node_client::NodeClient;
use crate::single_disk_farm::identity::Identity;
use ab_core_primitives::block::header::{BlockHeaderEd25519Seal, OwnedBlockHeaderSeal};
use ab_farmer_rpc_primitives::{BlockSealInfo, BlockSealResponse};
use futures::StreamExt;
use std::future::Future;
use tracing::{info, warn};

pub(super) async fn block_sealing<NC>(
    node_client: NC,
    identity: Identity,
) -> anyhow::Result<impl Future<Output = ()>>
where
    NC: NodeClient,
{
    info!("Subscribing to block sealing notifications");

    let mut block_sealing_info_notifications = node_client.subscribe_block_sealing().await?;
    let own_public_key_hash = identity.public_key().hash();

    let block_sealing_fut = async move {
        while let Some(BlockSealInfo {
            pre_seal_hash,
            public_key_hash,
        }) = block_sealing_info_notifications.next().await
        {
            // Multiple plots might have solved, only sign with the correct one
            if public_key_hash != own_public_key_hash {
                continue;
            }

            match node_client
                .submit_block_seal(BlockSealResponse {
                    pre_seal_hash,
                    seal: OwnedBlockHeaderSeal::Ed25519(BlockHeaderEd25519Seal {
                        public_key: identity.public_key(),
                        signature: identity.sign_pre_seal_hash(&pre_seal_hash),
                    }),
                })
                .await
            {
                Ok(_) => {
                    info!(
                        "Successfully sealed block pre-seal hash {}",
                        hex::encode(pre_seal_hash)
                    );
                }
                Err(error) => {
                    warn!(
                        %error,
                        "Failed to send seal for block pre-seal hash {}",
                        hex::encode(pre_seal_hash),
                    );
                }
            }
        }
    };

    Ok(block_sealing_fut)
}
