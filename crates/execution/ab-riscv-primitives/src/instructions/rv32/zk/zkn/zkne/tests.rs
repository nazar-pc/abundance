use crate::instructions::Instruction;
use crate::instructions::rv32::zk::zkn::zknd::Rv32AesBs;
use crate::instructions::rv32::zk::zkn::zkne::Rv32ZkneInstruction;
use crate::registers::general_purpose::Reg;

const FUNCT5_ESI: u32 = 0b10001;
const FUNCT5_ESMI: u32 = 0b10011;

fn make_rv32_zkne(funct5: u32, rd: u32, rs1: u32, rs2: u32, bs: u32) -> u32 {
    (bs << 30) | (funct5 << 25) | (rs2 << 20) | (rs1 << 15) | (rd << 7) | 0b0110011
}

// aes32esi

#[test]
fn test_aes32esi_bs0() {
    let inst = make_rv32_zkne(FUNCT5_ESI, 1, 1, 2, 0);
    let decoded = Rv32ZkneInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZkneInstruction::Aes32Esi {
            rd: Reg::Ra,
            rs1: Reg::Ra,
            rs2: Reg::Sp,
            bs: Rv32AesBs::B0,
        })
    );
}

#[test]
fn test_aes32esi_bs1() {
    let inst = make_rv32_zkne(FUNCT5_ESI, 3, 3, 4, 1);
    let decoded = Rv32ZkneInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZkneInstruction::Aes32Esi {
            rd: Reg::Gp,
            rs1: Reg::Gp,
            rs2: Reg::Tp,
            bs: Rv32AesBs::B1,
        })
    );
}

#[test]
fn test_aes32esi_bs2() {
    let inst = make_rv32_zkne(FUNCT5_ESI, 5, 5, 6, 2);
    let decoded = Rv32ZkneInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZkneInstruction::Aes32Esi {
            rd: Reg::T0,
            rs1: Reg::T0,
            rs2: Reg::T1,
            bs: Rv32AesBs::B2,
        })
    );
}

#[test]
fn test_aes32esi_bs3() {
    let inst = make_rv32_zkne(FUNCT5_ESI, 7, 7, 8, 3);
    let decoded = Rv32ZkneInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZkneInstruction::Aes32Esi {
            rd: Reg::T2,
            rs1: Reg::T2,
            rs2: Reg::S0,
            bs: Rv32AesBs::B3,
        })
    );
}

// aes32esmi

#[test]
fn test_aes32esmi_bs0() {
    let inst = make_rv32_zkne(FUNCT5_ESMI, 1, 1, 2, 0);
    let decoded = Rv32ZkneInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZkneInstruction::Aes32Esmi {
            rd: Reg::Ra,
            rs1: Reg::Ra,
            rs2: Reg::Sp,
            bs: Rv32AesBs::B0,
        })
    );
}

#[test]
fn test_aes32esmi_bs3() {
    let inst = make_rv32_zkne(FUNCT5_ESMI, 9, 9, 10, 3);
    let decoded = Rv32ZkneInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZkneInstruction::Aes32Esmi {
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
    let inst =
        (FUNCT5_ESI << 25) | (2u32 << 20) | (1u32 << 15) | (0b001 << 12) | (1u32 << 7) | 0b0110011;
    let decoded = Rv32ZkneInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_wrong_opcode_rejected() {
    // opcode = 0b0010011 (OP-IMM) instead of 0b0110011 (OP)
    let inst = (FUNCT5_ESI << 25) | (2u32 << 20) | (1u32 << 15) | (1u32 << 7) | 0b0010011;
    let decoded = Rv32ZkneInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_aes32esi_and_aes32esmi_distinct_funct5() {
    // Same registers and bs, different funct5 → different variants
    let inst_esi = make_rv32_zkne(FUNCT5_ESI, 1, 1, 2, 1);
    let inst_esmi = make_rv32_zkne(FUNCT5_ESMI, 1, 1, 2, 1);
    let dec_esi = Rv32ZkneInstruction::<Reg<u32>>::try_decode(inst_esi);
    let dec_esmi = Rv32ZkneInstruction::<Reg<u32>>::try_decode(inst_esmi);
    assert!(matches!(
        dec_esi,
        Some(Rv32ZkneInstruction::Aes32Esi { .. })
    ));
    assert!(matches!(
        dec_esmi,
        Some(Rv32ZkneInstruction::Aes32Esmi { .. })
    ));
}

#[test]
fn test_unknown_funct5_rejected() {
    // funct5 = 0b10010: not aes32esi (0b10001) or aes32esmi (0b10011)
    let inst = make_rv32_zkne(0b10010, 1, 1, 2, 0);
    let decoded = Rv32ZkneInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_all_bs_values_decode_for_aes32esmi() {
    for bs in 0u32..=3 {
        let inst = make_rv32_zkne(FUNCT5_ESMI, 1, 1, 2, bs);
        let decoded = Rv32ZkneInstruction::<Reg<u32>>::try_decode(inst);
        let expected_bs = Rv32AesBs::from_bits(bs as u8).unwrap();
        assert_eq!(
            decoded,
            Some(Rv32ZkneInstruction::Aes32Esmi {
                rd: Reg::Ra,
                rs1: Reg::Ra,
                rs2: Reg::Sp,
                bs: expected_bs,
            }),
            "bs={bs} failed"
        );
    }
}
