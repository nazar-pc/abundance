//! Utilities for working with contract files

#![feature(
    bigint_helper_methods,
    const_option_ops,
    const_trait_impl,
    maybe_uninit_as_bytes,
    maybe_uninit_fill,
    trusted_len,
    const_try
)]
#![expect(incomplete_features, reason = "generic_const_exprs")]
// TODO: This feature is not actually used in this crate, but is added as a workaround for
//  https://github.com/rust-lang/rust/issues/141492
#![feature(generic_const_exprs)]
#![no_std]

pub mod contract_instruction_prototype;

use crate::contract_instruction_prototype::ContractInstructionPrototype;
use ab_contracts_common::metadata::decode::{
    MetadataDecoder, MetadataDecodingError, MetadataItem, MethodMetadataItem,
    MethodsMetadataDecoder,
};
use ab_io_type::trivial_type::TrivialType;
use ab_riscv_primitives::instruction::BaseInstruction;
use ab_riscv_primitives::registers::EReg;
use core::iter;
use core::iter::TrustedLen;
use core::mem::MaybeUninit;
use replace_with::replace_with_or_abort;
use tracing::{debug, trace};

/// Magic bytes at the beginning of the file
pub const CONTRACT_FILE_MAGIC: [u8; 4] = *b"ABC0";
/// A register type used by contracts
pub type ContractRegister = EReg<u64>;
/// An instruction type used by contracts
pub type ContractInstruction = ContractInstructionPrototype<ContractRegister>;

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
    pub metadata_size: u16,
    /// Number of methods in the contract
    pub num_methods: u16,
    /// Host call function offset in bytes relative to the start of the file.
    ///
    /// `0` means no host call.
    pub host_call_fn_offset: u32,
}

/// Metadata about each method of the contract that can be called from the outside
#[derive(Debug, Clone, Copy, PartialEq, Eq, TrivialType)]
#[repr(C)]
pub struct ContractFileMethodMetadata {
    /// Offset of the method code in bytes relative to the start of the file
    pub offset: u32,
    /// Size of the method code in bytes
    pub size: u32,
}

#[derive(Debug, Copy, Clone)]
pub struct ContractFileMethod<'a> {
    /// Address of the method in the contract memory
    pub address: u32,
    /// Method metadata item
    pub method_metadata_item: MethodMetadataItem<'a>,
    /// Method metadata bytes.
    ///
    /// Can be used to compute [`MethodFingerprint`].
    ///
    /// [`MethodFingerprint`]: ab_contracts_common::method::MethodFingerprint
    pub method_metadata_bytes: &'a [u8],
}

