use crate::importing_blocks::{ImportingBlockHandle, ImportingBlocks, ParentBlockImportStatus};
use crate::{BlockImport, BlockImportError};
use ab_client_api::{BlockDetails, BlockOrigin, ChainInfo, ChainInfoWrite};
use ab_client_block_verification::{BlockVerification, BlockVerificationError};
use ab_client_consensus_common::BlockImportingNotification;
use ab_client_consensus_common::consensus_parameters::{
    DeriveConsensusParametersChainInfo, DeriveConsensusParametersConsensusInfo,
};
use ab_client_consensus_common::state::GlobalState;
use ab_core_primitives::block::header::owned::OwnedBeaconChainHeader;
use ab_core_primitives::block::owned::OwnedBeaconChainBlock;
use ab_core_primitives::block::{BlockNumber, BlockRoot};
use ab_core_primitives::hashes::Blake3Hash;
use ab_proof_of_space::Table;
use futures::channel::mpsc;
use futures::prelude::*;
use rclite::Arc;
use send_future::SendFuture;
use std::marker::PhantomData;
use std::sync::Arc as StdArc;
use tracing::{info, warn};

/// Errors for [`BeaconChainBlockImport`]
#[derive(Debug, thiserror::Error)]
pub enum BeaconChainBlockImportError {
    /// Block verification error
    #[error("Block verification error: {error}")]
    VerificationError {
        /// Block verification error
        #[from]
        error: BlockVerificationError,
    },
}

impl From<BeaconChainBlockImportError> for BlockImportError {
    #[inline(always)]
    fn from(error: BeaconChainBlockImportError) -> Self {
        Self::Custom {
            error: error.into(),
        }
    }
}

/// A custom wrapper that will query chain info from either already persisted blocks or blocks that
/// are currently queued for import
#[derive(Debug)]
struct VerificationChainInfo<'a, CI> {
    chain_info: &'a CI,
    importing_blocks: &'a ImportingBlocks<OwnedBeaconChainHeader>,
}

impl<'a, CI> DeriveConsensusParametersChainInfo for VerificationChainInfo<'a, CI>
where
    CI: ChainInfo<OwnedBeaconChainBlock>,
{
    fn ancestor_header_consensus_info(
        &self,
        ancestor_block_number: BlockNumber,
        descendant_block_root: &BlockRoot,
    ) -> Option<DeriveConsensusParametersConsensusInfo> {
        if let Some(consensus_info) = self
            .chain_info
            .ancestor_header_consensus_info(ancestor_block_number, descendant_block_root)
        {
            return Some(consensus_info);
        }

        let mut current_block_root = *descendant_block_root;
        loop {
            let Some(importing_entry) = self.importing_blocks.get(&current_block_root) else {
                break;
            };
            let header = importing_entry.header().header();

            if header.prefix.number == ancestor_block_number {
                return Some(DeriveConsensusParametersConsensusInfo::from_consensus_info(
                    header.consensus_info,
                ));
            }

            current_block_root = *header.root();
        }

        // Query again in case of a race condition where previously importing block was imported in
        // between iterations in the above loop
        self.chain_info
            .ancestor_header_consensus_info(ancestor_block_number, descendant_block_root)
    }
}

#[derive(Debug)]
pub struct BeaconChainBlockImport<PosTable, CI, BV> {
    chain_info: CI,
    block_verification: BV,
    importing_blocks: ImportingBlocks<OwnedBeaconChainHeader>,
    block_importing_notification_sender: mpsc::Sender<BlockImportingNotification>,
    block_import_notification_sender: mpsc::Sender<OwnedBeaconChainBlock>,
    _pos_table: PhantomData<PosTable>,
}

impl<PosTable, CI, BV> BlockImport<OwnedBeaconChainBlock>
    for BeaconChainBlockImport<PosTable, CI, BV>
where
    PosTable: Table,
    CI: ChainInfoWrite<OwnedBeaconChainBlock>,
    BV: BlockVerification<OwnedBeaconChainBlock>,
{
    fn import(
        &self,
        block: OwnedBeaconChainBlock,
        origin: BlockOrigin,
    ) -> Result<impl Future<Output = Result<(), BlockImportError>> + Send, BlockImportError> {
        let parent_root = &block.header.header().prefix.parent_root;

        let (parent_header, parent_block_mmr, parent_block_import_status) =
            if let Some((parent_header, parent_block_details)) =
                self.chain_info.header_with_details(parent_root)
            {
                (
                    parent_header,
                    parent_block_details.mmr_with_block,
                    ParentBlockImportStatus::Imported {
                        system_contract_states: parent_block_details.system_contract_states,
                    },
                )
            } else if let Some(importing_entry) = self.importing_blocks.get(parent_root) {
                (
                    importing_entry.header().clone(),
                    Arc::clone(importing_entry.mmr()),
                    ParentBlockImportStatus::Importing {
                        entry: importing_entry,
                    },
                )
            } else {
                return Err(BlockImportError::UnknownParentBlock {
                    block_root: *parent_root,
                });
            };

        let parent_block_mmr_root = Blake3Hash::from(
            parent_block_mmr
                .root()
                .ok_or(BlockImportError::ParentBlockMmrInvalid)?,
        );
        let mut block_mmr = *parent_block_mmr;

        if !block_mmr.add_leaf(&block.header.header().root()) {
            return Err(BlockImportError::CantExtendMmr);
        }

        let importing_handle = self
            .importing_blocks
            .insert(block.header.clone(), Arc::new(block_mmr))
            .ok_or(BlockImportError::AlreadyImporting)?;

        if self
            .chain_info
            .header(&block.header.header().root())
            .is_some()
        {
            return Err(BlockImportError::AlreadyImported);
        }

        Ok(self.import(
            parent_header,
            parent_block_mmr_root,
            block,
            origin,
            importing_handle,
            parent_block_import_status,
        ))
    }
}

