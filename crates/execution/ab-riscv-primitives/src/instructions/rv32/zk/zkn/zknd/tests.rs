use crate::instructions::Instruction;
use crate::instructions::rv32::zk::zkn::zknd::{Rv32AesBs, Rv32ZkndInstruction};
use crate::registers::general_purpose::Reg;

const FUNCT5_DSI: u32 = 0b10101;
const FUNCT5_DSMI: u32 = 0b10111;

fn make_rv32_zknd(funct5: u32, rd: u32, rs1: u32, rs2: u32, bs: u32) -> u32 {
    (bs << 30u8) | (funct5 << 25u8) | (rs2 << 20u8) | (rs1 << 15u8) | (rd << 7u8) | 0b011_0011
}

// aes32dsi

#[test]
fn test_aes32dsi_bs0() {
    let inst = make_rv32_zknd(FUNCT5_DSI, 1, 1, 2, 0);
    let decoded = Rv32ZkndInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZkndInstruction::Aes32Dsi {
            rd: Reg::Ra,
            rs1: Reg::Ra,
            rs2: Reg::Sp,
            bs: Rv32AesBs::B0,
        })
    );
}

#[test]
fn test_aes32dsi_bs1() {
    let inst = make_rv32_zknd(FUNCT5_DSI, 3, 3, 4, 1);
    let decoded = Rv32ZkndInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZkndInstruction::Aes32Dsi {
            rd: Reg::Gp,
            rs1: Reg::Gp,
            rs2: Reg::Tp,
            bs: Rv32AesBs::B1,
        })
    );
}

#[test]
fn test_aes32dsi_bs2() {
    let inst = make_rv32_zknd(FUNCT5_DSI, 5, 5, 6, 2);
    let decoded = Rv32ZkndInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZkndInstruction::Aes32Dsi {
            rd: Reg::T0,
            rs1: Reg::T0,
            rs2: Reg::T1,
            bs: Rv32AesBs::B2,
        })
    );
}

#[test]
fn test_aes32dsi_bs3() {
    let inst = make_rv32_zknd(FUNCT5_DSI, 7, 7, 8, 3);
    let decoded = Rv32ZkndInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZkndInstruction::Aes32Dsi {
            rd: Reg::T2,
            rs1: Reg::T2,
            rs2: Reg::S0,
            bs: Rv32AesBs::B3,
        })
    );
}

// aes32dsmi

#[test]
fn test_aes32dsmi_bs0() {
    let inst = make_rv32_zknd(FUNCT5_DSMI, 1, 1, 2, 0);
    let decoded = Rv32ZkndInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZkndInstruction::Aes32Dsmi {
            rd: Reg::Ra,
            rs1: Reg::Ra,
            rs2: Reg::Sp,
            bs: Rv32AesBs::B0,
        })
    );
}

#[test]
fn test_aes32dsmi_bs3() {
    let inst = make_rv32_zknd(FUNCT5_DSMI, 9, 9, 10, 3);
    let decoded = Rv32ZkndInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZkndInstruction::Aes32Dsmi {
            rd: Reg::S1,
            rs1: Reg::S1,
            rs2: Reg::A0,
            bs: Rv32AesBs::B3,
        })
    );
}
// rejection cases

#[test]
fn test_wrong_funct3_rejected() {
    // funct3 = 0b001 instead of 0b000
    let inst = (FUNCT5_DSI << 25u8)
        | (2 << 20u8)
        | (1 << 15u8)
        | (0b001 << 12u8)
        | (1 << 7u8)
        | 0b011_0011;
    let decoded = Rv32ZkndInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_wrong_opcode_rejected() {
    // opcode = 0b001_0011 (OP-IMM) instead of 0b011_0011 (OP)
    let inst = (FUNCT5_DSI << 25u8) | (2 << 20u8) | (1 << 15u8) | (1 << 7u8) | 0b001_0011;
    let decoded = Rv32ZkndInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_aes32dsi_and_aes32dsmi_distinct_funct5() {
    // Same registers and bs, different funct5 -> different variants
    let inst_dsi = make_rv32_zknd(FUNCT5_DSI, 1, 1, 2, 1);
    let inst_dsmi = make_rv32_zknd(FUNCT5_DSMI, 1, 1, 2, 1);
    let dec_dsi = Rv32ZkndInstruction::<Reg<u32>>::try_decode(inst_dsi);
    let dec_dsmi = Rv32ZkndInstruction::<Reg<u32>>::try_decode(inst_dsmi);
    assert!(matches!(
        dec_dsi,
        Some(Rv32ZkndInstruction::Aes32Dsi { .. })
    ));
    assert!(matches!(
        dec_dsmi,
        Some(Rv32ZkndInstruction::Aes32Dsmi { .. })
    ));
}

#[test]
fn test_unknown_funct5_rejected() {
    // funct5 = 0b1_1110: not aes32dsi (0b1_0101) or aes32dsmi (0b1_0111)
    let inst = make_rv32_zknd(0b1_1110, 1, 1, 2, 0);
    let decoded = Rv32ZkndInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_all_bs_values_decode_for_aes32dsmi() {
    for bs in 0u32..=3 {
        let inst = make_rv32_zknd(FUNCT5_DSMI, 1, 1, 2, bs);
        let decoded = Rv32ZkndInstruction::<Reg<u32>>::try_decode(inst);
        let expected_bs = Rv32AesBs::from_bits(bs as u8).unwrap();
        assert_eq!(
            decoded,
            Some(Rv32ZkndInstruction::Aes32Dsmi {
                rd: Reg::Ra,
                rs1: Reg::Ra,
                rs2: Reg::Sp,
                bs: expected_bs,
            }),
            "bs={bs} failed"
        );
    }
}