/// Error for [`ContractFile::parse()`]
#[derive(Debug, thiserror::Error)]
pub enum ContractFileParseError {
    /// The file is too large, must fit into `u32`
    #[error("The file is too large, must fit into `u32`: {file_size} bytes")]
    FileTooLarge {
        /// Actual file size
        file_size: usize,
    },
    /// The file does not have a header (not enough bytes)
    #[error("The file does not have a header (not enough bytes)")]
    NoHeader,
    /// The magic bytes in the header are incorrect
    #[error("The magic bytes in the header are incorrect")]
    WrongMagicBytes,
    /// The metadata section is out of bounds of the file
    #[error(
        "The metadata section is out of bounds of the file: offset {offset}, size {size}, file \
        size {file_size}"
    )]
    MetadataOutOfRange {
        /// Offset of the metadata section in bytes relative to the start of the file
        offset: u32,
        /// Size of the metadata section in bytes
        size: u16,
        /// Size of the file in bytes
        file_size: u32,
    },
    /// Failed to decode metadata item
    #[error("Failed to decode metadata item")]
    MetadataDecoding,
    /// The file is too small
    #[error(
        "The file is too small: num_methods {num_methods}, read_only_section_size \
        {read_only_section_size}, file_size {file_size}"
    )]
    FileTooSmall {
        /// Number of methods in the contract
        num_methods: u16,
        /// Size of the read-only section in bytes as stored in the file
        read_only_section_size: u32,
        /// Size of the file in bytes
        file_size: u32,
    },
    /// Method is unaligned
    #[error("Method is unaligned: file offset {file_offset}, memory address {memory_address}")]
    MethodUnaligned {
        /// Offset of the method in bytes relative to the start of the file
        file_offset: u32,
        /// Address of the method in bytes relative to the beginning of the initialized memory
        memory_address: u32,
    },
    /// Method offset is out of bounds of the file
    #[error(
        "Method offset is out of bounds of the file: offset {offset}, code section \
        offset {code_section_offset} file_size {file_size}"
    )]
    MethodOutOfRange {
        /// Offset of the method in bytes relative to the start of the file
        offset: u32,
        /// Offset of the code section in bytes relative to the start of the file
        code_section_offset: u32,
        /// Size of the file in bytes
        file_size: u32,
    },
    /// Host call function is unaligned
    #[error(
        "Host call function is unaligned: file offset {file_offset}, memory address \
        {memory_address}"
    )]
    HostCallFnUnaligned {
        /// Offset of the method in bytes relative to the start of the file
        file_offset: u32,
        /// Address of the method in bytes relative to the beginning of the initialized memory
        memory_address: u32,
    },
    /// The host call function offset is out of bounds of the file
    #[error(
        "The host call function offset is out of bounds of the file: offset {offset}, code section \
        offset {code_section_offset} file_size {file_size}"
    )]
    HostCallFnOutOfRange {
        /// Offset of the host call function in bytes relative to the start of the file
        offset: u32,
        /// Offset of the code section in bytes relative to the start of the file
        code_section_offset: u32,
        /// Size of the file in bytes
        file_size: u32,
    },
    /// Host call function doesn't have auipc + jalr tailcall pattern
    #[error("The host call function doesn't have auipc + jalr tailcall pattern: {first} {second}")]
    InvalidHostCallFnPattern {
        /// First instruction of the host call function
        first: ContractInstruction,
        /// Second instruction of the host call function
        second: ContractInstruction,
    },
    /// The read-only section file size is larger than the memory size
    #[error(
        "The read-only section file size is larger than the memory size: file_size {file_size}, \
        memory_size {memory_size}"
    )]
    InvalidReadOnlySizes {
        /// Size of the read-only section in bytes as stored in the file
        file_size: u32,
        /// Size of the read-only section in bytes as will be written to memory during execution
        memory_size: u32,
    },
    /// There are not enough methods in the header to match the number of methods in the actual
    /// metadata
    #[error(
        "There are not enough methods in the header to match the number of methods in the actual \
        metadata: header_num_methods {header_num_methods}, metadata_method_index \
        {metadata_method_index}"
    )]
    InsufficientHeaderMethods {
        /// Number of methods in the header
        header_num_methods: u16,
        /// Index of the method in the actual metadata that is missing from the header
        metadata_method_index: u16,
    },
    /// The number of methods in the header does not match the number of methods in the actual
    /// metadata
    #[error(
        "The number of methods in the header {header_num_methods} does not match the number of \
        methods in the actual metadata {metadata_num_methods}"
    )]
    MetadataNumMethodsMismatch {
        /// Number of methods in the header
        header_num_methods: u16,
        /// Number of methods in the actual metadata
        metadata_num_methods: u16,
    },
    /// Unexpected instruction encountered while parsing the code section
    #[error("Unexpected instruction encountered while parsing the code section: {instruction}")]
    UnexpectedInstruction {
        /// Instruction
        instruction: ContractInstruction,
    },
    /// The code section is empty
    #[error("The code section is empty")]
    CodeEmpty,
    /// Unexpected trailing code bytes encountered while parsing the code section
    #[error(
        "Unexpected trailing code bytes encountered while parsing the code section: {num_bytes} \
        trailing bytes"
    )]
    UnexpectedTrailingCodeBytes {
        /// Number of trailing bytes encountered
        num_bytes: usize,
    },
    /// The last instruction in the code section must be a jump instruction
    #[error("The last instruction in the code section must be a jump instruction: {instruction}")]
    LastInstructionMustBeJump {
        /// Instruction that is expected to be a jump instruction
        instruction: ContractInstruction,
    },
}

impl From<MetadataDecodingError<'_>> for ContractFileParseError {
    fn from(error: MetadataDecodingError<'_>) -> Self {
        debug!(?error, "Failed to decode metadata item");
        Self::MetadataDecoding
    }
}

