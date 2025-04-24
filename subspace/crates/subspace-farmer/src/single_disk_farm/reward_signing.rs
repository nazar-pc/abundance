use crate::node_client::NodeClient;
use crate::single_disk_farm::identity::Identity;
use futures::StreamExt;
use std::future::Future;
use subspace_core_primitives::sr25519::{PublicKey, RewardSignature, Signature};
use subspace_rpc_primitives::{RewardSignatureResponse, RewardSigningInfo};
use tracing::{info, warn};

pub(super) async fn reward_signing<NC>(
    node_client: NC,
    identity: Identity,
) -> anyhow::Result<impl Future<Output = ()>>
where
    NC: NodeClient,
{
    info!("Subscribing to reward signing notifications");

    let mut reward_signing_info_notifications = node_client.subscribe_reward_signing().await?;
    let own_public_key_hash = PublicKey::from(identity.public_key().to_bytes()).hash();

    let reward_signing_fut = async move {
        while let Some(RewardSigningInfo {
            hash,
            public_key_hash,
        }) = reward_signing_info_notifications.next().await
        {
            // Multiple plots might have solved, only sign with correct one
            if public_key_hash != own_public_key_hash {
                continue;
            }

            let signature = identity.sign_reward_hash(&hash);

            match node_client
                .submit_reward_signature(RewardSignatureResponse {
                    hash,
                    signature: Some(RewardSignature {
                        public_key: PublicKey::from(identity.public_key().to_bytes()),
                        signature: Signature::from(signature.to_bytes()),
                    }),
                })
                .await
            {
                Ok(_) => {
                    info!("Successfully signed reward hash 0x{}", hex::encode(hash));
                }
                Err(error) => {
                    warn!(
                        %error,
                        "Failed to send signature for reward hash 0x{}",
                        hex::encode(hash),
                    );
                }
            }
        }
    };

    Ok(reward_signing_fut)
}