impl<PosTable, CI, BV> BeaconChainBlockImport<PosTable, CI, BV>
where
    PosTable: Table,
    CI: ChainInfoWrite<OwnedBeaconChainBlock>,
    BV: BlockVerification<OwnedBeaconChainBlock>,
{
    /// Create a new instance
    #[inline(always)]
    pub fn new(
        chain_info: CI,
        block_verification: BV,
        block_importing_notification_sender: mpsc::Sender<BlockImportingNotification>,
        block_import_notification_sender: mpsc::Sender<OwnedBeaconChainBlock>,
    ) -> Self {
        Self {
            chain_info,
            block_verification,
            importing_blocks: ImportingBlocks::new(),
            block_importing_notification_sender,
            block_import_notification_sender,
            _pos_table: PhantomData,
        }
    }

    async fn import(
        &self,
        parent_header: OwnedBeaconChainHeader,
        parent_block_mmr_root: Blake3Hash,
        block: OwnedBeaconChainBlock,
        origin: BlockOrigin,
        importing_handle: ImportingBlockHandle<OwnedBeaconChainHeader>,
        parent_block_import_status: ParentBlockImportStatus<OwnedBeaconChainHeader>,
    ) -> Result<(), BlockImportError> {
        let parent_header = parent_header.header();
        let header = block.header.header();
        let body = block.body.body();

        let log_block_import = match origin {
            BlockOrigin::LocalBlockBuilder { .. } => true,
            BlockOrigin::Sync => false,
            BlockOrigin::Broadcast => true,
        };

        // TODO: `.send()` is a hack for compiler bug, see:
        //  https://github.com/rust-lang/rust/issues/100013#issuecomment-2210995259
        self.block_verification
            .verify_concurrent(
                parent_header,
                &parent_block_mmr_root,
                header,
                body,
                &origin,
                &VerificationChainInfo {
                    chain_info: &self.chain_info,
                    importing_blocks: &self.importing_blocks,
                },
            )
            .send()
            .await
            .map_err(BeaconChainBlockImportError::from)?;

        let Some(system_contract_states) = parent_block_import_status.wait().await else {
            return Err(BlockImportError::ParentBlockImportFailed);
        };

        // TODO: `.send()` is a hack for compiler bug, see:
        //  https://github.com/rust-lang/rust/issues/100013#issuecomment-2210995259
        self.block_verification
            .verify_sequential(parent_header, &parent_block_mmr_root, header, body, &origin)
            .send()
            .await
            .map_err(BeaconChainBlockImportError::from)?;

        let global_state = GlobalState::new(&system_contract_states);

        // TODO: Execute block

        let state_root = global_state.root();

        if header.result.state_root != state_root {
            return Err(BlockImportError::InvalidStateRoot {
                expected: state_root,
                actual: header.result.state_root,
            });
        }

        let system_contract_states = global_state.to_system_contract_states();

        let (acknowledgement_sender, mut acknowledgement_receiver) = mpsc::channel(0);
        if let Err(error) = self
            .block_importing_notification_sender
            .clone()
            .send(BlockImportingNotification {
                block_number: header.prefix.number,
                acknowledgement_sender,
            })
            .await
        {
            warn!(%error, "Failed to send block importing notification");
        }

        while acknowledgement_receiver.next().await.is_some() {
            // Wait for all the acknowledgements to arrive
        }

        let number = header.prefix.number;
        let root = *header.root();

        self.chain_info
            .persist_block(
                block.clone(),
                BlockDetails {
                    mmr_with_block: Arc::clone(importing_handle.mmr()),
                    system_contract_states: StdArc::clone(&system_contract_states),
                },
            )
            .await?;

        importing_handle.set_success(system_contract_states);

        if let Err(error) = self
            .block_import_notification_sender
            .clone()
            .send(block)
            .await
        {
            warn!(
                %error,
                block_number = %number,
                block_root = %root,
                "Failed to send block import notification"
            );
        }

        if log_block_import {
            info!(
                %number,
                %root,
                "🏆 Imported block",
            );
        }

        Ok(())
    }
}