// TODO: Description of the file format, assumptions and invariants
/// A container for a parsed contract file
#[derive(Debug)]
pub struct ContractFile<'a> {
    read_only_section_file_size: u32,
    read_only_section_memory_size: u32,
    num_methods: u16,
    bytes: &'a [u8],
}

impl<'a> ContractFile<'a> {
    /// Parse file bytes and verify that internal invariants are valid.
    ///
    /// `contract_method` argument is an optional callback called for each method in the contract
    /// file with its method address in the contract memory, metadata item, and corresponding
    /// metadata bytes. This can be used to collect available methods during parsing and avoid extra
    /// iteration later using [`Self::iterate_methods()`] to compute [`MethodFingerprint`], etc.
    ///
    /// [`MethodFingerprint`]: ab_contracts_common::method::MethodFingerprint
    pub fn parse<CM>(
        file_bytes: &'a [u8],
        mut contract_method: CM,
    ) -> Result<Self, ContractFileParseError>
    where
        CM: FnMut(ContractFileMethod<'a>) -> Result<(), ContractFileParseError>,
    {
        let file_size = u32::try_from(file_bytes.len()).map_err(|_error| {
            ContractFileParseError::FileTooLarge {
                file_size: file_bytes.len(),
            }
        })?;
        let (header_bytes, after_header_bytes) = file_bytes
            .split_at_checked(size_of::<ContractFileHeader>())
            .ok_or(ContractFileParseError::NoHeader)?;
        // SAFETY: Size is correct, content is checked below
        let header = unsafe { ContractFileHeader::read_unaligned_unchecked(header_bytes) };

        if header.magic != CONTRACT_FILE_MAGIC {
            return Err(ContractFileParseError::WrongMagicBytes);
        }

        if header.read_only_section_file_size > header.read_only_section_memory_size {
            return Err(ContractFileParseError::InvalidReadOnlySizes {
                file_size: header.read_only_section_file_size,
                memory_size: header.read_only_section_memory_size,
            });
        }

        let metadata_bytes = file_bytes
            .get(header.metadata_offset as usize..)
            .ok_or(ContractFileParseError::MetadataOutOfRange {
                offset: header.metadata_offset,
                size: header.metadata_size,
                file_size,
            })?
            .get(..header.metadata_size as usize)
            .ok_or(ContractFileParseError::MetadataOutOfRange {
                offset: header.metadata_offset,
                size: header.metadata_size,
                file_size,
            })?;

        let read_only_padding_size =
            header.read_only_section_memory_size - header.read_only_section_file_size;
        let read_only_section_offset = ContractFileHeader::SIZE
            + u32::from(header.num_methods) * ContractFileMethodMetadata::SIZE;
        let code_section_offset =
            read_only_section_offset.saturating_add(header.read_only_section_file_size);

        {
            let mut contract_file_methods_metadata_iter = {
                let mut file_contract_metadata_bytes = after_header_bytes;

                (0..header.num_methods).map(move |_| {
                    let contract_file_method_metadata_bytes = file_contract_metadata_bytes
                        .split_off(..size_of::<ContractFileMethodMetadata>())
                        .ok_or(ContractFileParseError::FileTooSmall {
                            num_methods: header.num_methods,
                            read_only_section_size: header.read_only_section_file_size,
                            file_size,
                        })?;
                    // SAFETY: The number of bytes is correct, content is checked below
                    let contract_file_method_metadata = unsafe {
                        ContractFileMethodMetadata::read_unaligned_unchecked(
                            contract_file_method_metadata_bytes,
                        )
                    };

                    if (contract_file_method_metadata.offset + contract_file_method_metadata.size)
                        > file_size
                    {
                        return Err(ContractFileParseError::FileTooSmall {
                            num_methods: header.num_methods,
                            read_only_section_size: header.read_only_section_file_size,
                            file_size,
                        });
                    }

                    if contract_file_method_metadata.offset < code_section_offset {
                        return Err(ContractFileParseError::MethodOutOfRange {
                            offset: contract_file_method_metadata.offset,
                            code_section_offset,
                            file_size,
                        });
                    }

                    Ok(contract_file_method_metadata)
                })
            };

            let mut metadata_num_methods = 0;
            let mut remaining_metadata_bytes = metadata_bytes;
            let mut metadata_decoder = MetadataDecoder::new(metadata_bytes);

            while let Some(maybe_metadata_item) = metadata_decoder.decode_next() {
                let metadata_item = maybe_metadata_item?;
                trace!(?metadata_item, "Decoded metadata item");

                let mut methods_metadata_decoder = metadata_item.into_decoder();
                loop {
                    // This is used instead of `while let Some(method_metadata_decoder)` because the
                    // compiler is not smart enough to understand where `method_metadata_decoder` is
                    // dropped
                    let Some(method_metadata_decoder) = methods_metadata_decoder.decode_next()
                    else {
                        break;
                    };

                    let before_remaining_bytes = method_metadata_decoder.remaining_metadata_bytes();
                    let (_, method_metadata_item) = method_metadata_decoder.decode_next()?;

                    trace!(?method_metadata_item, "Decoded method metadata item");
                    metadata_num_methods += 1;

                    let method_metadata_bytes = remaining_metadata_bytes
                        .split_off(
                            ..before_remaining_bytes
                                - methods_metadata_decoder.remaining_metadata_bytes(),
                        )
                        .ok_or(MetadataDecodingError::NotEnoughMetadata)?;

                    let contract_file_method_metadata = contract_file_methods_metadata_iter
                        .next()
                        .ok_or(ContractFileParseError::InsufficientHeaderMethods {
                            header_num_methods: header.num_methods,
                            metadata_method_index: metadata_num_methods - 1,
                        })??;
                    let address = contract_file_method_metadata.offset - read_only_section_offset
                        + read_only_padding_size;

                    if !address.is_multiple_of(size_of::<u32>() as u32) {
                        return Err(ContractFileParseError::MethodUnaligned {
                            file_offset: contract_file_method_metadata.offset,
                            memory_address: address,
                        });
                    }

                    contract_method(ContractFileMethod {
                        address,
                        method_metadata_item,
                        method_metadata_bytes,
                    })?;
                }
            }

            if metadata_num_methods != header.num_methods {
                return Err(ContractFileParseError::MetadataNumMethodsMismatch {
                    header_num_methods: header.num_methods,
                    metadata_num_methods,
                });
            }
        }

        if code_section_offset >= file_size {
            return Err(ContractFileParseError::FileTooSmall {
                num_methods: header.num_methods,
                read_only_section_size: header.read_only_section_file_size,
                file_size,
            });
        }

        if header.host_call_fn_offset != 0 {
            if header.host_call_fn_offset >= file_size
                || header.host_call_fn_offset < code_section_offset
            {
                return Err(ContractFileParseError::HostCallFnOutOfRange {
                    offset: header.host_call_fn_offset,
                    code_section_offset,
                    file_size,
                });
            }

            let instructions_bytes = file_bytes
                .get(header.host_call_fn_offset as usize..)
                .ok_or(ContractFileParseError::HostCallFnOutOfRange {
                    offset: header.host_call_fn_offset,
                    code_section_offset,
                    file_size,
                })?
                .get(..size_of::<[u32; 2]>())
                .ok_or(ContractFileParseError::HostCallFnOutOfRange {
                    offset: header.host_call_fn_offset,
                    code_section_offset,
                    file_size,
                })?;

            let first_instruction = u32::from_le_bytes([
                instructions_bytes[0],
                instructions_bytes[1],
                instructions_bytes[2],
                instructions_bytes[3],
            ]);
            let second_instruction = u32::from_le_bytes([
                instructions_bytes[4],
                instructions_bytes[5],
                instructions_bytes[6],
                instructions_bytes[7],
            ]);

            let first = ContractInstruction::decode(first_instruction);
            let second = ContractInstruction::decode(second_instruction);

            // TODO: Should it be canonicalized to a fixed immediate and temporary after conversion
            //  from ELF?
            // Checks if two consecutive instructions are:
            //   auipc x?, 0x?
            //   jalr  x0, offset(x?)
            let matches_expected_pattern = if let (
                ContractInstruction::Auipc {
                    rd: auipc_rd,
                    imm: _,
                },
                ContractInstruction::Jalr {
                    rd: jalr_rd,
                    rs1: jalr_rs1,
                    imm: _,
                },
            ) = (first, second)
            {
                auipc_rd == jalr_rs1 && jalr_rd == ContractRegister::Zero
            } else {
                false
            };

            if !matches_expected_pattern {
                return Err(ContractFileParseError::InvalidHostCallFnPattern { first, second });
            }

            let address =
                header.host_call_fn_offset - read_only_section_offset + read_only_padding_size;

            if !address.is_multiple_of(size_of::<u32>() as u32) {
                return Err(ContractFileParseError::HostCallFnUnaligned {
                    file_offset: header.host_call_fn_offset,
                    memory_address: address,
                });
            }
        }

        // Ensure code only consists of expected instructions
        {
            let mut remaining_code_file_bytes = &file_bytes[code_section_offset as usize..];

            let mut instruction = ContractInstruction::Unimp;
            while let Some(instruction_bytes) =
                remaining_code_file_bytes.split_off(..size_of::<u32>())
            {
                instruction = ContractInstruction::decode(u32::from_le_bytes([
                    instruction_bytes[0],
                    instruction_bytes[1],
                    instruction_bytes[2],
                    instruction_bytes[3],
                ]));
                // Intentionally exhaustive list of all instructions
                match instruction {
                    ContractInstruction::Mul { .. }
                    | ContractInstruction::Mulh { .. }
                    | ContractInstruction::Mulhsu { .. }
                    | ContractInstruction::Mulhu { .. }
                    | ContractInstruction::Div { .. }
                    | ContractInstruction::Divu { .. }
                    | ContractInstruction::Rem { .. }
                    | ContractInstruction::Remu { .. }
                    | ContractInstruction::Mulw { .. }
                    | ContractInstruction::Divw { .. }
                    | ContractInstruction::Divuw { .. }
                    | ContractInstruction::Remw { .. }
                    | ContractInstruction::Remuw { .. }
                    | ContractInstruction::AddUw { .. }
                    | ContractInstruction::Sh1add { .. }
                    | ContractInstruction::Sh1addUw { .. }
                    | ContractInstruction::Sh2add { .. }
                    | ContractInstruction::Sh2addUw { .. }
                    | ContractInstruction::Sh3add { .. }
                    | ContractInstruction::Sh3addUw { .. }
                    | ContractInstruction::SlliUw { .. }
                    | ContractInstruction::Andn { .. }
                    | ContractInstruction::Orn { .. }
                    | ContractInstruction::Xnor { .. }
                    | ContractInstruction::Clz { .. }
                    | ContractInstruction::Clzw { .. }
                    | ContractInstruction::Ctz { .. }
                    | ContractInstruction::Ctzw { .. }
                    | ContractInstruction::Cpop { .. }
                    | ContractInstruction::Cpopw { .. }
                    | ContractInstruction::Max { .. }
                    | ContractInstruction::Maxu { .. }
                    | ContractInstruction::Min { .. }
                    | ContractInstruction::Minu { .. }
                    | ContractInstruction::Sextb { .. }
                    | ContractInstruction::Sexth { .. }
                    | ContractInstruction::Zexth { .. }
                    | ContractInstruction::Rol { .. }
                    | ContractInstruction::Rolw { .. }
                    | ContractInstruction::Ror { .. }
                    | ContractInstruction::Rori { .. }
                    | ContractInstruction::Roriw { .. }
                    | ContractInstruction::Rorw { .. }
                    | ContractInstruction::Orcb { .. }
                    | ContractInstruction::Rev8 { .. }
                    | ContractInstruction::Bset { .. }
                    | ContractInstruction::Bseti { .. }
                    | ContractInstruction::Bclr { .. }
                    | ContractInstruction::Bclri { .. }
                    | ContractInstruction::Binv { .. }
                    | ContractInstruction::Binvi { .. }
                    | ContractInstruction::Bext { .. }
                    | ContractInstruction::Bexti { .. }
                    | ContractInstruction::Clmul { .. }
                    | ContractInstruction::Clmulh { .. }
                    | ContractInstruction::Clmulr { .. }
                    | ContractInstruction::Add { .. }
                    | ContractInstruction::Sub { .. }
                    | ContractInstruction::Sll { .. }
                    | ContractInstruction::Slt { .. }
                    | ContractInstruction::Sltu { .. }
                    | ContractInstruction::Xor { .. }
                    | ContractInstruction::Srl { .. }
                    | ContractInstruction::Sra { .. }
                    | ContractInstruction::Or { .. }
                    | ContractInstruction::And { .. }
                    | ContractInstruction::Addw { .. }
                    | ContractInstruction::Subw { .. }
                    | ContractInstruction::Sllw { .. }
                    | ContractInstruction::Srlw { .. }
                    | ContractInstruction::Sraw { .. }
                    | ContractInstruction::Addi { .. }
                    | ContractInstruction::Slti { .. }
                    | ContractInstruction::Sltiu { .. }
                    | ContractInstruction::Xori { .. }
                    | ContractInstruction::Ori { .. }
                    | ContractInstruction::Andi { .. }
                    | ContractInstruction::Slli { .. }
                    | ContractInstruction::Srli { .. }
                    | ContractInstruction::Srai { .. }
                    | ContractInstruction::Addiw { .. }
                    | ContractInstruction::Slliw { .. }
                    | ContractInstruction::Srliw { .. }
                    | ContractInstruction::Sraiw { .. }
                    | ContractInstruction::Lb { .. }
                    | ContractInstruction::Lh { .. }
                    | ContractInstruction::Lw { .. }
                    | ContractInstruction::Ld { .. }
                    | ContractInstruction::Lbu { .. }
                    | ContractInstruction::Lhu { .. }
                    | ContractInstruction::Lwu { .. }
                    | ContractInstruction::Jalr { .. }
                    | ContractInstruction::Sb { .. }
                    | ContractInstruction::Sh { .. }
                    | ContractInstruction::Sw { .. }
                    | ContractInstruction::Sd { .. }
                    | ContractInstruction::Beq { .. }
                    | ContractInstruction::Bne { .. }
                    | ContractInstruction::Blt { .. }
                    | ContractInstruction::Bge { .. }
                    | ContractInstruction::Bltu { .. }
                    | ContractInstruction::Bgeu { .. }
                    | ContractInstruction::Lui { .. }
                    | ContractInstruction::Auipc { .. }
                    | ContractInstruction::Jal { .. }
                    | ContractInstruction::Ebreak
                    | ContractInstruction::Unimp => {
                        // Expected instruction
                    }
                    ContractInstruction::Fence { .. }
                    | ContractInstruction::Ecall
                    | ContractInstruction::Invalid(_) => {
                        return Err(ContractFileParseError::UnexpectedInstruction { instruction });
                    }
                }
            }

            let is_jump_instruction = matches!(
                instruction,
                ContractInstruction::Jalr { .. }
                    | ContractInstruction::Beq { .. }
                    | ContractInstruction::Bne { .. }
                    | ContractInstruction::Blt { .. }
                    | ContractInstruction::Bge { .. }
                    | ContractInstruction::Bltu { .. }
                    | ContractInstruction::Bgeu { .. }
                    | ContractInstruction::Jal { .. }
            );
            if !is_jump_instruction {
                return Err(ContractFileParseError::LastInstructionMustBeJump { instruction });
            }

            if !remaining_code_file_bytes.is_empty() {
                return Err(ContractFileParseError::UnexpectedTrailingCodeBytes {
                    num_bytes: remaining_code_file_bytes.len(),
                });
            }
        }

        Ok(Self {
            read_only_section_file_size: header.read_only_section_file_size,
            read_only_section_memory_size: header.read_only_section_memory_size,
            num_methods: header.num_methods,
            bytes: file_bytes,
        })
    }

    /// Similar to [`ContractFile::parse()`] but does not verify internal invariants and assumes the
    /// input is valid.
    ///
    /// This method is more efficient and does no checks that [`ContractFile::parse()`] does.
    ///
    /// # Safety
    /// Must be a valid input, for example, previously verified using [`ContractFile::parse()`].
    pub unsafe fn parse_unchecked(file_bytes: &'a [u8]) -> Self {
        // SAFETY: Unchecked method assumed input is correct
        let header = unsafe { ContractFileHeader::read_unaligned_unchecked(file_bytes) };

        Self {
            read_only_section_file_size: header.read_only_section_file_size,
            read_only_section_memory_size: header.read_only_section_memory_size,
            num_methods: header.num_methods,
            bytes: file_bytes,
        }
    }

    /// Get file header
    #[inline(always)]
    pub fn header(&self) -> ContractFileHeader {
        // SAFETY: Protected internal invariant checked in constructor
        unsafe { ContractFileHeader::read_unaligned_unchecked(self.bytes) }
    }

    /// Metadata stored in the file
    #[inline]
    pub fn metadata_bytes(&self) -> &[u8] {
        let header = self.header();
        // SAFETY: Protected internal invariant checked in constructor
        unsafe {
            self.bytes
                .get_unchecked(header.metadata_offset as usize..)
                .get_unchecked(..header.metadata_size as usize)
        }
    }

    /// Memory allocation required for the contract
    #[inline]
    pub fn contract_memory_size(&self) -> u32 {
        let read_only_section_offset = ContractFileHeader::SIZE
            + u32::from(self.num_methods) * ContractFileMethodMetadata::SIZE;
        let read_only_padding_size =
            self.read_only_section_memory_size - self.read_only_section_file_size;
        self.bytes.len() as u32 - read_only_section_offset + read_only_padding_size
    }

    /// Initialize contract memory with file contents.
    ///
    /// Use [`Self::contract_memory_size()`] to identify the exact necessary amount of memory.
    #[must_use = "Must check that contract memory was large enough"]
    pub fn initialize_contract_memory(&self, mut contract_memory: &mut [MaybeUninit<u8>]) -> bool {
        let contract_memory_input_size = contract_memory.len();
        let read_only_section_offset = ContractFileHeader::SIZE
            + u32::from(self.num_methods) * ContractFileMethodMetadata::SIZE;
        let read_only_padding_size =
            self.read_only_section_memory_size - self.read_only_section_file_size;

        // SAFETY: Protected internal invariant checked in constructor
        let source_bytes = unsafe {
            self.bytes
                .get_unchecked(read_only_section_offset as usize..)
        };

        // Simple case: memory exactly matches the file-backed sections
        if contract_memory.len() == source_bytes.len() {
            contract_memory.write_copy_of_slice(source_bytes);
            return true;
        }

        let Some(read_only_file_target_bytes) =
            contract_memory.split_off_mut(..self.read_only_section_file_size as usize)
        else {
            trace!(
                %contract_memory_input_size,
                contract_memory_size = %self.contract_memory_size(),
                read_only_section_file_size = self.read_only_section_file_size,
                "Not enough bytes to write read-only section from the file"
            );

            return false;
        };

        // SAFETY: Protected internal invariant checked in constructor
        let (read_only_file_source_bytes, code_source_bytes) =
            unsafe { source_bytes.split_at_unchecked(self.read_only_section_file_size as usize) };
        // Write read-only data
        read_only_file_target_bytes.write_copy_of_slice(read_only_file_source_bytes);

        let Some(read_only_padding_bytes) =
            contract_memory.split_off_mut(..read_only_padding_size as usize)
        else {
            trace!(
                %contract_memory_input_size,
                contract_memory_size = %self.contract_memory_size(),
                read_only_section_file_size = self.read_only_section_file_size,
                read_only_section_memory_size = self.read_only_section_memory_size,
                %read_only_padding_size,
                "Not enough bytes to write read-only padding section"
            );

            return false;
        };

        // Write read-only padding
        read_only_padding_bytes.write_filled(0);

        if code_source_bytes.len() != contract_memory.len() {
            trace!(
                %contract_memory_input_size,
                contract_memory_size = %self.contract_memory_size(),
                read_only_section_file_size = self.read_only_section_file_size,
                read_only_section_memory_size = self.read_only_section_memory_size,
                %read_only_padding_size,
                code_size = %code_source_bytes.len(),
                "Not enough bytes to write code section from the file"
            );

            return false;
        }

        contract_memory.write_copy_of_slice(code_source_bytes);

        true
    }

    /// Get the complete code section with instructions
    pub fn get_code(&self) -> &[u8] {
        let read_only_section_offset = ContractFileHeader::SIZE
            + u32::from(self.num_methods) * ContractFileMethodMetadata::SIZE;

        // SAFETY: Protected internal invariant checked in constructor
        let source_bytes = unsafe {
            self.bytes
                .get_unchecked(read_only_section_offset as usize..)
        };

        // SAFETY: Protected internal invariant checked in constructor
        let (_read_only_file_source_bytes, code_source_bytes) =
            unsafe { source_bytes.split_at_unchecked(self.read_only_section_file_size as usize) };

        code_source_bytes
    }

    /// Iterate over all methods in the contract
    pub fn iterate_methods(
        &self,
    ) -> impl ExactSizeIterator<Item = ContractFileMethod<'_>> + TrustedLen {
        let metadata_bytes = self.metadata_bytes();

        #[ouroboros::self_referencing]
        struct MethodsMetadataIterState<'metadata> {
            metadata_decoder: MetadataDecoder<'metadata>,
            #[borrows(mut metadata_decoder)]
            #[covariant]
            methods_metadata_decoder: Option<MethodsMetadataDecoder<'this, 'metadata>>,
        }

