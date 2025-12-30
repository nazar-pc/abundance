//! Convert RISC-V ELF `cdylib` into Abundance contract file format

use ab_aligned_buffer::SharedAlignedBuffer;
use ab_contract_file::{CONTRACT_FILE_MAGIC, ContractFileHeader, ContractFileMethodMetadata};
use ab_contracts_common::metadata::decode::MetadataDecoder;
use ab_contracts_common::{HOST_CALL_FN, HOST_CALL_FN_IMPORT, METADATA_STATIC_NAME_PREFIX};
use ab_io_type::trivial_type::TrivialType;
use anyhow::Context;
use object::elf::{
    EF_RISCV_RVE, ELFCLASS64, ELFDATA2LSB, ELFMAG, ELFOSABI_GNU, EM_RISCV, ET_DYN, FileHeader64,
    Ident, R_RISCV_JUMP_SLOT, SHN_LORESERVE, STB_GLOBAL, STV_DEFAULT,
};
use object::read::elf::{ElfFile, ElfFile64};
use object::{
    CompressedData, CompressionFormat, LittleEndian, Object, ObjectSection, ObjectSymbol,
    ObjectSymbolTable, RelocationFlags, RelocationTarget, SymbolKind, SymbolSection, U16, U32, U64,
};
use std::collections::HashMap;
use tracing::{debug, trace};

fn is_correct_header(header: &FileHeader64<LittleEndian>) -> bool {
    let expected_header = FileHeader64 {
        e_ident: Ident {
            magic: ELFMAG,
            class: ELFCLASS64,
            data: ELFDATA2LSB,
            version: 1,
            os_abi: ELFOSABI_GNU,
            abi_version: 0,
            padding: [0; _],
        },
        e_type: U16::new(LittleEndian, ET_DYN),
        e_machine: U16::new(LittleEndian, EM_RISCV),
        e_version: U32::new(LittleEndian, 1),
        e_entry: U64::new(LittleEndian, 0),
        e_phoff: header.e_phoff,
        e_shoff: header.e_shoff,
        e_flags: U32::new(LittleEndian, EF_RISCV_RVE),
        e_ehsize: U16::new(LittleEndian, 64),
        e_phentsize: header.e_phentsize,
        e_phnum: header.e_phnum,
        e_shentsize: header.e_shentsize,
        e_shnum: header.e_shnum,
        e_shstrndx: header.e_shstrndx,
    };

    // Should have been just `==`, but https://github.com/gimli-rs/object/issues/830
    object::pod::bytes_of(header) == object::pod::bytes_of(&expected_header)
}

fn check_relocations(elf: &ElfFile<'_, FileHeader64<LittleEndian>>) -> anyhow::Result<()> {
    let mut dynamic_relocations = elf.dynamic_relocations().into_iter().flatten();
    let maybe_first_relocation = dynamic_relocations.next();

    if dynamic_relocations.next().is_some() {
        return Err(anyhow::anyhow!(
            "Only a single PLT relocation for host function call import is allowed, make sure to \
            build an optimized cdylib"
        ));
    }

    let Some((address, relocation)) = maybe_first_relocation else {
        return Ok(());
    };

    debug!(
        %address,
        ?relocation,
        "Found a single relocation"
    );

    // TODO: There is no such relocation in `object` crate yet:
    //  https://github.com/gimli-rs/object/issues/833
    if relocation.flags()
        != (RelocationFlags::Elf {
            r_type: R_RISCV_JUMP_SLOT,
        })
    {
        return Err(anyhow::anyhow!("Unexpected relocation: {relocation:?}"));
    }

    let RelocationTarget::Symbol(symbol_index) = relocation.target() else {
        return Err(anyhow::anyhow!(
            "Only a single PLT relocation for host function call import is allowed, make sure to \
            build an optimized cdylib"
        ));
    };

    let sym = elf
        .dynamic_symbol_table()
        .context("Failed to get dynamic symbol table")?
        .symbol_by_index(symbol_index)
        .context("Failed to get relocation symbol by its index")?;

    let name = sym
        .name()
        .with_context(|| format!("Failed to get relocation symbol name: {relocation:?} {sym:?}"))?;
    debug!(
        %name,
        "PLT relocation name"
    );

    if name != HOST_CALL_FN_IMPORT {
        return Err(anyhow::anyhow!(
            "Unexpected PLT relocation name {name}: {relocation:?} {sym:?}"
        ));
    }

    if relocation.addend() != 0 || relocation.has_implicit_addend() {
        return Err(anyhow::anyhow!(
            "Unexpected PLT relocation {name}: {relocation:?} {sym:?}"
        ));
    }

    Ok(())
}

