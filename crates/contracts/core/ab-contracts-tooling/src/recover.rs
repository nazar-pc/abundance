//! Recover RISC-V ELF `cdylib` from Abundance contract file format.
//!
//! Conversion of ELF into the contract file is lossy, so the recovered ELF file only contains what
//! is necessary for it to be a valid and functional shared library that converts back into exactly
//! the same contract file: read-only data with contract metadata, code, exported methods, host call
//! function and its import with corresponding PLT and GOT entries.
//!
//! The layout resembles what `lld` produces for contracts, except that read-only data starts at a
//! page boundary since its original alignment is not stored in the contract file.

use ab_contract_file::ContractFile;
use ab_contract_file::instruction::{ContractInstruction, ContractRegister};
use ab_contracts_common::{HOST_CALL_FN, HOST_CALL_FN_IMPORT};
use ab_riscv_primitives::prelude::*;
use anyhow::Context;
use object::elf::{
    DF_1_NOW, DF_BIND_NOW, DF_SYMBOLIC, DT_FLAGS, DT_FLAGS_1, DT_HASH, DT_JMPREL, DT_NULL,
    DT_PLTGOT, DT_PLTREL, DT_PLTRELSZ, DT_RELA, DT_STRSZ, DT_STRTAB, DT_SYMENT, DT_SYMTAB, Dyn64,
    EF_RISCV_RVC, ELFOSABI_GNU, EM_RISCV, ET_DYN, PF_R, PF_W, PF_X, PT_DYNAMIC, PT_GNU_STACK,
    PT_LOAD, PT_PHDR, PT_RISCV_ATTRIBUTES, ProgramFlags, ProgramHeader64, ProgramType,
    R_RISCV_JUMP_SLOT, Rela64, SHF_ALLOC, SHF_EXECINSTR, SHF_INFO_LINK, SHF_WRITE, SHN_UNDEF,
    SHT_PROGBITS, SHT_RELA, SHT_RISCV_ATTRIBUTES, STB_GLOBAL, STT_FUNC, STT_NOTYPE, STV_DEFAULT,
    Sym64, SymbolInfo, Tag_File,
};
use object::pod::bytes_of_slice;
use object::write::StringId;
use object::write::elf::{
    AttributesWriter, FileHeader, ProgramHeader, Rel, SectionHeader, SectionIndex, Sym,
    SymbolIndex, Writer,
};
use object::{Endianness, LittleEndian, U64};
use std::collections::HashSet;

/// Page size used for segment alignment
const PAGE_SIZE: u64 = 0x1000;
/// Alignment of 64-bit words, ELF structures and tables containing them
const WORD_ALIGNMENT: u64 = size_of::<u64>() as u64;
/// Alignment of instructions with compressed instructions support
const INSTRUCTION_ALIGNMENT: u64 = size_of::<u16>() as u64;
const PROGRAM_HEADER_SIZE: u64 = size_of::<ProgramHeader64<LittleEndian>>() as u64;
const SYMBOL_SIZE: u64 = size_of::<Sym64<LittleEndian>>() as u64;
const RELA_SIZE: u64 = size_of::<Rela64<LittleEndian>>() as u64;
const DYNAMIC_ENTRY_SIZE: u64 = size_of::<Dyn64<LittleEndian>>() as u64;
/// `PT_PHDR`, three `PT_LOAD`, `PT_DYNAMIC`, `PT_GNU_STACK` and `PT_RISCV_ATTRIBUTES`
const NUM_PROGRAM_HEADERS: usize = 7;
/// `DT_FLAGS`, `DT_FLAGS_1`, `DT_SYMTAB`, `DT_SYMENT`, `DT_STRTAB`, `DT_STRSZ`, `DT_HASH` and
/// `DT_NULL`
const NUM_DYNAMIC_ENTRIES: usize = 8;
/// `DT_JMPREL`, `DT_PLTRELSZ`, `DT_PLTGOT` and `DT_PLTREL`
const NUM_PLT_DYNAMIC_ENTRIES: usize = 4;
/// Size of the PLT header, which is followed by the only PLT entry for the host call function
/// import
const PLT_HEADER_SIZE: u64 = 32;
const PLT_ENTRY_SIZE: u64 = 16;
const PLT_SIZE: u64 = PLT_HEADER_SIZE + PLT_ENTRY_SIZE;
/// Two reserved entries for the dynamic linker followed by a single entry for the host call
/// function import
const GOT_PLT_ENTRIES: usize = 3;
const GOT_PLT_SIZE: u64 = (GOT_PLT_ENTRIES * size_of::<u64>()) as u64;
/// Offset of the host call function import entry relative to the beginning of `.got.plt`
const GOT_PLT_IMPORT_ENTRY_OFFSET: u64 = 2 * WORD_ALIGNMENT;
/// `Tag_RISCV_stack_align` attribute tag
const TAG_RISCV_STACK_ALIGN: u64 = 4;
/// `Tag_RISCV_arch` attribute tag
const TAG_RISCV_ARCH: u64 = 5;
/// `Tag_RISCV_stack_align` as required by the psABI
const RISCV_STACK_ALIGN: u64 = 16;

