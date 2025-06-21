use crate::importing_blocks::{ImportingBlockEntry, ImportingBlockHandle, ImportingBlocks};
use crate::{BlockImport, BlockImportError};
use ab_client_api::{BlockOrigin, ChainInfoWrite};
use ab_client_block_verification::{BlockVerification, BlockVerificationError};
use ab_core_primitives::block::header::owned::OwnedBeaconChainHeader;
use ab_core_primitives::block::owned::OwnedBeaconChainBlock;
use ab_core_primitives::hashes::Blake3Hash;
use ab_proof_of_space::Table;
use send_future::SendFuture;
use std::marker::PhantomData;
use std::sync::Arc;

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

        let (parent_header, parent_block_mmr, maybe_parent_importing_entry) =
            if let Some(parent_header) = self.chain_info.header(parent_root) {
                let parent_block_mmr = self
                    .chain_info
                    .mmr_with_block(parent_root)
                    .ok_or(BlockImportError::ParentBlockMmrMissing)?;

                (parent_header, parent_block_mmr, None)
            } else if let Some(importing_entry) = self.importing_blocks.get(parent_root) {
                (
                    importing_entry.header().clone(),
                    Arc::clone(importing_entry.mmr()),
                    Some(importing_entry),
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
            maybe_parent_importing_entry,
        ))
    }
}

impl<PosTable, CI, BV> BeaconChainBlockImport<PosTable, CI, BV>
where
    PosTable: Table,
    CI: ChainInfoWrite<OwnedBeaconChainBlock>,
    BV: BlockVerification<OwnedBeaconChainBlock>,
{
    /// Create new instance
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
        maybe_parent_importing_entry: Option<ImportingBlockEntry<OwnedBeaconChainHeader>>,
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

        if maybe_parent_importing_entry
            .as_ref()
            .map(ImportingBlockEntry::has_failed)
            .unwrap_or_default()
        {
            // Early exit to avoid extra work
            return Err(BlockImportError::ParentBlockImportFailed);
        }

        // TODO: Execute block

        if maybe_parent_importing_entry
            .as_ref()
            .map(ImportingBlockEntry::has_failed)
            .unwrap_or_default()
        {
            // Early exit to avoid extra work
            return Err(BlockImportError::ParentBlockImportFailed);
        }

        // TODO: Check state root

        // Wait for parent block to be imported successfully
        if let Some(parent_importing_entry) = maybe_parent_importing_entry
            && !parent_importing_entry.wait_success().await
        {
            return Err(BlockImportError::ParentBlockImportFailed);
        }

        self.chain_info
            .persist_block(block, Arc::clone(importing_handle.mmr()))
            .await?;

        importing_handle.set_success();

        Ok(())
    }
}