#[derive(Debug, Copy, Clone)]
struct ParsedSections {
    metadata_offset: u64,
    metadata_size: u64,
    ro_data_offset: u64,
    ro_data_file_size: u64,
    ro_data_memory_size: u64,
    code_offset: u64,
    code_size: u64,
}

fn parse_sections(elf: &ElfFile<'_, FileHeader64<LittleEndian>>) -> anyhow::Result<ParsedSections> {
    let mut maybe_metadata_section = None;
    let mut maybe_rodata_section = None;
    let mut maybe_code_section = None;

    for section in elf.sections() {
        // TODO: This log is not very usable right now:
        //  https://github.com/gimli-rs/object/issues/834
        // trace!(?section, "Processing section");
        trace!(name = %section.name().unwrap_or_default(), "Processing section");

        match section.name().context("Failed to get section name")? {
            "ab-contract-metadata" => {
                let CompressedData {
                    format,
                    data: _,
                    uncompressed_size,
                } = section
                    .compressed_data()
                    .context("Failed to get section data")?;
                if !matches!(format, CompressionFormat::None) {
                    return Err(anyhow::anyhow!(
                        "Section `ab-contract-metadata` is compressed with {format:?}, but shouldn't be"
                    ));
                }
                if uncompressed_size != section.size() {
                    return Err(anyhow::anyhow!(
                        "Section `ab-contract-metadata` has unexpected paddings: file size \
                        {uncompressed_size} != in-memory size {}",
                        section.size()
                    ));
                }
                maybe_metadata_section.replace(section);
            }
            ".rodata" => {
                let CompressedData {
                    format,
                    data: _,
                    uncompressed_size,
                } = section
                    .compressed_data()
                    .context("Failed to get section data")?;
                if !matches!(format, CompressionFormat::None) {
                    return Err(anyhow::anyhow!(
                        "Section `.rodata` is compressed with {format:?}, but shouldn't be"
                    ));
                }
                if uncompressed_size != section.size() {
                    return Err(anyhow::anyhow!(
                        "Section `.rodata` has unexpected paddings: file size \
                        {uncompressed_size} != in-memory size {}",
                        section.size()
                    ));
                }
                maybe_rodata_section.replace(section);
            }
            ".text" => {
                let CompressedData {
                    format,
                    data: _,
                    uncompressed_size,
                } = section
                    .compressed_data()
                    .context("Failed to get section data")?;
                if !matches!(format, CompressionFormat::None) {
                    return Err(anyhow::anyhow!(
                        "Section `.text` is compressed with {format:?}, but shouldn't be"
                    ));
                }
                if uncompressed_size != section.size() {
                    return Err(anyhow::anyhow!(
                        "Section `.text` has unexpected paddings: file size \
                        {uncompressed_size} != in-memory size {}",
                        section.size()
                    ));
                }
                maybe_code_section.replace(section);
            }
            _ => {
                // Ignore everything else
            }
        }
    }

    let Some(metadata_section) = maybe_metadata_section else {
        return Err(anyhow::anyhow!("Section `ab-contract-metadata` not found"));
    };
    let Some(code_section) = maybe_code_section else {
        return Err(anyhow::anyhow!("Section `.text` not found"));
    };

    let metadata_section_address = metadata_section.address();
    let (metadata_section_offset, metadata_section_size) = metadata_section
        .file_range()
        .context("Failed to get `ab-contract-metadata` section range")?;
    let code_section_address = code_section.address();
    let (code_section_offset, code_section_size) = code_section
        .file_range()
        .context("Failed to get `.text` section range")?;

    let (rodata_section_address, (rodata_section_offset, rodata_section_size)) =
        match maybe_rodata_section {
            Some(rodata_section) => (
                rodata_section.address(),
                rodata_section
                    .file_range()
                    .context("Failed to get `.rodata` section range")?,
            ),
            None => (metadata_section_address, (metadata_section_offset, 0)),
        };

    // TODO: Hypothetically `.rodata` can be before metadata and have padding, update if/when that
    //  is the case
    // Can be reordered, but always next to each other
    if !(rodata_section_offset == metadata_section_offset + metadata_section_size
        || metadata_section_offset == rodata_section_offset + rodata_section_size)
    {
        return Err(anyhow::anyhow!(
            "Section `.rodata` and `ab-contract-metadata` are not next to each other: \
            rodata_section_offset={rodata_section_offset}, \
            rodata_section_size={rodata_section_size}, \
            metadata_section_offset={metadata_section_offset}, \
            metadata_section_size={metadata_section_size}"
        ));
    }

    let ro_data_offset = metadata_section_offset.min(rodata_section_offset);

    if ro_data_offset > code_section_offset {
        return Err(anyhow::anyhow!(
            "`.text` section must be after `.rodata` and `ab-contract-metadata` sections: \
            ro_data_offset={ro_data_offset}, \
            code_section_offset={code_section_offset}"
        ));
    }

    let ro_data_address = metadata_section_address.min(rodata_section_address);

    // Calculate in-memory read-only data size from addresses, such that after loading everything is
    // correct relatively to each other, even though some bytes may, technically, not belong to the
    // original read-only memory as such
    let Some(ro_data_memory_size) = code_section_address.checked_sub(ro_data_address) else {
        return Err(anyhow::anyhow!(
            "`.text` section must be after `.rodata` and `ab-contract-metadata` sections: \
            ro_data_address={ro_data_address}, \
            code_section_address={code_section_address}"
        ));
    };

    Ok(ParsedSections {
        metadata_offset: metadata_section_offset,
        metadata_size: metadata_section_size,
        ro_data_offset,
        ro_data_file_size: code_section_offset - ro_data_offset,
        ro_data_memory_size,
        code_offset: code_section_offset,
        code_size: code_section_size,
    })
}

