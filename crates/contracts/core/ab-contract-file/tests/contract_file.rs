#![feature(core_io_borrowed_buf)]

use ab_contract_file::{
    CONTRACT_FILE_MAGIC, ContractFile, ContractFileHeader, ContractFileMethodMetadata,
    ContractFileParseError,
};
use ab_contracts_common::metadata::ContractMetadataKind;
use ab_io_type::trivial_type::TrivialType;
use std::assert_matches;
use std::io::BorrowedBuf;
use std::mem::offset_of;

/// `c.li a0, 0`
const LI_A0_0: [u8; 2] = 0x4501_u16.to_le_bytes();
/// `c.jr ra`
const RET: [u8; 2] = 0x8082_u16.to_le_bytes();
/// The first half of `jal zero, 0`
const JAL_ZERO_0_FIRST_HALF: [u8; 2] = 0x006f_u16.to_le_bytes();

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

/// Contract file with read-only section consisting of `.rodata` followed by metadata, and code
/// consisting of methods placed one after another
fn contract_file(
    rodata: &[u8],
    metadata: &[u8],
    read_only_padding: u32,
    methods: &[Vec<u8>],
) -> Vec<u8> {
    let num_methods = u16::try_from(methods.len()).unwrap();
    let read_only_offset =
        ContractFileHeader::SIZE + u32::from(num_methods) * ContractFileMethodMetadata::SIZE;
    let read_only_section_file_size = u32::try_from(rodata.len() + metadata.len()).unwrap();

    let header = ContractFileHeader {
        magic: CONTRACT_FILE_MAGIC,
        read_only_section_file_size,
        read_only_section_memory_size: read_only_section_file_size + read_only_padding,
        metadata_offset: read_only_offset + u32::try_from(rodata.len()).unwrap(),
        metadata_size: u16::try_from(metadata.len()).unwrap(),
        num_methods,
        host_call_fn_offset: 0,
    };

    let mut file = header.as_bytes().to_vec();
    let mut method_offset = read_only_offset + read_only_section_file_size;
    for method in methods {
        let size = u32::try_from(method.len()).unwrap();
        let method_metadata = ContractFileMethodMetadata {
            offset: method_offset,
            size,
        };
        file.extend_from_slice(method_metadata.as_bytes());
        method_offset += size;
    }
    file.extend_from_slice(rodata);
    file.extend_from_slice(metadata);
    file.extend(methods.iter().flatten());
    file
}

