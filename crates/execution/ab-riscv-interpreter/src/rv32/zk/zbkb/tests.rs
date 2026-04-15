use crate::rv32::test_utils::{execute, initialize_state};
use ab_riscv_primitives::prelude::*;

#[test]
fn test_pack() {
    let mut state = initialize_state([Rv32ZbkbInstruction::Pack {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    // rs1 lower 16 bits -> rd[15:0], rs2 lower 16 bits -> rd[31:16]
    state.regs.write(Reg::A0, 0xDEAD_1234u32);
    state.regs.write(Reg::A1, 0xCAFE_5678u32);

    execute(&mut state).unwrap();

    // rd[15:0] = rs1[15:0] = 0x1234
    // rd[31:16] = rs2[15:0] = 0x5678
    assert_eq!(state.regs.read(Reg::A2), 0x5678_1234u32);
}

#[test]
fn test_pack_zeros() {
    let mut state = initialize_state([Rv32ZbkbInstruction::Pack {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0);
    state.regs.write(Reg::A1, 0);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0);
}

#[test]
fn test_packh() {
    let mut state = initialize_state([Rv32ZbkbInstruction::Packh {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    // rd[7:0]  = rs1[7:0], rd[15:8] = rs2[7:0], rd[31:16] = 0
    state.regs.write(Reg::A0, 0xFFFF_FF42u32);
    state.regs.write(Reg::A1, 0xFFFF_FF37u32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0x0000_3742u32);
}

#[test]
fn test_packh_only_low_bytes() {
    let mut state = initialize_state([Rv32ZbkbInstruction::Packh {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0xAB);
    state.regs.write(Reg::A1, 0xCD);

    execute(&mut state).unwrap();

    // rd = 0x0000_CD_AB
    assert_eq!(state.regs.read(Reg::A2), 0xCDABu32);
}

#[test]
fn test_brev8() {
    let mut state = initialize_state([Rv32ZbkbInstruction::Brev8 {
        rd: Reg::A2,
        rs1: Reg::A0,
    }]);

    // Each byte has its bits reversed individually:
    // 0x01 -> 0x80, 0x02 -> 0x40, 0x03 -> 0xC0, 0x04 -> 0x20
    state.regs.write(Reg::A0, 0x0403_0201u32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0x20C0_4080u32);
}

#[test]
fn test_brev8_all_ones() {
    let mut state = initialize_state([Rv32ZbkbInstruction::Brev8 {
        rd: Reg::A2,
        rs1: Reg::A0,
    }]);

    // 0xFF reversed is 0xFF
    state.regs.write(Reg::A0, 0xFFFF_FFFFu32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xFFFF_FFFFu32);
}

#[test]
fn test_brev8_single_byte() {
    let mut state = initialize_state([Rv32ZbkbInstruction::Brev8 {
        rd: Reg::A2,
        rs1: Reg::A0,
    }]);

    // 0x01 = 0b00000001 reversed is 0b10000000 = 0x80
    state.regs.write(Reg::A0, 0x01u32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0x80u32);
}

#[test]
fn test_brev8_zero() {
    let mut state = initialize_state([Rv32ZbkbInstruction::Brev8 {
        rd: Reg::A2,
        rs1: Reg::A0,
    }]);

    state.regs.write(Reg::A0, 0u32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0u32);
}

#[test]
fn test_zip_lower_half_only() {
    let mut state = initialize_state([Rv32ZbkbInstruction::Zip {
        rd: Reg::A2,
        rs1: Reg::A0,
    }]);

    // Only bit 0 set in lower half: should scatter to rd[0]
    state.regs.write(Reg::A0, 0x0000_0001u32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0x0000_0001u32);
}

#[test]
fn test_zip_upper_half_only() {
    let mut state = initialize_state([Rv32ZbkbInstruction::Zip {
        rd: Reg::A2,
        rs1: Reg::A0,
    }]);

    // Only bit 16 set (upper half bit 0): should scatter to rd[1]
    state.regs.write(Reg::A0, 0x0001_0000u32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0x0000_0002u32);
}

#[test]
fn test_zip_all_ones() {
    let mut state = initialize_state([Rv32ZbkbInstruction::Zip {
        rd: Reg::A2,
        rs1: Reg::A0,
    }]);

    // All bits set: zip of all-ones is all-ones
    state.regs.write(Reg::A0, 0xFFFF_FFFFu32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xFFFF_FFFFu32);
}

#[test]
fn test_zip_zero() {
    let mut state = initialize_state([Rv32ZbkbInstruction::Zip {
        rd: Reg::A2,
        rs1: Reg::A0,
    }]);

    state.regs.write(Reg::A0, 0u32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0u32);
}

#[test]
fn test_unzip_lower_half_only() {
    let mut state = initialize_state([Rv32ZbkbInstruction::Unzip {
        rd: Reg::A2,
        rs1: Reg::A0,
    }]);

    // rd[0] (even) -> result[0]; rd[1] (odd) -> result[16]
    state.regs.write(Reg::A0, 0x0000_0003u32); // bits 0 and 1 set

    execute(&mut state).unwrap();

    // bit 0 (even) -> result[0], bit 1 (odd) -> result[16]
    assert_eq!(state.regs.read(Reg::A2), 0x0001_0001u32);
}

#[test]
fn test_unzip_all_ones() {
    let mut state = initialize_state([Rv32ZbkbInstruction::Unzip {
        rd: Reg::A2,
        rs1: Reg::A0,
    }]);

    state.regs.write(Reg::A0, 0xFFFF_FFFFu32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xFFFF_FFFFu32);
}

#[test]
fn test_unzip_zero() {
    let mut state = initialize_state([Rv32ZbkbInstruction::Unzip {
        rd: Reg::A2,
        rs1: Reg::A0,
    }]);

    state.regs.write(Reg::A0, 0u32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0u32);
}

#[test]
fn test_zip_then_unzip_is_identity() {
    // zip followed by unzip must recover the original value
    let original = 0xDEAD_BEEFu32;

    let mut state = initialize_state([
        Rv32ZbkbInstruction::Zip {
            rd: Reg::A1,
            rs1: Reg::A0,
        },
        Rv32ZbkbInstruction::Unzip {
            rd: Reg::A2,
            rs1: Reg::A1,
        },
    ]);

    state.regs.write(Reg::A0, original);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), original);
}

#[test]
fn test_unzip_then_zip_is_identity() {
    // unzip followed by zip must recover the original value
    let original = 0xCAFE_BABEu32;

    let mut state = initialize_state([
        Rv32ZbkbInstruction::Unzip {
            rd: Reg::A1,
            rs1: Reg::A0,
        },
        Rv32ZbkbInstruction::Zip {
            rd: Reg::A2,
            rs1: Reg::A1,
        },
    ]);

    state.regs.write(Reg::A0, original);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), original);
}