fn check_imports(elf: &ElfFile<'_, FileHeader64<LittleEndian>>) -> anyhow::Result<()> {
    let imports = elf.imports().context("Failed to get imports")?;

    if imports.len() > 1 {
        return Err(anyhow::anyhow!(
            "Expected at most one import, got {}",
            imports.len()
        ));
    }

    if let Some(import) = imports.into_iter().next()
        && import.name() != HOST_CALL_FN_IMPORT.as_bytes()
    {
        return Err(anyhow::anyhow!(
            "Expected import `{HOST_CALL_FN_IMPORT}`, got `{}`",
            String::from_utf8_lossy(import.name())
        ));
    }

    Ok(())
}

#[derive(Debug, Copy, Clone)]
struct ParsedExport {
    offset: u64,
    size: u64,
}

fn parse_exports<'a>(
    elf: &'a ElfFile<'a, FileHeader64<LittleEndian>>,
) -> anyhow::Result<HashMap<&'a str, ParsedExport>> {
    elf.dynamic_symbols()
        .enumerate()
        .filter_map(|(index, symbol)| {
            // TODO: This log is not very usable right now:
            //  https://github.com/gimli-rs/object/issues/834
            // trace!(
            //     %index,
            //     ?symbol,
            //     "Processing symbol"
            // );

            let name = match symbol.name() {
                Ok(name) => name,
                Err(error) => return Some(Err(error).context("Failed to get symbol name")),
            };
            let elf_symbol = symbol.elf_symbol();

            if elf_symbol.st_bind() != STB_GLOBAL {
                return Some(Err(anyhow::anyhow!(
                    "Non-STB_GLOBAL symbol {name}: {symbol:?}"
                )));
            }
            if elf_symbol.st_other != STV_DEFAULT {
                return Some(Err(anyhow::anyhow!(
                    "Non-STV_DEFAULT symbol {name}: {symbol:?}"
                )));
            }
            if elf_symbol.st_shndx.get(LittleEndian) >= SHN_LORESERVE {
                return Some(Err(anyhow::anyhow!(
                    "Unexpected reserved section index for symbol {name}: {symbol:?}"
                )));
            }

            match symbol.kind() {
                SymbolKind::Unknown => {
                    if !(symbol.size() == 0 && name == HOST_CALL_FN_IMPORT) {
                        return Some(Err(anyhow::anyhow!(
                            "Unexpected unknown symbol {name}: {symbol:?}"
                        )));
                    }

                    None
                }
                SymbolKind::Text => {
                    let SymbolSection::Section(section_index) = symbol.section() else {
                        return Some(Err(anyhow::anyhow!(
                            "Unexpected section type for symbol {name}: {symbol:?}"
                        )));
                    };
                    let section = match elf.section_by_index(section_index) {
                        Ok(section) => section,
                        Err(error) => {
                            return Some(Err(error).context(format!(
                                "Failed to get section {section_index} for symbol {name}"
                            )));
                        }
                    };
                    let Some(offset_within_section) =
                        symbol.address().checked_sub(section.address())
                    else {
                        return Some(Err(anyhow::anyhow!(
                            "Invalid offset calculation for symbol {name}: \
                            address {} < section address {}",
                            symbol.address(),
                            section.address()
                        )));
                    };

                    let Some((section_offset, _section_size)) = section.file_range() else {
                        return Some(Err(anyhow::anyhow!(
                            "Failed to get file range for section {section_index} for symbol {name}"
                        )));
                    };
                    let offset = section_offset + offset_within_section;
                    let size = symbol.size();
                    debug!(
                        %index,
                        %name,
                        %offset,
                        %size,
                        "Found export function"
                    );

                    Some(Ok((name, ParsedExport { offset, size })))
                }
                SymbolKind::Data => {
                    if !name.starts_with(METADATA_STATIC_NAME_PREFIX) {
                        return Some(Err(anyhow::anyhow!(
                            "Unexpected STT_OBJECT {name}: {symbol:?}"
                        )));
                    }

                    None
                }
                _ => Some(Err(anyhow::anyhow!("Unexpected symbol {name}: {symbol:?}"))),
            }
        })
        .collect()
}

