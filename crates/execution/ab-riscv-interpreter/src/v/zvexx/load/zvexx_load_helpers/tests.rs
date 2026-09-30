use crate::VirtualMemoryError;
use crate::basic::BasicMemory;
use crate::v::zvexx::load::zvexx_load_helpers::{access_wraps, effective_address, read_bytes};
use crate::v::zvexx::store::zvexx_store_helpers::write_bytes;
use ab_riscv_primitives::prelude::*;
use core::assert_matches;

/// Last 64 bytes of the 32-bit address space
const RV32_HIGH_ADDR: u64 = (1 << 32) - 64;

#[test]
fn effective_address_wraps_modulo_xlen() {
    assert_eq!(effective_address::<Reg<u32>>(0xffff_fff0, 0x20), 0x10);
    assert_eq!(effective_address::<Reg<u32>>(0x1000, 0x20), 0x1020);
    // Offsets wider than XLEN only contribute their low XLEN bits
    assert_eq!(effective_address::<Reg<u32>>(0x1000, 0x1_0000_0000), 0x1000);
    assert_eq!(effective_address::<Reg<u64>>(u64::MAX - 0xf, 0x20), 0x10);
    assert_eq!(effective_address::<Reg<u64>>(0x1000, u64::MAX), 0xfff);
}

#[test]
fn access_wraps_only_past_end_of_address_space() {
    assert!(!access_wraps::<Reg<u32>>(0xffff_fff0, 0x10));
    assert!(access_wraps::<Reg<u32>>(0xffff_fff0, 0x11));
    assert!(!access_wraps::<Reg<u64>>(0xffff_fff0, 0x11));
    assert!(!access_wraps::<Reg<u64>>(u64::MAX - 0xf, 0x10));
    assert!(access_wraps::<Reg<u64>>(u64::MAX - 0xf, 0x11));
}

#[test]
fn rv32_read_bytes_wraps_to_address_zero() {
    let mut memory = BasicMemory::<RV32_HIGH_ADDR, 64>::default();
    memory
        .get_mut_bytes(RV32_HIGH_ADDR + 48, 16)
        .unwrap()
        .fill(0xaa);

    // Fully within the address space
    let mut dst = [0; 16];
    read_bytes::<Reg<u32>, _>(&memory, RV32_HIGH_ADDR + 48, &mut dst).unwrap();
    assert_eq!(dst, [0xaa; 16]);

    // The remaining bytes come from address zero, not from above 4 GiB, which this memory does not
    // have either
    let mut dst = [0; 17];
    assert_matches!(
        read_bytes::<Reg<u32>, _>(&memory, RV32_HIGH_ADDR + 48, &mut dst),
        Err(VirtualMemoryError::OutOfBoundsRead { address: 0 })
    );
}

#[test]
fn rv32_write_bytes_wraps_to_address_zero() {
    let mut memory = BasicMemory::<RV32_HIGH_ADDR, 64>::default();

    write_bytes::<Reg<u32>, _>(&mut memory, RV32_HIGH_ADDR + 48, &[0xaa; 16]).unwrap();
    assert_eq!(
        memory.get_mut_bytes(RV32_HIGH_ADDR + 48, 16).unwrap(),
        [0xaa; 16]
    );

    assert_matches!(
        write_bytes::<Reg<u32>, _>(&mut memory, RV32_HIGH_ADDR + 48, &[0xbb; 17]),
        Err(VirtualMemoryError::OutOfBoundsWrite { address: 0 })
    );
}