#[derive(Debug, Copy, Clone)]
struct ParsedHostCallFn {
    /// Address relative to the beginning of contract memory
    address: u64,
    size: u64,
    /// Address of the PLT entry that the host call function jumps to, relative to the beginning of
    /// contract memory
    plt_entry_address: u64,
}

#[derive(Debug)]
struct ParsedContract<'a> {
    metadata: &'a [u8],
    rodata: &'a [u8],
    /// Whether the metadata is located before `.rodata` in memory
    metadata_first: bool,
    /// Size of the read-only memory region, which includes zero padding before code
    read_only_memory_size: u64,
    code: &'a [u8],
    /// Dynamic symbols except the null symbol: the host call function import (if any), methods and
    /// the host call function (if any)
    symbols: Vec<DynamicSymbol<'a>>,
    host_call_fn: Option<ParsedHostCallFn>,
}

#[derive(Debug, Copy, Clone)]
struct DynamicSymbol<'a> {
    name: &'a str,
    /// Address relative to the beginning of contract memory and size of a function, `None` for an
    /// import
    function: Option<(u64, u64)>,
}

fn parse_host_call_fn(
    contract_file: &ContractFile<'_>,
    code_offset: usize,
    read_only_memory_size: u64,
) -> anyhow::Result<Option<ParsedHostCallFn>> {
    let host_call_fn_offset = contract_file.header().host_call_fn_offset;
    if host_call_fn_offset == 0 {
        return Ok(None);
    }

    let offset_in_code = usize::try_from(host_call_fn_offset)?
        .checked_sub(code_offset)
        .context("Host call function is before code section")?;
    let instruction_bytes = contract_file
        .get_code()
        .get(offset_in_code..)
        .context("Host call function is out of range of code section")?;

    // Compressed instruction might be the last one in the file, in which case the remaining bytes
    // are zero-padded for decoding
    let mut instruction_word = [0; size_of::<u32>()];
    instruction_word
        .iter_mut()
        .zip(instruction_bytes)
        .for_each(|(target, source)| *target = *source);
    let instruction =
        <ContractInstruction as Instruction>::try_decode(u32::from_le_bytes(instruction_word))
            .context("Failed to decode host call function instruction")?;

    #[expect(
        clippy::rest_pattern_accessible_field,
        reason = "Do not need other fields"
    )]
    let jump_offset = match instruction {
        ContractInstruction::Jal {
            rd: ContractRegister::Zero,
            imm,
            ..
        } => i64::from(imm),
        ContractInstruction::CJ { imm, .. } => i64::from(imm),
        _ => {
            return Err(anyhow::anyhow!(
                "Unexpected host call function instruction {instruction}"
            ));
        }
    };

    let address = read_only_memory_size + u64::try_from(offset_in_code)?;
    let plt_entry_address = address
        .checked_add_signed(jump_offset)
        .context("Host call function jumps outside of contract memory")?;

    Ok(Some(ParsedHostCallFn {
        address,
        size: u64::from(instruction.size()),
        plt_entry_address,
    }))
}