#[test]
fn method_addresses() {
    let rodata = [0xaa; 13];
    let metadata = trait_metadata("Test", &["first", "second", "third"]);
    let methods = [
        [LI_A0_0, RET].concat(),
        RET.to_vec(),
        [LI_A0_0, LI_A0_0, RET].concat(),
    ];
    // Similar to what `lld` produces when it places code on a separate page, results in code
    // starting at an even address
    let read_only_padding = 0x1001;
    let file = contract_file(&rodata, &metadata, read_only_padding, &methods);

    let mut parsed_methods = Vec::new();
    let contract_file = ContractFile::parse(&file, |method| {
        parsed_methods.push((
            method.method_metadata_item.method_name,
            method.address,
            method.size,
        ));
        Ok(())
    })
    .unwrap();

    // Contract memory contains read-only data, padding and code, so method addresses are offsets
    // within code shifted by the size of read-only data and padding
    let code_address = contract_file.header().read_only_section_memory_size;
    assert_eq!(
        code_address,
        u32::try_from(rodata.len() + metadata.len()).unwrap() + read_only_padding
    );
    let expected_methods = [
        (b"first".as_slice(), code_address, 4),
        (b"second".as_slice(), code_address + 4, 2),
        (b"third".as_slice(), code_address + 6, 6),
    ];
    assert_eq!(parsed_methods, expected_methods);

    let iterated_methods = contract_file
        .iterate_methods()
        .map(|method| {
            (
                method.method_metadata_item.method_name,
                method.address,
                method.size,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(iterated_methods, expected_methods);

    let mut contract_memory = vec![0; contract_file.contract_memory_size() as usize];
    assert!(
        contract_file.initialize_contract_memory(
            BorrowedBuf::from(contract_memory.as_mut_slice()).unfilled()
        )
    );
    for ((_, address, size), method) in expected_methods.into_iter().zip(&methods) {
        assert_eq!(
            &contract_memory[address as usize..][..size as usize],
            method.as_slice()
        );
    }
}

#[test]
fn method_range_overflow() {
    let metadata = trait_metadata("Test", &["first"]);
    let read_only_padding = u32::try_from(metadata.len() % 2).unwrap();
    let mut file = contract_file(&[], &metadata, read_only_padding, &[RET.to_vec()]);
    ContractFile::parse(&file, |_| Ok(())).unwrap();

    // Method offset and size that overflow `u32` when added together
    let code_offset = u32::try_from(file.len() - RET.len()).unwrap();
    let method_metadata = ContractFileMethodMetadata {
        offset: code_offset,
        size: u32::MAX,
    };
    file[ContractFileHeader::SIZE as usize..][..ContractFileMethodMetadata::SIZE as usize]
        .copy_from_slice(method_metadata.as_bytes());

    assert_matches!(
        ContractFile::parse(&file, |_| Ok(())),
        Err(ContractFileParseError::FileTooSmall {
            num_methods: _,
            read_only_section_size: _,
            file_size: _
        })
    );
}

#[test]
fn method_at_end_of_code() {
    let metadata = trait_metadata("Test", &["first"]);
    let read_only_padding = u32::try_from(metadata.len() % 2).unwrap();
    let mut file = contract_file(&[], &metadata, read_only_padding, &[RET.to_vec()]);
    ContractFile::parse(&file, |_| Ok(())).unwrap();

    // Empty method that starts right after the last instruction
    let file_size = u32::try_from(file.len()).unwrap();
    let method_metadata = ContractFileMethodMetadata {
        offset: file_size,
        size: 0,
    };
    file[ContractFileHeader::SIZE as usize..][..ContractFileMethodMetadata::SIZE as usize]
        .copy_from_slice(method_metadata.as_bytes());

    assert_matches!(
        ContractFile::parse(&file, |_| Ok(())),
        Err(ContractFileParseError::MethodOutOfRange {
            offset,
            code_section_offset: _,
            file_size: _
        }) if offset == file_size
    );
}

#[test]
fn truncated_last_instruction() {
    let metadata = trait_metadata("Test", &["first"]);
    let read_only_padding = u32::try_from(metadata.len() % 2).unwrap();
    // The last two bytes are the first half of `jal zero, 0`, which only decodes when padded with
    // zeroes that are not in the file
    let file = contract_file(
        &[],
        &metadata,
        read_only_padding,
        &[[LI_A0_0, JAL_ZERO_0_FIRST_HALF].concat()],
    );

    assert_matches!(
        ContractFile::parse(&file, |_| Ok(())),
        Err(ContractFileParseError::UnexpectedTrailingCodeBytes { num_bytes: 2 })
    );
}

#[test]
fn truncated_host_call_fn() {
    let metadata = trait_metadata("Test", &["first"]);
    let read_only_padding = u32::try_from(metadata.len() % 2).unwrap();
    let mut file = contract_file(
        &[],
        &metadata,
        read_only_padding,
        &[[RET, JAL_ZERO_0_FIRST_HALF].concat()],
    );
    // Host call function is the first half of `jal zero, 0` in the last two bytes of the file
    let host_call_fn_offset = u32::try_from(file.len() - JAL_ZERO_0_FIRST_HALF.len()).unwrap();
    file[offset_of!(ContractFileHeader, host_call_fn_offset)..][..size_of::<u32>()]
        .copy_from_slice(&host_call_fn_offset.to_le_bytes());

    assert_matches!(
        ContractFile::parse(&file, |_| Ok(())),
        Err(ContractFileParseError::HostCallFnOutOfRange {
            offset,
            code_section_offset: _,
            file_size: _
        }) if offset == host_call_fn_offset
    );
}

#[test]
fn contract_memory_size_limit() {
    let metadata = trait_metadata("Test", &["first"]);
    let methods = [RET.to_vec()];
    let read_only_section_file_size = u32::try_from(metadata.len()).unwrap();
    let code_size = u32::try_from(RET.len()).unwrap();
    // The largest contract memory where the method still starts at an even address
    let largest_read_only_section_memory_size = (u32::MAX - code_size) & !1;
    let largest_read_only_padding =
        largest_read_only_section_memory_size - read_only_section_file_size;

    let file = contract_file(&[], &metadata, largest_read_only_padding, &methods);
    // Contract memory no longer fits into `u32`
    let too_large_file = contract_file(&[], &metadata, largest_read_only_padding + 2, &methods);

    let mut parsed_addresses = Vec::new();
    let contract_file = ContractFile::parse(&file, |method| {
        parsed_addresses.push(method.address);
        Ok(())
    })
    .unwrap();
    assert_eq!(parsed_addresses, [largest_read_only_section_memory_size]);
    assert_eq!(
        contract_file
            .iterate_methods()
            .map(|method| method.address)
            .collect::<Vec<_>>(),
        [largest_read_only_section_memory_size]
    );
    assert_eq!(
        contract_file.contract_memory_size(),
        largest_read_only_section_memory_size + code_size
    );

    assert_matches!(
        ContractFile::parse(&too_large_file, |_| Ok(())),
        Err(ContractFileParseError::ContractMemoryTooLarge {
            read_only_section_memory_size: _,
            code_size: _
        })
    );
}
