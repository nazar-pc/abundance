use crate::rv64::test_utils::{execute, initialize_state};
use ab_riscv_primitives::instructions::rv64::zk::zbkb::Rv64ZbkbInstruction;
use ab_riscv_primitives::registers::general_purpose::Reg;

#[test]
fn test_pack() {
    let mut state = initialize_state([Rv64ZbkbInstruction::Pack {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    // rs1 lower 32 bits -> rd[31:0], rs2 lower 32 bits -> rd[63:32]
    state.regs.write(Reg::A0, 0xDEAD_BEEF_1234_5678u64);
    state.regs.write(Reg::A1, 0xCAFE_BABE_ABCD_EF01u64);

    execute(&mut state).unwrap();

    // rd[31:0] = rs1[31:0] = 0x1234_5678
    // rd[63:32] = rs2[31:0] = 0xABCD_EF01
    assert_eq!(state.regs.read(Reg::A2), 0xABCD_EF01_1234_5678u64);
}

#[test]
fn test_pack_zeros() {
    let mut state = initialize_state([Rv64ZbkbInstruction::Pack {
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
    let mut state = initialize_state([Rv64ZbkbInstruction::Packh {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    // rd[7:0]  = rs1[7:0], rd[15:8] = rs2[7:0], rd[63:16] = 0
    state.regs.write(Reg::A0, 0xFFFF_FFFF_FFFF_FF42u64);
    state.regs.write(Reg::A1, 0xFFFF_FFFF_FFFF_FF37u64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0x0000_0000_0000_3742u64);
}

#[test]
fn test_packh_only_low_bytes() {
    let mut state = initialize_state([Rv64ZbkbInstruction::Packh {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0xAB);
    state.regs.write(Reg::A1, 0xCD);

    execute(&mut state).unwrap();

    // rd = 0x00..00_CD_AB
    assert_eq!(state.regs.read(Reg::A2), 0xCDABu64);
}

#[test]
fn test_packw() {
    let mut state = initialize_state([Rv64ZbkbInstruction::Packw {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    // rd[15:0] = rs1[15:0], rd[31:16] = rs2[15:0], then sign-extend to 64
    state.regs.write(Reg::A0, 0xFFFF_FFFF_FFFF_1234u64);
    state.regs.write(Reg::A1, 0xFFFF_FFFF_FFFF_5678u64);

    execute(&mut state).unwrap();

    // word = 0x5678_1234, sign-extended: positive, so upper bits are 0
    assert_eq!(state.regs.read(Reg::A2), 0x0000_0000_5678_1234u64);
}

#[test]
fn test_packw_sign_extension() {
    let mut state = initialize_state([Rv64ZbkbInstruction::Packw {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    // rs2[15:0] = 0x8000 -> word bit 31 is 1 -> sign-extend to 64 bits
    state.regs.write(Reg::A0, 0x0000u64);
    state.regs.write(Reg::A1, 0x8000u64);

    execute(&mut state).unwrap();

    // word = 0x8000_0000, sign-extended to 0xFFFF_FFFF_8000_0000
    assert_eq!(state.regs.read(Reg::A2), 0xFFFF_FFFF_8000_0000u64);
}

#[test]
fn test_brev8() {
    let mut state = initialize_state([Rv64ZbkbInstruction::Brev8 {
        rd: Reg::A2,
        rs1: Reg::A0,
    }]);

    // Each byte has its bits reversed individually:
    // 0x01 -> 0x80, 0x02 -> 0x40, 0x03 -> 0xC0, 0x04 -> 0x20
    // 0x05 -> 0xA0, 0x06 -> 0x60, 0x07 -> 0xE0, 0x08 -> 0x10
    state.regs.write(Reg::A0, 0x0807_0605_0403_0201u64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0x10E0_60A0_20C0_4080u64);
}

#[test]
fn test_brev8_all_ones() {
    let mut state = initialize_state([Rv64ZbkbInstruction::Brev8 {
        rd: Reg::A2,
        rs1: Reg::A0,
    }]);

    // 0xFF reversed is 0xFF
    state.regs.write(Reg::A0, 0xFFFF_FFFF_FFFF_FFFFu64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xFFFF_FFFF_FFFF_FFFFu64);
}

#[test]
fn test_brev8_single_byte() {
    let mut state = initialize_state([Rv64ZbkbInstruction::Brev8 {
        rd: Reg::A2,
        rs1: Reg::A0,
    }]);

    // 0x01 = 0b00000001 reversed is 0b10000000 = 0x80
    state.regs.write(Reg::A0, 0x01u64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0x80u64);
}

#[test]
fn test_brev8_zero() {
    let mut state = initialize_state([Rv64ZbkbInstruction::Brev8 {
        rd: Reg::A2,
        rs1: Reg::A0,
    }]);

    state.regs.write(Reg::A0, 0u64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0u64);
}