        let metadata_decoder = MetadataDecoder::new(metadata_bytes);

        let mut methods_metadata_state =
            MethodsMetadataIterState::new(metadata_decoder, |metadata_decoder| {
                metadata_decoder
                    .decode_next()
                    .and_then(Result::ok)
                    .map(MetadataItem::into_decoder)
            });

        let mut metadata_methods_iter = iter::from_fn(move || {
            loop {
                let maybe_next_item = methods_metadata_state.with_methods_metadata_decoder_mut(
                    |maybe_methods_metadata_decoder| {
                        let methods_metadata_decoder = maybe_methods_metadata_decoder.as_mut()?;
                        let method_metadata_decoder = methods_metadata_decoder.decode_next()?;

                        let before_remaining_bytes =
                            method_metadata_decoder.remaining_metadata_bytes();

                        let (_, method_metadata_item) = method_metadata_decoder
                            .decode_next()
                            .expect("Input is valid according to function contract; qed");

                        // SAFETY: Protected internal invariant checked in constructor
                        let method_metadata_bytes = unsafe {
                            metadata_bytes
                                .get_unchecked(metadata_bytes.len() - before_remaining_bytes..)
                                .get_unchecked(
                                    ..before_remaining_bytes
                                        - methods_metadata_decoder.remaining_metadata_bytes(),
                                )
                        };

                        Some((method_metadata_item, method_metadata_bytes))
                    },
                );

                if let Some(next_item) = maybe_next_item {
                    return Some(next_item);
                }

                // Process methods of the next contract/trait
                replace_with_or_abort(&mut methods_metadata_state, |methods_metadata_state| {
                    let metadata_decoder = methods_metadata_state.into_heads().metadata_decoder;
                    MethodsMetadataIterState::new(metadata_decoder, |metadata_decoder| {
                        metadata_decoder
                            .decode_next()
                            .and_then(Result::ok)
                            .map(MetadataItem::into_decoder)
                    })
                });

                if methods_metadata_state
                    .borrow_methods_metadata_decoder()
                    .is_none()
                {
                    return None;
                }
            }
        });

        let read_only_padding_size =
            self.read_only_section_memory_size - self.read_only_section_file_size;
        // SAFETY: Protected internal invariant checked in constructor
        let contract_file_methods_metadata_bytes =
            unsafe { self.bytes.get_unchecked(size_of::<ContractFileHeader>()..) };

        (0..self.num_methods).map(move |method_index| {
            // SAFETY: Protected internal invariant checked in constructor
            let contract_file_method_metadata_bytes = unsafe {
                contract_file_methods_metadata_bytes
                    .get_unchecked(
                        method_index as usize * size_of::<ContractFileMethodMetadata>()..,
                    )
                    .get_unchecked(..size_of::<ContractFileMethodMetadata>())
            };
            // SAFETY: Protected internal invariant checked in constructor
            let contract_file_method_metadata = unsafe {
                ContractFileMethodMetadata::read_unaligned_unchecked(
                    contract_file_method_metadata_bytes,
                )
            };

            let (method_metadata_item, method_metadata_bytes) = metadata_methods_iter
                .next()
                .expect("Protected internal invariant checked in constructor; qed");

            ContractFileMethod {
                address: contract_file_method_metadata.offset + read_only_padding_size,
                method_metadata_item,
                method_metadata_bytes,
            }
        })
    }
}
