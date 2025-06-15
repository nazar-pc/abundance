use crate::{BlockImport, BlockImportError};
use ab_client_api::{BlockOrigin, ChainInfo};
use ab_client_block_verification::{BlockVerification, BlockVerificationError};
use ab_core_primitives::block::body::BeaconChainBody;
use ab_core_primitives::block::header::BeaconChainHeader;
use ab_core_primitives::block::owned::OwnedBeaconChainBlock;
use ab_proof_of_space::Table;
use send_future::SendFuture;
use std::marker::PhantomData;

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
    _pos_table: PhantomData<PosTable>,
}

impl<PosTable, CI, BV> BlockImport<OwnedBeaconChainBlock>
    for BeaconChainBlockImport<PosTable, CI, BV>
where
    PosTable: Table,
    CI: ChainInfo<OwnedBeaconChainBlock>,
    BV: BlockVerification<OwnedBeaconChainBlock>,
{
    fn import(
        &self,
        block: OwnedBeaconChainBlock,
        origin: BlockOrigin,
    ) -> Result<impl Future<Output = Result<(), BlockImportError>> + Send, BlockImportError> {
        let parent_root = &block.header.header().prefix.parent_root;
        // TODO: Check for queued blocks as a fallback for concurrent block import
        let parent_header =
            self.chain_info
                .header(parent_root)
                .ok_or(BlockImportError::UnknownParentBlock {
                    block_root: *parent_root,
                })?;

        // TODO: Store block header in the temporary storage, so the next block can be imported
        //  concurrently

        Ok(async move {
            self.import(
                parent_header.header(),
                block.header.header(),
                block.body.body(),
                origin,
            )
            .await
        })
    }
}

impl<PosTable, CI, BV> BeaconChainBlockImport<PosTable, CI, BV>
where
    PosTable: Table,
    CI: ChainInfo<OwnedBeaconChainBlock>,
    BV: BlockVerification<OwnedBeaconChainBlock>,
{
    /// Create new instance
    #[inline(always)]
    pub fn new(chain_info: CI, block_verification: BV) -> Self {
        Self {
            chain_info,
            block_verification,
            _pos_table: PhantomData,
        }
    }

    async fn import(
        &self,
        parent_header: &BeaconChainHeader<'_>,
        header: &BeaconChainHeader<'_>,
        body: &BeaconChainBody<'_>,
        origin: BlockOrigin,
    ) -> Result<(), BlockImportError> {
        // TODO: `.send()` is a hack for compiler bug, see:
        //  https://github.com/rust-lang/rust/issues/100013#issuecomment-2210995259
        self.block_verification
            .verify(parent_header, header, body, origin)
            .send()
            .await
            .map_err(BeaconChainBlockImportError::from)?;

        // TODO: Execute block

        // TODO: Check state root

        // TODO: Store block after successful import, unblock import of subsequent blocks, remove
        //  header from temporary storage

        Ok(())
    }
}
