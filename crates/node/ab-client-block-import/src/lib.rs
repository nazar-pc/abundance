pub mod segment_headers_store;

/// Error for [`BlockImport`]
#[derive(Debug, thiserror::Error)]
pub enum BlockImportError {
    // TODO: Error variants
}

/// Block import interface
pub trait BlockImport<Block> {
    /// Import provided block
    fn import(&self, block: Block) -> Result<(), BlockImportError>;
}
