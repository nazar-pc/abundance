use ab_contract_file::instruction::{ContractInstruction, ContractRegister};
use ab_contract_file::{
    CONTRACT_FILE_MAGIC, ContractFile, ContractFileHeader, ContractFileMethodMetadata,
};
use ab_contracts_common::metadata::ContractMetadataKind;
use ab_contracts_common::{HOST_CALL_FN, HOST_CALL_FN_IMPORT};
use ab_contracts_tooling::build::{BuildOptions, build_cdylib};
use ab_contracts_tooling::convert::convert;
use ab_contracts_tooling::recover::recover;
use ab_contracts_tooling::target_specification::TargetSpecification;
use ab_io_type::trivial_type::TrivialType;
use ab_riscv_primitives::prelude::*;
use object::elf::{
    DF_1_NOW, DF_BIND_NOW, DF_SYMBOLIC, DT_FLAGS, DT_FLAGS_1, DT_HASH, DT_JMPREL, DT_NULL,
    DT_PLTGOT, DT_PLTREL, DT_PLTRELSZ, DT_RELA, DT_STRSZ, DT_STRTAB, DT_SYMENT, DT_SYMTAB, PF_R,
    PF_W, PF_X, PT_DYNAMIC, PT_LOAD, R_RISCV_JUMP_SLOT, SHN_UNDEF,
};
use object::read::elf::{ElfFile64, ProgramHeader, SectionHeader};
use object::{
    LittleEndian, Object, ObjectSection, ObjectSymbol, ObjectSymbolTable, RelocationFlags,
    RelocationTarget, SymbolIndex, SymbolKind,
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

const PAGE_SIZE: u64 = 0x1000;
const PLT_HEADER_SIZE: u64 = 32;
/// `c.li a0, 0`
const LI_A0_0: [u8; 2] = 0x4501_u16.to_le_bytes();
/// `c.jr ra`
const RET: [u8; 2] = 0x8082_u16.to_le_bytes();

/// `c.j offset`
fn c_j(offset: i16) -> [u8; 2] {
    let imm = offset.cast_unsigned();
    let bit = |index: u16| (imm >> index) & 1;
    let instruction = (0b101 << 13)
        | (bit(11) << 12)
        | (bit(4) << 11)
        | (bit(9) << 10)
        | (bit(8) << 9)
        | (bit(10) << 8)
        | (bit(6) << 7)
        | (bit(7) << 6)
        | (bit(3) << 5)
        | (bit(2) << 4)
        | (bit(1) << 3)
        | (bit(5) << 2)
        | 0b01;
    instruction.to_le_bytes()
}

/// `jal zero, offset`
fn jal_zero(offset: i32) -> [u8; 4] {
    let imm = offset.cast_unsigned();
    let instruction = (((imm >> 20) & 1) << 31)
        | (((imm >> 1) & 0x3ff) << 21)
        | (((imm >> 11) & 1) << 20)
        | (((imm >> 12) & 0xff) << 12)
        | 0b110_1111;
    instruction.to_le_bytes()
}

/// Trait metadata with stateless view methods without arguments
fn trait_metadata(trait_name: &str, method_names: &[&str]) -> Vec<u8> {
    let mut metadata = vec![
        ContractMetadataKind::Trait as u8,
        u8::try_from(trait_name.len()).unwrap(),
    ];
    metadata.extend_from_slice(trait_name.as_bytes());
    metadata.push(u8::try_from(method_names.len()).unwrap());
    for method_name in method_names {
        metadata.push(ContractMetadataKind::ViewStateless as u8);
        metadata.push(u8::try_from(method_name.len()).unwrap());
        metadata.extend_from_slice(method_name.as_bytes());
        // Number of arguments
        metadata.push(0);
    }
    metadata
}

/// Synthetic contract file with hand-assembled code
#[derive(Debug, Default)]
struct TestContract {
    rodata_before_metadata: Vec<u8>,
    rodata_after_metadata: Vec<u8>,
    /// Actual padding might be one byte larger, such that code starts at an even address
    min_read_only_padding: u32,
    /// Overrides read-only section memory size derived from `min_read_only_padding`
    read_only_section_memory_size: Option<u32>,
    code: Vec<u8>,
    /// Method name, offset within code and size
    methods: Vec<(&'static str, u32, u32)>,
    /// Offset within code
    host_call_fn_offset: Option<u32>,
}

impl TestContract {
    fn build(&self) -> Vec<u8> {
        let method_names = self
            .methods
            .iter()
            .map(|&(name, _, _)| name)
            .collect::<Vec<_>>();
        let metadata = trait_metadata("Test", &method_names);
        let read_only = [
            self.rodata_before_metadata.as_slice(),
            &metadata,
            &self.rodata_after_metadata,
        ]
        .concat();

        let num_methods = u16::try_from(self.methods.len()).unwrap();
        let read_only_offset =
            ContractFileHeader::SIZE + u32::from(num_methods) * ContractFileMethodMetadata::SIZE;
        let read_only_section_file_size = u32::try_from(read_only.len()).unwrap();
        let read_only_section_memory_size =
            self.read_only_section_memory_size.unwrap_or_else(|| {
                (read_only_section_file_size + self.min_read_only_padding)
                    .next_multiple_of(size_of::<u16>() as u32)
            });
        let code_offset = read_only_offset + read_only_section_file_size;

        let header = ContractFileHeader {
            magic: CONTRACT_FILE_MAGIC,
            read_only_section_file_size,
            read_only_section_memory_size,
            metadata_offset: read_only_offset
                + u32::try_from(self.rodata_before_metadata.len()).unwrap(),
            metadata_size: u16::try_from(metadata.len()).unwrap(),
            num_methods,
            host_call_fn_offset: self
                .host_call_fn_offset
                .map_or(0, |offset| code_offset + offset),
        };

        let mut file = header.as_bytes().to_vec();
        for &(_, offset, size) in &self.methods {
            let method_metadata = ContractFileMethodMetadata {
                offset: code_offset + offset,
                size,
            };
            file.extend_from_slice(method_metadata.as_bytes());
        }
        file.extend_from_slice(&read_only);
        file.extend_from_slice(&self.code);
        file
    }
}

/// Recover ELF file, check that it converts back into the same contract file and that the result
/// is stable
fn recover_and_check(contract_file: &[u8]) -> Vec<u8> {
    let elf = recover(contract_file).unwrap();
    check_recovered_elf(contract_file, &elf);

    let converted_contract_file = convert(&elf).unwrap();
    assert_eq!(converted_contract_file, contract_file);
    assert_eq!(recover(&converted_contract_file).unwrap(), elf);

    elf
}

fn decode(bytes: &[u8]) -> ContractInstruction {
    let mut instruction = [0; 4];
    instruction
        .iter_mut()
        .zip(bytes)
        .for_each(|(target, source)| *target = *source);
    ContractInstruction::try_decode(u32::from_le_bytes(instruction)).unwrap()
}

/// Find dynamic symbol index using `.hash` section the same way as the dynamic linker does it
fn find_symbol_using_hash_table(elf: &ElfFile64<'_, LittleEndian>, name: &str) -> SymbolIndex {
    let hash_table = elf
        .section_by_name(".hash")
        .unwrap()
        .data()
        .unwrap()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|&word| u32::from_le_bytes(word) as usize)
        .collect::<Vec<_>>();
    let [num_buckets, num_chains, ..] = hash_table[..] else {
        panic!("Hash table is too small");
    };
    let buckets = &hash_table[2..][..num_buckets];
    let chains = &hash_table[2 + num_buckets..][..num_chains];
    let symbols = elf.dynamic_symbol_table().unwrap();

    let mut index = buckets[object::elf::hash(name.as_bytes()) as usize % num_buckets];
    while index != 0 {
        if symbols.symbol_by_index(SymbolIndex(index)).unwrap().name() == Ok(name) {
            return SymbolIndex(index);
        }
        index = chains[index];
    }
    panic!("Symbol {name} not found using hash table");
}

/// Check recovered ELF against the contract file it was recovered from
fn check_recovered_elf(contract_file_bytes: &[u8], elf_bytes: &[u8]) {
    let mut methods = Vec::new();
    let contract_file = ContractFile::parse(contract_file_bytes, |method| {
        methods.push((
            str::from_utf8(method.method_metadata_item.method_name).unwrap(),
            u64::from(method.address),
            u64::from(method.size),
        ));
        Ok(())
    })
    .unwrap();
    assert_eq!(
        contract_file
            .iterate_methods()
            .map(|method| (u64::from(method.address), u64::from(method.size)))
            .collect::<Vec<_>>(),
        methods
            .iter()
            .map(|&(_, address, size)| (address, size))
            .collect::<Vec<_>>()
    );
    let header = contract_file.header();
    let code = contract_file.get_code();
    let code_offset = contract_file_bytes.len() - code.len();
    let read_only = &contract_file_bytes
        [code_offset - header.read_only_section_file_size as usize..code_offset];

    let elf = ElfFile64::<LittleEndian>::parse(elf_bytes).unwrap();
    let endian = LittleEndian;

    // Segments must be valid for the dynamic loader
    let load_segments = elf
        .elf_program_headers()
        .iter()
        .filter(|segment| segment.p_type(endian) == PT_LOAD)
        .collect::<Vec<_>>();
    assert_eq!(load_segments.len(), 3);
    let mut previous_segment_end = 0;
    for segment in &load_segments {
        assert_eq!(segment.p_align(endian), PAGE_SIZE);
        assert_eq!(
            segment.p_offset(endian) % PAGE_SIZE,
            segment.p_vaddr(endian) % PAGE_SIZE
        );
        assert_eq!(segment.p_filesz(endian), segment.p_memsz(endian));
        assert!(segment.p_vaddr(endian) >= previous_segment_end);
        assert!(segment.p_offset(endian) + segment.p_filesz(endian) <= elf_bytes.len() as u64);
        previous_segment_end = segment.p_vaddr(endian) + segment.p_memsz(endian);
    }
    // Memory as mapped by the dynamic loader
    let read_memory = |address: u64, size: usize| {
        let segment = load_segments
            .iter()
            .find(|segment| {
                segment.p_vaddr(endian) <= address
                    && address + size as u64 <= segment.p_vaddr(endian) + segment.p_memsz(endian)
            })
            .unwrap_or_else(|| panic!("{size} bytes at {address:#x} are not mapped"));
        &elf_bytes[(segment.p_offset(endian) + address - segment.p_vaddr(endian)) as usize..]
            [..size]
    };

    // Read-only data starts at a page boundary and code follows it at the same offset as in
    // contract memory
    let section_address = |name| {
        elf.section_by_name(name)
            .unwrap_or_else(|| panic!("Section {name} not found"))
            .address()
    };
    let read_only_address =
        elf.section_by_name(".rodata")
            .map_or(section_address("ab-contract-metadata"), |rodata| {
                rodata
                    .address()
                    .min(section_address("ab-contract-metadata"))
            });
    assert!(read_only_address.is_multiple_of(PAGE_SIZE));
    assert_eq!(read_memory(read_only_address, read_only.len()), read_only);
    assert_eq!(
        elf.section_by_name("ab-contract-metadata")
            .unwrap()
            .data()
            .unwrap(),
        contract_file.metadata_bytes()
    );
    let code_address = read_only_address + u64::from(header.read_only_section_memory_size);
    assert_eq!(section_address(".text"), code_address);
    assert_eq!(read_memory(code_address, code.len()), code);

    // Memory permissions as set by the dynamic loader
    let segment_flags = |address: u64| {
        load_segments
            .iter()
            .find(|segment| {
                segment.p_vaddr(endian) <= address
                    && address < segment.p_vaddr(endian) + segment.p_memsz(endian)
            })
            .unwrap_or_else(|| panic!("Address {address:#x} is not mapped"))
            .p_flags(endian)
    };
    assert_eq!(segment_flags(read_only_address), PF_R);
    assert_eq!(segment_flags(code_address), PF_R | PF_X);
    assert_eq!(segment_flags(section_address(".dynamic")), PF_R | PF_W);

    // Exported methods
    let dynamic_symbols = elf.dynamic_symbol_table().unwrap();
    let mut expected_num_symbols = methods.len();
    for &(name, address, size) in &methods {
        let symbol = dynamic_symbols
            .symbol_by_index(find_symbol_using_hash_table(&elf, name))
            .unwrap();
        assert_eq!(symbol.kind(), SymbolKind::Text);
        assert!(symbol.is_global());
        assert_eq!(symbol.address(), read_only_address + address);
        assert_eq!(symbol.size(), size);
    }

    // Dynamic section
    let dynamic = elf.section_by_name(".dynamic").unwrap();
    let dynamic_segment = elf
        .elf_program_headers()
        .iter()
        .find(|segment| segment.p_type(endian) == PT_DYNAMIC)
        .unwrap();
    assert_eq!(dynamic_segment.p_vaddr(endian), dynamic.address());
    assert_eq!(dynamic_segment.p_memsz(endian), dynamic.size());
    let dynamic_entries = dynamic
        .data()
        .unwrap()
        .as_chunks::<16>()
        .0
        .iter()
        .map(|entry| {
            let (tag, value) = entry.split_at(8);
            (
                i64::from_le_bytes(tag.try_into().unwrap()),
                u64::from_le_bytes(value.try_into().unwrap()),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(dynamic_entries.last(), Some(&(DT_NULL.0, 0)));
    let dynamic_entries = dynamic_entries.into_iter().collect::<HashMap<_, _>>();
    assert_eq!(dynamic_entries[&DT_FLAGS.0], (DF_SYMBOLIC | DF_BIND_NOW).0);
    assert_eq!(dynamic_entries[&DT_FLAGS_1.0], DF_1_NOW.0);
    assert_eq!(dynamic_entries[&DT_SYMTAB.0], section_address(".dynsym"));
    assert_eq!(dynamic_entries[&DT_SYMENT.0], 24);
    assert_eq!(dynamic_entries[&DT_STRTAB.0], section_address(".dynstr"));
    assert_eq!(
        dynamic_entries[&DT_STRSZ.0],
        elf.section_by_name(".dynstr").unwrap().size()
    );
    assert_eq!(dynamic_entries[&DT_HASH.0], section_address(".hash"));

    let imports = elf
        .imports()
        .unwrap()
        .map(|import| import.unwrap().name().into_name().unwrap().to_vec())
        .collect::<Vec<_>>();
    let relocations = elf
        .dynamic_relocations()
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    if header.host_call_fn_offset == 0 {
        assert!(imports.is_empty());
        assert!(relocations.is_empty());
        assert!(elf.section_by_name(".plt").is_none());
        assert!(!dynamic_entries.contains_key(&DT_PLTGOT.0));
    } else {
        expected_num_symbols += 2;

        let host_call_fn = dynamic_symbols
            .symbol_by_index(find_symbol_using_hash_table(&elf, HOST_CALL_FN))
            .unwrap();
        let host_call_fn_address =
            code_address + u64::from(header.host_call_fn_offset) - code_offset as u64;
        assert_eq!(host_call_fn.kind(), SymbolKind::Text);
        assert_eq!(host_call_fn.address(), host_call_fn_address);
        // Code is followed by PLT, so there are always enough bytes for the largest instruction
        let host_call_fn_instruction = decode(read_memory(host_call_fn_address, 4));
        assert_eq!(
            host_call_fn.size(),
            u64::from(host_call_fn_instruction.size())
        );
        #[expect(clippy::rest_pattern_accessible_field, reason = "Not needed")]
        let plt_entry_address = match host_call_fn_instruction {
            ContractInstruction::Jal {
                rd: ContractRegister::Zero,
                imm,
                ..
            } => host_call_fn_address.wrapping_add_signed(i64::from(imm)),
            ContractInstruction::CJ { imm, .. } => {
                host_call_fn_address.wrapping_add_signed(i64::from(imm))
            }
            instruction => panic!("Unexpected host call function instruction {instruction}"),
        };

        let import = find_symbol_using_hash_table(&elf, HOST_CALL_FN_IMPORT);
        let import_symbol = dynamic_symbols.symbol_by_index(import).unwrap();
        assert!(import_symbol.is_undefined());
        assert_eq!(import_symbol.elf_symbol().st_shndx.get(endian), SHN_UNDEF);
        assert_eq!(imports, [HOST_CALL_FN_IMPORT.as_bytes()]);

        // PLT entry loads the target address from the GOT slot that the dynamic linker fills
        // according to the relocation and jumps there
        #[expect(clippy::rest_pattern_accessible_field, reason = "Not needed")]
        let got_slot_address = match (
            decode(read_memory(plt_entry_address, 4)),
            decode(read_memory(plt_entry_address + 4, 4)),
            decode(read_memory(plt_entry_address + 8, 4)),
        ) {
            (
                ContractInstruction::Auipc {
                    rd: ContractRegister::T3,
                    imm: hi,
                    ..
                },
                ContractInstruction::Ld {
                    rd: ContractRegister::T3,
                    rs1: ContractRegister::T3,
                    imm: lo,
                    ..
                },
                ContractInstruction::Jalr {
                    rd: ContractRegister::T1,
                    rs1: ContractRegister::T3,
                    imm: 0,
                    ..
                },
            ) => plt_entry_address
                .wrapping_add_signed(i64::from(hi))
                .wrapping_add_signed(i64::from(lo)),
            instructions => panic!("Unexpected PLT entry instructions {instructions:?}"),
        };
        assert_eq!(relocations.len(), 1);
        let (relocation_address, relocation) = &relocations[0];
        assert_eq!(*relocation_address, got_slot_address);
        assert_eq!(
            relocation.flags(),
            RelocationFlags::Elf {
                r_type: R_RISCV_JUMP_SLOT
            }
        );
        assert_eq!(relocation.target(), RelocationTarget::Symbol(import));
        assert_eq!(relocation.addend(), 0);
        assert!(!relocation.has_implicit_addend());

        // The GOT slot initially points to the PLT header, which loads the lazy binding resolver
        // from the beginning of `.got.plt`
        let plt_address = plt_entry_address - PLT_HEADER_SIZE;
        assert_eq!(section_address(".plt"), plt_address);
        assert_eq!(segment_flags(plt_address), PF_R | PF_X);
        assert_eq!(read_memory(got_slot_address, 8), plt_address.to_le_bytes());
        #[expect(clippy::rest_pattern_accessible_field, reason = "Not needed")]
        let got_plt_address = match (
            decode(read_memory(plt_address, 4)),
            decode(read_memory(plt_address + 8, 4)),
        ) {
            (
                ContractInstruction::Auipc {
                    rd: ContractRegister::T2,
                    imm: hi,
                    ..
                },
                ContractInstruction::Ld {
                    rd: ContractRegister::T3,
                    rs1: ContractRegister::T2,
                    imm: lo,
                    ..
                },
            ) => plt_address
                .wrapping_add_signed(i64::from(hi))
                .wrapping_add_signed(i64::from(lo)),
            instructions => panic!("Unexpected PLT header instructions {instructions:?}"),
        };
        assert_eq!(section_address(".got.plt"), got_plt_address);
        assert_eq!(segment_flags(got_plt_address), PF_R | PF_W);
        assert_eq!(got_slot_address, got_plt_address + 16);

        assert_eq!(dynamic_entries[&DT_PLTGOT.0], got_plt_address);
        assert_eq!(dynamic_entries[&DT_JMPREL.0], section_address(".rela.plt"));
        assert_eq!(dynamic_entries[&DT_PLTRELSZ.0], 24);
        assert_eq!(dynamic_entries[&DT_PLTREL.0], DT_RELA.0 as u64);
    }

    assert_eq!(dynamic_symbols.symbols().count(), expected_num_symbols);
}

#[test]
fn rodata_before_metadata_with_compressed_host_call() {
    let contract = TestContract {
        rodata_before_metadata: (0..=255).collect(),
        // Similar to what `lld` produces when it places code on a separate page
        min_read_only_padding: 0x1001,
        code: [
            [LI_A0_0, RET].concat(),
            RET.to_vec(),
            [LI_A0_0, LI_A0_0, RET].concat(),
            // Jumps to the only PLT entry after the PLT header, which is placed with a small gap
            // after code
            c_j(48).to_vec(),
        ]
        .concat(),
        methods: vec![("first", 0, 4), ("second", 4, 2), ("third", 6, 6)],
        host_call_fn_offset: Some(12),
        ..TestContract::default()
    };

    recover_and_check(&contract.build());
}

#[test]
fn metadata_before_rodata_with_distant_host_call() {
    let contract = TestContract {
        rodata_after_metadata: vec![0xaa; 13],
        // Read-only data and code share a page in memory
        min_read_only_padding: 0x10,
        code: [
            [LI_A0_0, RET].concat(),
            RET.to_vec(),
            // Jumps far away from code, leaving a large gap before PLT, and uses immediate bits in
            // both halves of the instruction
            jal_zero(0x1_5556).to_vec(),
        ]
        .concat(),
        methods: vec![("first", 0, 4), ("second", 4, 2)],
        host_call_fn_offset: Some(6),
        ..TestContract::default()
    };

    recover_and_check(&contract.build());
}

#[test]
fn metadata_only_without_host_call() {
    let contract = TestContract {
        min_read_only_padding: 0x3456,
        code: [[LI_A0_0, RET].concat(), [LI_A0_0, LI_A0_0, RET].concat()].concat(),
        // Identical methods might be folded into one by the linker
        methods: vec![("first", 0, 4), ("second", 4, 6), ("third", 4, 6)],
        ..TestContract::default()
    };

    recover_and_check(&contract.build());
}

#[test]
fn host_call_without_methods() {
    let contract = TestContract {
        rodata_before_metadata: vec![0xaa; 3],
        code: c_j(48).to_vec(),
        host_call_fn_offset: Some(0),
        ..TestContract::default()
    };

    recover_and_check(&contract.build());
}

#[test]
fn plt_right_after_code() {
    let contract = TestContract {
        // PLT header starts right where code ends
        code: [RET.to_vec(), c_j(34).to_vec()].concat(),
        methods: vec![("first", 0, 2)],
        host_call_fn_offset: Some(2),
        ..TestContract::default()
    };

    recover_and_check(&contract.build());
}

#[test]
fn largest_contract_memory() {
    let code = [RET.to_vec(), c_j(34).to_vec()].concat();
    let code_size = u32::try_from(code.len()).unwrap();
    let contract = TestContract {
        read_only_section_memory_size: Some((u32::MAX - code_size) & !1),
        code,
        methods: vec![("first", 0, 2)],
        host_call_fn_offset: Some(2),
        ..TestContract::default()
    };

    recover_and_check(&contract.build());
}

#[test]
fn contract_memory_too_large() {
    let contract = TestContract {
        read_only_section_memory_size: Some(u32::MAX - 1),
        code: [RET.to_vec(), c_j(34).to_vec()].concat(),
        methods: vec![("first", 0, 2)],
        host_call_fn_offset: Some(2),
        ..TestContract::default()
    };
    let contract_file = contract.build();

    ContractFile::parse(&contract_file, |_| Ok(())).unwrap_err();
    recover(&contract_file).unwrap_err();
}

#[test]
fn method_range_overflow() {
    let contract = TestContract {
        code: RET.to_vec(),
        methods: vec![("first", 0, u32::MAX)],
        ..TestContract::default()
    };
    let contract_file = contract.build();

    ContractFile::parse(&contract_file, |_| Ok(())).unwrap_err();
    recover(&contract_file).unwrap_err();
}

#[test]
fn plt_overlapping_code() {
    let contract = TestContract {
        // PLT header would overlap with the host call function
        code: [RET.to_vec(), c_j(32).to_vec()].concat(),
        methods: vec![("first", 0, 2)],
        host_call_fn_offset: Some(2),
        ..TestContract::default()
    };
    let contract_file = contract.build();

    ContractFile::parse(&contract_file, |_| Ok(())).unwrap();
    recover(&contract_file).unwrap_err();
}

#[test]
fn metadata_in_the_middle_of_read_only_data() {
    let contract = TestContract {
        rodata_before_metadata: vec![0xaa; 3],
        rodata_after_metadata: vec![0xbb; 5],
        code: RET.to_vec(),
        methods: vec![("first", 0, 2)],
        ..TestContract::default()
    };
    let contract_file = contract.build();

    ContractFile::parse(&contract_file, |_| Ok(())).unwrap();
    recover(&contract_file).unwrap_err();
}

#[test]
fn host_call_fn_jumping_into_code() {
    let contract = TestContract {
        code: [RET.to_vec(), c_j(0).to_vec()].concat(),
        methods: vec![("first", 0, 2)],
        host_call_fn_offset: Some(2),
        ..TestContract::default()
    };
    let contract_file = contract.build();

    ContractFile::parse(&contract_file, |_| Ok(())).unwrap();
    recover(&contract_file).unwrap_err();
}

#[test]
fn duplicate_method_names() {
    let contract = TestContract {
        code: RET.to_vec(),
        methods: vec![("first", 0, 2), ("first", 0, 2)],
        ..TestContract::default()
    };
    let contract_file = contract.build();

    ContractFile::parse(&contract_file, |_| Ok(())).unwrap();
    recover(&contract_file).unwrap_err();
}

#[test]
fn reserved_method_name() {
    let contract = TestContract {
        code: RET.to_vec(),
        methods: vec![(HOST_CALL_FN, 0, 2)],
        ..TestContract::default()
    };
    let contract_file = contract.build();

    ContractFile::parse(&contract_file, |_| Ok(())).unwrap();
    recover(&contract_file).unwrap_err();
}

/// Build an example contract, then check that the ELF recovered from the contract file is
/// equivalent to the original one produced by the linker
fn check_example_contract(package: &str) {
    let target_specification =
        TargetSpecification::create(Path::new(env!("CARGO_TARGET_TMPDIR"))).unwrap();
    let cdylib_path = build_cdylib(BuildOptions {
        package: Some(package),
        features: None,
        no_default_features: false,
        profile: "contract",
        target_specification_path: target_specification.path(),
        target_dir: None,
    })
    .unwrap();
    let original_elf_bytes = fs::read(cdylib_path).unwrap();

    let contract_file = convert(&original_elf_bytes).unwrap();
    let recovered_elf_bytes = recover_and_check(&contract_file);

    let original_elf = ElfFile64::<LittleEndian>::parse(original_elf_bytes.as_slice()).unwrap();
    let recovered_elf = ElfFile64::<LittleEndian>::parse(recovered_elf_bytes.as_slice()).unwrap();

    for section_name in [".text", "ab-contract-metadata"] {
        assert_eq!(
            original_elf
                .section_by_name(section_name)
                .unwrap()
                .data()
                .unwrap(),
            recovered_elf
                .section_by_name(section_name)
                .unwrap()
                .data()
                .unwrap(),
            "Section {section_name} differs"
        );
    }

    // The compiler also lists extensions implied by their parts (like `c` for `zca`) in
    // `Tag_RISCV_arch`, while the recovered file uses ISA string of contract instructions, which
    // doesn't, so only the latter's extensions must be present in the former
    let (original_stack_align, original_arch) = riscv_attributes(&original_elf);
    let (recovered_stack_align, recovered_arch) = riscv_attributes(&recovered_elf);
    assert_eq!(original_stack_align, recovered_stack_align);
    assert_eq!(
        recovered_arch,
        <ContractInstruction as Instruction>::ISA_STRING
    );
    let original_extensions = isa_string_extensions(&original_arch);
    for extension in isa_string_extensions(&recovered_arch) {
        assert!(
            original_extensions.contains(&extension),
            "{extension} is not in {original_arch}"
        );
    }

    // Functions and PLT are at the same offsets relative to the beginning of code
    let relative_functions = |elf: &ElfFile64<'_, LittleEndian>| {
        let code_address = elf.section_by_name(".text").unwrap().address();
        let mut functions = elf
            .dynamic_symbols()
            .filter(|symbol| symbol.kind() == SymbolKind::Text)
            .map(|symbol| {
                (
                    symbol.name().unwrap().to_string(),
                    symbol.address() - code_address,
                    symbol.size(),
                )
            })
            .collect::<Vec<_>>();
        functions.sort();
        let plt_offset = elf.section_by_name(".plt").unwrap().address() - code_address;
        (functions, plt_offset)
    };
    assert_eq!(
        relative_functions(&original_elf),
        relative_functions(&recovered_elf)
    );
}

/// `Tag_RISCV_stack_align` and `Tag_RISCV_arch` attributes, which must be the only attributes
fn riscv_attributes(elf: &ElfFile64<'_, LittleEndian>) -> (u64, String) {
    let attributes_section = elf
        .section_by_name(".riscv.attributes")
        .unwrap()
        .elf_section_header()
        .attributes(elf.endian(), elf.data())
        .unwrap();
    let mut subsections = attributes_section.subsections().unwrap();
    let subsection = subsections.next().unwrap().unwrap();
    assert_eq!(subsection.vendor(), b"riscv");
    assert!(subsections.next().unwrap().is_none());
    let mut subsubsections = subsection.subsubsections();
    let subsubsection = subsubsections.next().unwrap().unwrap();
    assert!(subsubsections.next().unwrap().is_none());

    let mut attributes = subsubsection.attributes();
    let mut stack_align = None;
    let mut arch = None;
    while let Some(tag) = attributes.read_tag().unwrap() {
        match tag {
            // `Tag_RISCV_stack_align`
            4 => {
                stack_align = Some(attributes.read_integer().unwrap());
            }
            // `Tag_RISCV_arch`
            5 => {
                arch = Some(
                    str::from_utf8(attributes.read_string().unwrap())
                        .unwrap()
                        .to_string(),
                );
            }
            tag => {
                panic!("Unexpected attribute tag {tag}");
            }
        }
    }

    (stack_align.unwrap(), arch.unwrap())
}

/// Extensions of an ISA string with versions, the base ISA is the first one
fn isa_string_extensions(isa_string: &str) -> Vec<&str> {
    isa_string
        .strip_prefix("rv64")
        .unwrap()
        .split('_')
        .collect()
}

#[test]
#[cfg_attr(miri, ignore = "Builds contracts with Cargo")]
fn example_contract_flipper() {
    check_example_contract("ab-example-contract-flipper");
}

#[test]
#[cfg_attr(miri, ignore = "Builds contracts with Cargo")]
fn example_contract_ft() {
    check_example_contract("ab-example-contract-ft");
}

#[test]
#[cfg_attr(miri, ignore = "Builds contracts with Cargo")]
fn example_contract_wallet() {
    check_example_contract("ab-example-contract-wallet");
}
