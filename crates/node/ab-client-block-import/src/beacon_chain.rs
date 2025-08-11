use crate::importing_blocks::{ImportingBlockHandle, ImportingBlocks, ParentBlockImportStatus};
use crate::{BlockImport, BlockImportError};
use ab_client_api::{BlockDetails, BlockOrigin, ChainInfoWrite};
use ab_client_block_verification::{BlockVerification, BlockVerificationError};
use ab_client_consensus_common::state::GlobalState;
use ab_core_primitives::block::header::owned::OwnedBeaconChainHeader;
use ab_core_primitives::block::owned::OwnedBeaconChainBlock;
use ab_core_primitives::hashes::Blake3Hash;
use ab_proof_of_space::Table;
use rclite::Arc;
use send_future::SendFuture;
use std::marker::PhantomData;
use std::sync::Arc as StdArc;

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

#[derive(Debug)]
pub struct BeaconChainBlockImport<PosTable, CI, BV> {
    chain_info: CI,
    block_verification: BV,
    importing_blocks: ImportingBlocks<OwnedBeaconChainHeader>,
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
    pub fn new(chain_info: CI, block_verification: BV) -> Self {
        Self {
            chain_info,
            block_verification,
            importing_blocks: ImportingBlocks::new(),
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

        // TODO: `.send()` is a hack for compiler bug, see:
        //  https://github.com/rust-lang/rust/issues/100013#issuecomment-2210995259
        self.block_verification
            .verify(parent_header, &parent_block_mmr_root, header, body, origin)
            .send()
            .await
            .map_err(BeaconChainBlockImportError::from)?;

        if parent_block_import_status.has_failed() {
            // Early exit to avoid extra work
            return Err(BlockImportError::ParentBlockImportFailed);
        }

        let Some(system_contract_states) = parent_block_import_status.wait().await else {
            return Err(BlockImportError::ParentBlockImportFailed);
        };

        let global_state = GlobalState::new(&system_contract_states);

        // TODO: Execute block

        let state_root = global_state.root();

        if header.result.state_root == state_root {
            return Err(BlockImportError::InvalidStateRoot {
                expected: state_root,
                actual: header.result.state_root,
            });
        }

        let system_contract_states = global_state.to_system_contract_states();

        self.chain_info
            .persist_block(
                block,
                BlockDetails {
                    mmr_with_block: Arc::clone(importing_handle.mmr()),
                    system_contract_states: StdArc::clone(&system_contract_states),
                },
            )
            .await?;

        importing_handle.set_success(system_contract_states);

        Ok(())
    }
}