fn extract_host_call_fn_offset(
    input_file: &[u8],
    parsed_exports: &mut HashMap<&str, ParsedExport>,
) -> anyhow::Result<u64> {
    let Some(host_call_fn) = parsed_exports.remove(HOST_CALL_FN) else {
        return Ok(0);
    };

    if host_call_fn.size != size_of::<[u32; 2]>() as u64 {
        return Err(anyhow::anyhow!(
            "Host call function {HOST_CALL_FN} has invalid size {}",
            host_call_fn.size
        ));
    }
    let host_call_fn_offset = host_call_fn.offset;
    input_file
        .get(host_call_fn_offset as usize..)
        .with_context(|| {
            format!(
                "Host call address {host_call_fn_offset} out of range of input file ({} bytes)",
                input_file.len()
            )
        })?
        .get(..size_of::<[u32; 2]>())
        .context("Not enough bytes to get instructions of host call function")?;

    Ok(host_call_fn_offset)
}

fn parse_metadata_methods(
    parsed_exports: &mut HashMap<&str, ParsedExport>,
    metadata_bytes: &[u8],
) -> anyhow::Result<Vec<ParsedExport>> {
    let mut metadata_methods = Vec::new();

    let mut metadata_decoder = MetadataDecoder::new(metadata_bytes);

    while let Some(maybe_metadata_item) = metadata_decoder.decode_next() {
        let metadata_item = maybe_metadata_item.map_err(|error| {
            anyhow::Error::msg(error.to_string()).context("Failed to decode metadata item")
        })?;
        debug!(?metadata_item, "Decoded metadata item");

        let mut methods_metadata_decoder = metadata_item.into_decoder();
        while let Some(method_metadata_decoder) = methods_metadata_decoder.decode_next() {
            let (mut arguments_metadata_decoder, method_metadata_item) =
                method_metadata_decoder.decode_next().map_err(|error| {
                    anyhow::Error::msg(error.to_string())
                        .context("Failed to decode method metadata")
                })?;

            trace!(?method_metadata_item, "Decoded method metadata item");

            let method_name =
                str::from_utf8(method_metadata_item.method_name).with_context(|| {
                    format!(
                        "Non-UTF-8 method name: {:?}",
                        method_metadata_item.method_name
                    )
                })?;
            let symbol = parsed_exports
                .remove(method_name)
                .with_context(|| anyhow::anyhow!("Method {method_name} not found in symbols"))?;

            metadata_methods.push(symbol);

            while let Some(maybe_argument_metadata_item) = arguments_metadata_decoder.decode_next()
            {
                // Must be decoded to completion to preserve the correct decoding order
                let argument_metadata_item = maybe_argument_metadata_item.map_err(|error| {
                    anyhow::Error::msg(error.to_string())
                        .context("Failed to decode argument metadata item")
                })?;

                trace!(?argument_metadata_item, "Decoded argument metadata item");
            }
        }
    }

    Ok(metadata_methods)
}