fn parse_contract(input_file: &[u8]) -> anyhow::Result<ParsedContract<'_>> {
    let mut contract_file_methods = Vec::new();
    let contract_file = ContractFile::parse(input_file, |contract_file_method| {
        contract_file_methods.push(contract_file_method);
        Ok(())
    })
    .context("Failed to parse contract file")?;
    let header = contract_file.header();

    let code_offset = input_file.len() - contract_file.get_code().len();
    let code = input_file
        .get(code_offset..)
        .context("Code section is out of range of the contract file")?;
    let read_only_size = usize::try_from(header.read_only_section_file_size)?;
    let read_only_offset = code_offset
        .checked_sub(read_only_size)
        .context("Read-only section is out of range of the contract file")?;
    let read_only = input_file
        .get(read_only_offset..code_offset)
        .context("Read-only section is out of range of the contract file")?;
    let read_only_memory_size = u64::from(header.read_only_section_memory_size);

    if header.metadata_size == 0 {
        return Err(anyhow::anyhow!("Metadata not found"));
    }
    let metadata_offset = usize::try_from(header.metadata_offset)?;
    let metadata = input_file
        .get(metadata_offset..)
        .and_then(|metadata| metadata.get(..usize::from(header.metadata_size)))
        .context("Metadata is out of range of the contract file")?;

    // Metadata and `.rodata` are stored next to each other, and which one goes first can only be
    // derived from the metadata offset
    let (metadata_first, rodata) = if metadata_offset == read_only_offset {
        let rodata = read_only
            .get(metadata.len()..)
            .context("Metadata is larger than read-only section")?;
        (true, rodata)
    } else if metadata_offset + metadata.len() == code_offset {
        let rodata = read_only
            .len()
            .checked_sub(metadata.len())
            .and_then(|rodata_size| read_only.get(..rodata_size))
            .context("Metadata is larger than read-only section")?;
        (false, rodata)
    } else {
        return Err(anyhow::anyhow!(
            "Metadata must be either at the beginning or at the end of read-only section: \
            metadata_offset={metadata_offset}, metadata_size={}, \
            read_only_offset={read_only_offset}, code_offset={code_offset}",
            metadata.len()
        ));
    };

    let host_call_fn = parse_host_call_fn(&contract_file, code_offset, read_only_memory_size)?;

    // Names of exported methods must not clash with each other or with host call function and its
    // import, or else the recovered file will not convert back
    let mut symbol_names = HashSet::from([HOST_CALL_FN, HOST_CALL_FN_IMPORT]);
    let methods = contract_file_methods
        .into_iter()
        .map(|contract_file_method| {
            let method_name = contract_file_method.method_metadata_item.method_name;
            let name = str::from_utf8(method_name)
                .with_context(|| format!("Non-UTF-8 method name: {method_name:?}"))?;
            if name.contains('\0') {
                return Err(anyhow::anyhow!("Method name {name:?} contains zero byte"));
            }
            if !symbol_names.insert(name) {
                return Err(anyhow::anyhow!("Duplicate or reserved method name {name}"));
            }

            Ok(DynamicSymbol {
                name,
                function: Some((
                    u64::from(contract_file_method.address),
                    u64::from(contract_file_method.size),
                )),
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let symbols = host_call_fn
        .map(|_host_call_fn| DynamicSymbol {
            name: HOST_CALL_FN_IMPORT,
            function: None,
        })
        .into_iter()
        .chain(methods)
        .chain(host_call_fn.map(|host_call_fn| DynamicSymbol {
            name: HOST_CALL_FN,
            function: Some((host_call_fn.address, host_call_fn.size)),
        }))
        .collect();

    Ok(ParsedContract {
        metadata,
        rodata,
        metadata_first,
        read_only_memory_size,
        code,
        symbols,
        host_call_fn,
    })
}

/// Location of a section or segment in the file and in memory
#[derive(Debug, Default, Copy, Clone)]
struct Placement {
    offset: u64,
    address: u64,
    size: u64,
}

impl Placement {
    /// Placement in the first segment, where file offsets are equal to addresses
    fn identity(offset: u64, size: u64) -> Self {
        Self {
            offset,
            address: offset,
            size,
        }
    }

    fn end_offset(&self) -> u64 {
        self.offset + self.size
    }

    fn end_address(&self) -> u64 {
        self.address + self.size
    }

    /// Placement of the next section in the same segment at a file offset reserved after this one
    fn next_at(&self, offset: u64, size: u64) -> Self {
        Self {
            offset,
            address: self.end_address() + (offset - self.end_offset()),
            size,
        }
    }

    /// Reserve the file range at the exact offset
    fn reserve(&self, writer: &mut Writer<'_>) {
        writer.reserve_until(self.offset);
        writer.reserve(self.size, 1);
    }

    fn section_header(&self) -> SectionHeader {
        SectionHeader {
            sh_addr: self.address,
            sh_offset: self.offset,
            sh_size: self.size,
            ..SectionHeader::default()
        }
    }

    fn program_header(
        &self,
        p_type: ProgramType,
        p_flags: ProgramFlags,
        p_align: u64,
    ) -> ProgramHeader {
        ProgramHeader {
            p_type,
            p_flags,
            p_offset: self.offset,
            p_vaddr: self.address,
            p_paddr: self.address,
            p_filesz: self.size,
            p_memsz: self.size,
            p_align,
        }
    }
}

/// Section other than those whose headers [`Writer`] writes on its own
#[derive(Debug, Copy, Clone)]
struct Section {
    name: StringId,
    index: SectionIndex,
}

impl Section {
    fn reserve(writer: &mut Writer<'_>, name: &'static [u8]) -> Self {
        Self {
            name: writer.add_section_name(name),
            index: writer.reserve_section_index(),
        }
    }

    fn write_header(self, writer: &mut Writer<'_>, header: SectionHeader) {
        writer.write_section_header(&SectionHeader {
            sh_name: writer.section_name_offset(Some(self.name)),
            ..header
        });
    }
}

/// Parts of the ELF file that only exist when there is a host call function
#[derive(Debug, Copy, Clone)]
struct HostCallParts {
    import_symbol_index: SymbolIndex,
    rela_plt_section: Section,
    rela_plt: Placement,
    plt_section: Section,
    plt: Placement,
    got_plt_section: Section,
    got_plt: Placement,
}

/// Split PC-relative offset into `hi20` and `lo12` parts of `auipc` and I-type instruction pair,
/// already shifted into their positions in the instruction encoding
fn pc_relative_hi_lo(offset: u64) -> (u32, u32) {
    let hi = u32::try_from((offset.wrapping_add(0x800) >> 12) & 0xf_ffff)
        .expect("Masked to 20 bits; qed");
    let lo = u32::try_from(offset & 0xfff).expect("Masked to 12 bits; qed");
    (hi << 12, lo << 20)
}

/// PLT header followed by a single entry, the same way as `lld` generates them
fn plt_code(plt_address: u64, got_plt_address: u64) -> [u8; PLT_SIZE as usize] {
    let (header_hi, header_lo) = pc_relative_hi_lo(got_plt_address - plt_address);
    let (entry_hi, entry_lo) = pc_relative_hi_lo(
        (got_plt_address + GOT_PLT_IMPORT_ENTRY_OFFSET) - (plt_address + PLT_HEADER_SIZE),
    );

    let instructions: [u32; PLT_SIZE as usize / size_of::<u32>()] = [
        // auipc t2, %pcrel_hi(.got.plt)
        0x0000_0397 | header_hi,
        // sub t1, t1, t3
        0x41c3_0333,
        // ld t3, %pcrel_lo(.got.plt)(t2)
        0x0003_be03 | header_lo,
        // addi t1, t1, -(PLT header size + 12)
        0xfd43_0313,
        // addi t0, t2, %pcrel_lo(.got.plt)
        0x0003_8293 | header_lo,
        // srli t1, t1, 1
        0x0013_5313,
        // ld t0, 8(t0)
        0x0082_b283,
        // jr t3
        0x000e_0067,
        // auipc t3, %pcrel_hi(import entry in .got.plt)
        0x0000_0e17 | entry_hi,
        // ld t3, %pcrel_lo(import entry in .got.plt)(t3)
        0x000e_3e03 | entry_lo,
        // jalr t1, t3
        0x000e_0367,
        // nop
        0x0000_0013,
    ];

    let mut plt = [0; _];
    plt.as_chunks_mut::<{ size_of::<u32>() }>()
        .0
        .iter_mut()
        .zip(instructions)
        .for_each(|(bytes, instruction)| *bytes = instruction.to_le_bytes());
    plt
}

fn riscv_attributes() -> Vec<u8> {
    let mut attributes = AttributesWriter::new(Endianness::Little);
    attributes.start_subsection(b"riscv");
    attributes.start_subsubsection(Tag_File);
    attributes.write_attribute_tag(TAG_RISCV_STACK_ALIGN);
    attributes.write_attribute_integer(RISCV_STACK_ALIGN);
    attributes.write_attribute_tag(TAG_RISCV_ARCH);
    // ISA string of contract instructions allows disassemblers and debuggers to decode all
    // instructions correctly
    attributes.write_attribute_string(<ContractInstruction as Instruction>::ISA_STRING.as_bytes());
    attributes.end_subsubsection();
    attributes.end_subsection();
    attributes.data()
}

/// Recover RISC-V ELF `cdylib` from Abundance contract file format.
///
/// The output converts back into exactly the same contract file with [`convert()`].
///
/// [`convert()`]: crate::convert::convert
pub fn recover(input_file: &[u8]) -> anyhow::Result<Vec<u8>> {
    let ParsedContract {
        metadata,
        rodata,
        metadata_first,
        read_only_memory_size,
        code,
        symbols,
        host_call_fn,
    } = parse_contract(input_file)?;

    let riscv_attributes = riscv_attributes();
    // Metadata and `.rodata` in memory order, `.rodata` section is omitted when empty
    let read_only_parts: [(&'static [u8], &[u8]); 2] = if metadata_first {
        [(b"ab-contract-metadata", metadata), (b".rodata", rodata)]
    } else {
        [(b".rodata", rodata), (b"ab-contract-metadata", metadata)]
    };

    let mut output_file = Vec::new();
    let mut writer = Writer::new(Endianness::Little, true, &mut output_file);

    // Section indices are reserved in the same order as section headers are written
    writer.reserve_dynsym_section_index();
    writer.reserve_hash_section_index();
    writer.reserve_dynstr_section_index();
    let rela_plt_section = host_call_fn.map(|_| Section::reserve(&mut writer, b".rela.plt"));
    let read_only_sections = read_only_parts
        .into_iter()
        .filter(|(_name, data)| !data.is_empty())
        .map(|(name, data)| (Section::reserve(&mut writer, name), data))
        .collect::<Vec<_>>();
    let text_section = Section::reserve(&mut writer, b".text");
    let plt_section = host_call_fn.map(|_| Section::reserve(&mut writer, b".plt"));
    writer.reserve_dynamic_section_index();
    let got_plt_section = host_call_fn.map(|_| Section::reserve(&mut writer, b".got.plt"));
    let riscv_attributes_section = Section::reserve(&mut writer, b".riscv.attributes");
    writer.reserve_shstrtab_section_index();

    writer.reserve_null_dynamic_symbol_index();
    let symbol_ids = symbols
        .iter()
        .map(|symbol| {
            (
                writer.reserve_dynamic_symbol_index(),
                writer.add_dynamic_string(symbol.name.as_bytes()),
            )
        })
        .collect::<Vec<_>>();
    let import_symbol_index = symbols
        .iter()
        .zip(&symbol_ids)
        .find_map(|(symbol, &(index, _name))| symbol.function.is_none().then_some(index));
    let num_symbols = writer.dynamic_symbol_count();

    // The first `PT_LOAD` segment contains headers, dynamic linking information and read-only data,
    // with file offsets equal to addresses
    writer.reserve_file_header();
    writer.reserve_program_headers(u32::try_from(NUM_PROGRAM_HEADERS)?);
    let dynsym_address = writer.reserve_dynsym();
    let hash_address = writer.reserve_hash(num_symbols, num_symbols);
    let dynstr = Placement::identity(writer.reserve_dynstr()?, u64::from(writer.dynstr_len()));
    let rela_plt =
        host_call_fn.map(|_| Placement::identity(writer.reserve_relocations(1, true), RELA_SIZE));
    // Read-only data starts at a page boundary
    let read_only_start = writer.reserve(0, PAGE_SIZE);
    let read_only_sections = read_only_sections
        .into_iter()
        .map(|(section, data)| {
            let size = u64::try_from(data.len())?;
            let placement = Placement::identity(writer.reserve(size, 1), size);
            anyhow::Ok((section, placement, data))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let read_only = Placement::identity(read_only_start, writer.reserved_len() - read_only_start);
    let read_only_padding = read_only_memory_size
        .checked_sub(read_only.size)
        .context("Read-only section file size is larger than memory size")?;

    // The second `PT_LOAD` segment contains code and PLT. Code address is fixed relative to
    // read-only data, while file offset only needs to be congruent with it modulo page size.
    let text = Placement {
        offset: read_only.end_offset() + read_only_padding % PAGE_SIZE,
        address: read_only.address + read_only_memory_size,
        size: u64::try_from(code.len())?,
    };
    if !text.address.is_multiple_of(INSTRUCTION_ALIGNMENT) {
        return Err(anyhow::anyhow!(
            "Code section is not aligned to instruction boundary: read-only memory size \
            {read_only_memory_size}"
        ));
    }
    text.reserve(&mut writer);
    // PLT location is defined by the host call function that jumps to its only entry
    let plt = host_call_fn
        .map(|host_call_fn| {
            let address = (read_only.address + host_call_fn.plt_entry_address)
                .checked_sub(PLT_HEADER_SIZE)
                .filter(|&address| address >= text.end_address())
                .with_context(|| {
                    format!(
                        "Host call function must jump to PLT entry after code section, but jumps \
                        to {} relative to the beginning of contract memory",
                        host_call_fn.plt_entry_address
                    )
                })?;

            let plt = Placement {
                offset: text.offset + (address - text.address),
                address,
                size: PLT_SIZE,
            };
            plt.reserve(&mut writer);
            anyhow::Ok(plt)
        })
        .transpose()?;
    let executable_end = plt.unwrap_or(text);

    // The third `PT_LOAD` segment contains dynamic section and GOT, it starts at a new page in
    // memory
    let num_dynamic_entries = if host_call_fn.is_some() {
        NUM_DYNAMIC_ENTRIES + NUM_PLT_DYNAMIC_ENTRIES
    } else {
        NUM_DYNAMIC_ENTRIES
    };
    let dynamic = {
        let offset = writer.reserve_dynamic(num_dynamic_entries);
        Placement {
            offset,
            address: executable_end.end_address().next_multiple_of(PAGE_SIZE) + offset % PAGE_SIZE,
            size: u64::try_from(num_dynamic_entries)? * DYNAMIC_ENTRY_SIZE,
        }
    };
    let got_plt = host_call_fn
        .map(|_| dynamic.next_at(writer.reserve(GOT_PLT_SIZE, WORD_ALIGNMENT), GOT_PLT_SIZE));
    let writable_end = got_plt.unwrap_or(dynamic);

    // Non-allocated sections
    let riscv_attributes_placement = {
        let size = u64::try_from(riscv_attributes.len())?;
        Placement {
            offset: writer.reserve(size, 1),
            address: 0,
            size,
        }
    };
    writer.reserve_shstrtab()?;
    writer.reserve_section_headers();

    let host_call = try {
        HostCallParts {
            import_symbol_index: import_symbol_index?,
            rela_plt_section: rela_plt_section?,
            rela_plt: rela_plt?,
            plt_section: plt_section?,
            plt: plt?,
            got_plt_section: got_plt_section?,
            got_plt: got_plt?,
        }
    };

    let mut dynamic_entries = vec![
        (DT_FLAGS, (DF_SYMBOLIC | DF_BIND_NOW).0),
        (DT_FLAGS_1, DF_1_NOW.0),
    ];
    if let Some(host_call) = host_call {
        dynamic_entries.extend([
            (DT_JMPREL, host_call.rela_plt.address),
            (DT_PLTRELSZ, host_call.rela_plt.size),
            (DT_PLTGOT, host_call.got_plt.address),
            (DT_PLTREL, u64::try_from(DT_RELA.0)?),
        ]);
    }
    dynamic_entries.extend([
        (DT_SYMTAB, dynsym_address),
        (DT_SYMENT, SYMBOL_SIZE),
        (DT_STRTAB, dynstr.address),
        (DT_STRSZ, dynstr.size),
        (DT_HASH, hash_address),
        (DT_NULL, 0),
    ]);
    if dynamic_entries.len() != num_dynamic_entries {
        return Err(anyhow::anyhow!(
            "Number of dynamic entries mismatch: {} != {num_dynamic_entries}",
            dynamic_entries.len()
        ));
    }

    writer.write_file_header(&FileHeader {
        os_abi: ELFOSABI_GNU,
        abi_version: 0,
        e_type: ET_DYN,
        e_machine: EM_RISCV,
        e_entry: 0,
        e_flags: EF_RISCV_RVC,
    })?;

    let program_headers = Placement::identity(
        writer.write_align_program_headers(),
        u64::try_from(NUM_PROGRAM_HEADERS)? * PROGRAM_HEADER_SIZE,
    );
    let program_headers: [_; NUM_PROGRAM_HEADERS] = [
        program_headers.program_header(PT_PHDR, PF_R, WORD_ALIGNMENT),
        Placement::identity(0, read_only.end_offset()).program_header(PT_LOAD, PF_R, PAGE_SIZE),
        Placement {
            size: executable_end.end_address() - text.address,
            ..text
        }
        .program_header(PT_LOAD, PF_R | PF_X, PAGE_SIZE),
        Placement {
            size: writable_end.end_address() - dynamic.address,
            ..dynamic
        }
        .program_header(PT_LOAD, PF_R | PF_W, PAGE_SIZE),
        dynamic.program_header(PT_DYNAMIC, PF_R | PF_W, WORD_ALIGNMENT),
        Placement::default().program_header(PT_GNU_STACK, PF_R | PF_W, 0),
        riscv_attributes_placement.program_header(PT_RISCV_ATTRIBUTES, PF_R, 1),
    ];
    for program_header in &program_headers {
        writer.write_program_header(program_header);
    }

    writer.write_null_dynamic_symbol();
    for (symbol, (_index, name)) in symbols.iter().zip(symbol_ids) {
        let (section, st_type, st_value, st_size) = match symbol.function {
            Some((address, size)) => (
                Some(text_section.index.0),
                STT_FUNC,
                read_only.address + address,
                size,
            ),
            None => (None, STT_NOTYPE, 0, 0),
        };
        writer.write_dynamic_symbol(&Sym {
            section,
            st_name: writer.dynamic_string_offset(Some(name)),
            st_info: SymbolInfo::new(STB_GLOBAL, st_type),
            st_other: STV_DEFAULT.into(),
            // Only used for symbols without a section
            st_shndx: SHN_UNDEF,
            st_value,
            st_size,
        });
    }
    // SysV hash table with as many buckets as there are symbols, the same way as `lld` builds it
    writer.write_hash(num_symbols, num_symbols, |symbol_index| {
        let symbol = symbols.get(usize::try_from(symbol_index.checked_sub(1)?).ok()?)?;
        Some(object::elf::hash(symbol.name.as_bytes()))
    });
    writer.write_dynstr();

    if let Some(host_call) = host_call {
        writer.write_align_relocation();
        writer.write_relocation(
            true,
            &Rel {
                r_offset: host_call.got_plt.address + GOT_PLT_IMPORT_ENTRY_OFFSET,
                r_sym: host_call.import_symbol_index.0,
                r_type: R_RISCV_JUMP_SLOT,
                r_addend: 0,
            },
        );
    }
    for (_section, placement, data) in &read_only_sections {
        writer.pad_until(placement.offset);
        writer.write(data);
    }
    writer.pad_until(text.offset);
    writer.write(code);
    if let Some(host_call) = host_call {
        writer.pad_until(host_call.plt.offset);
        writer.write(&plt_code(host_call.plt.address, host_call.got_plt.address));
    }

    writer.write_align_dynamic();
    for (d_tag, d_val) in dynamic_entries {
        writer.write_dynamic(d_tag, d_val)?;
    }
    if let Some(host_call) = host_call {
        // Reserved entries are filled by the dynamic linker, while the import entry initially
        // points to the PLT header for lazy binding
        let got_plt_entries: [_; GOT_PLT_ENTRIES] =
            [0, 0, host_call.plt.address].map(|entry| U64::new(LittleEndian, entry));
        writer.pad_until(host_call.got_plt.offset);
        writer.write(bytes_of_slice(&got_plt_entries));
    }
    writer.pad_until(riscv_attributes_placement.offset);
    writer.write(&riscv_attributes);
    writer.write_shstrtab();

    writer.write_null_section_header();
    // Only the null symbol is local
    writer.write_dynsym_section_header(dynsym_address, 1);
    writer.write_hash_section_header(hash_address);
    writer.write_dynstr_section_header(dynstr.address);
    if let Some(host_call) = host_call {
        let dynsym_index = writer.dynsym_index();
        host_call.rela_plt_section.write_header(
            &mut writer,
            SectionHeader {
                sh_type: SHT_RELA,
                sh_flags: SHF_ALLOC | SHF_INFO_LINK,
                sh_link: dynsym_index.0,
                sh_info: host_call.got_plt_section.index.0,
                sh_addralign: WORD_ALIGNMENT,
                sh_entsize: RELA_SIZE,
                ..host_call.rela_plt.section_header()
            },
        );
    }
    for (section, placement, _data) in &read_only_sections {
        section.write_header(
            &mut writer,
            SectionHeader {
                sh_type: SHT_PROGBITS,
                sh_flags: SHF_ALLOC,
                sh_addralign: 1,
                ..placement.section_header()
            },
        );
    }
    text_section.write_header(
        &mut writer,
        SectionHeader {
            sh_type: SHT_PROGBITS,
            sh_flags: SHF_ALLOC | SHF_EXECINSTR,
            sh_addralign: INSTRUCTION_ALIGNMENT,
            ..text.section_header()
        },
    );
    if let Some(host_call) = host_call {
        host_call.plt_section.write_header(
            &mut writer,
            SectionHeader {
                sh_type: SHT_PROGBITS,
                sh_flags: SHF_ALLOC | SHF_EXECINSTR,
                sh_addralign: INSTRUCTION_ALIGNMENT,
                ..host_call.plt.section_header()
            },
        );
    }
    writer.write_dynamic_section_header(dynamic.address);
    if let Some(host_call) = host_call {
        host_call.got_plt_section.write_header(
            &mut writer,
            SectionHeader {
                sh_type: SHT_PROGBITS,
                sh_flags: SHF_ALLOC | SHF_WRITE,
                sh_addralign: WORD_ALIGNMENT,
                ..host_call.got_plt.section_header()
            },
        );
    }
    riscv_attributes_section.write_header(
        &mut writer,
        SectionHeader {
            sh_type: SHT_RISCV_ATTRIBUTES,
            sh_addralign: 1,
            ..riscv_attributes_placement.section_header()
        },
    );
    writer.write_shstrtab_section_header();

    Ok(output_file)
}
