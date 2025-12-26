//! Utilities for working with contract files
#![no_std]

use ab_io_type::trivial_type::TrivialType;

// Header:
//   * magic: `ABC0`
//   * read-only section size: u32
//   * metadata offset: u32
//   * metadata size: u32
//   * host call address offset: u32 (0 means no host call)
//   * for each method in metadata:
//     * code offset: u32
//     * size: u32

/// Magic bytes at the beginning of the file
pub const CONTRACT_FILE_MAGIC: [u8; 4] = *b"ABC0";

/// Header of the contract file
#[derive(Debug, Clone, Copy, PartialEq, Eq, TrivialType)]
#[repr(C)]
pub struct ContractFileHeader {
    /// Always [`CONTRACT_FILE_MAGIC`]
    pub magic: [u8; 4],
    /// Size of the read-only section in bytes as stored in the file
    pub read_only_section_file_size: u32,
    /// Size of the read-only section in bytes as will be written to memory during execution.
    ///
    /// If larger than `read_only_section_file_size`, then zeroed padding needs to be added.
    pub read_only_section_memory_size: u32,
    /// Offset of the metadata section in bytes relative to the start of the file
    pub metadata_offset: u32,
    /// Size of the metadata section in bytes
    pub metadata_size: u32,
    /// Host call function offset in bytes relative to the start of the file.
    ///
    /// `0` means no host call.
    pub host_call_fn_offset: u32,
}

/// Metadata about each function of the contract that can be called from the outside
#[derive(Debug, Clone, Copy, PartialEq, Eq, TrivialType)]
#[repr(C)]
pub struct ContractFileFunctionMetadata {
    /// Offset of the function code in bytes relative to the start of the file
    pub offset: u32,
    /// Size of the function code in bytes
    pub size: u32,
}