/// Convert RISC-V ELF `cdylib` into Abundance contract file format
pub fn convert(input_file: &[u8]) -> anyhow::Result<Vec<u8>> {
    let buffer = SharedAlignedBuffer::from_bytes(input_file);
    let elf =
        ElfFile64::<LittleEndian>::parse(buffer.as_slice()).context("Failed to parse ELF file")?;

    if !is_correct_header(elf.elf_header()) {
        return Err(anyhow::anyhow!(
            "Invalid ELF header: {:?}",
            elf.elf_header()
        ));
    }

    check_relocations(&elf)?;
    let ParsedSections {
        metadata_offset,
        metadata_size,
        ro_data_offset,
        ro_data_file_size,
        ro_data_memory_size,
        code_offset,
        code_size,
    } = parse_sections(&elf)?;

    if metadata_size == 0 {
        return Err(anyhow::anyhow!("Metadata not found"));
    }

    check_imports(&elf)?;

    let mut parsed_exports = parse_exports(&elf)?;

    let host_call_fn_offset = extract_host_call_fn_offset(input_file, &mut parsed_exports)?;

    if host_call_fn_offset != 0 && host_call_fn_offset < code_offset {
        return Err(anyhow::anyhow!(
            "Host call function offset {host_call_fn_offset} is before `.text` section offset \
            {code_offset}"
        ));
    }

    let metadata_bytes = input_file
        .get(metadata_offset as usize..)
        .with_context(|| {
            format!(
                "Metadata offset {metadata_offset} out of range of input file ({} bytes)",
                input_file.len()
            )
        })?
        .get(..metadata_size as usize)
        .with_context(|| format!("Metadata size {metadata_size} is invalid"))?;

    let metadata_methods = parse_metadata_methods(&mut parsed_exports, metadata_bytes)?;

    if !parsed_exports.is_empty() {
        return Err(anyhow::anyhow!("Found unused exports: {parsed_exports:?}"));
    }

    let header_size = size_of::<ContractFileHeader>();
    let methods_metadata_size = size_of::<ContractFileMethodMetadata>() * metadata_methods.len();
    let header_with_methods_metadata_size = (header_size + methods_metadata_size) as u64;

    let mut output_file = Vec::new();

    // Write file header
    let contract_file_header = ContractFileHeader {
        magic: CONTRACT_FILE_MAGIC,
        read_only_section_file_size: ro_data_file_size
            .try_into()
            .context("Read-only section size is over 32-bit")?,
        read_only_section_memory_size: ro_data_memory_size
            .try_into()
            .context("Read-only section size is over 32-bit")?,
        metadata_offset: (metadata_offset - ro_data_offset + header_with_methods_metadata_size)
            .try_into()
            .context("Metadata offset is over 32-bit")?,
        metadata_size: metadata_size
            .try_into()
            .context("Metadata size is over 16-bit")?,
        num_methods: metadata_methods
            .len()
            .try_into()
            .context("Number of methods is over 16-bit")?,
        host_call_fn_offset: if host_call_fn_offset == 0 {
            0
        } else {
            (host_call_fn_offset - ro_data_offset + header_with_methods_metadata_size)
                .try_into()
                .context("Host call offset is over 32-bit")?
        },
    };
    output_file.extend_from_slice(contract_file_header.as_bytes());

    // Write metadata of each method
    for metadata_method in metadata_methods {
        let contract_file_function_metadata = ContractFileMethodMetadata {
            offset: (metadata_method.offset - ro_data_offset + header_with_methods_metadata_size)
                .try_into()
                .context("Method offset is over 32-bit")?,
            size: metadata_method
                .size
                .try_into()
                .context("Method size is over 32-bit")?,
        };
        output_file.extend_from_slice(contract_file_function_metadata.as_bytes());
    }

    // Write read-only data and code
    output_file.extend_from_slice(
        &input_file[ro_data_offset as usize..(code_offset + code_size) as usize],
    );

    // TODO: Compress with zstd? If so, then read-only data can be expanded to the real size from
    //  the very beginning, such that after decompression it'll already have correct layout.
    Ok(output_file)
}
