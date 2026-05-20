#![expect(clippy::identity_op, reason = "Test readability")]
#![expect(clippy::unusual_byte_groupings, reason = "Test readability")]

use crate::instructions::Instruction;
use crate::instructions::rv32::c::zca::Rv32ZcaInstruction;
use crate::instructions::utils::I24;
use crate::registers::general_purpose::{EReg, Reg};

/// Build a CIW (C.ADDI4SPN) encoding.
/// nzuimm\[5:4] = inst\[12:11], nzuimm\[9:6] = inst\[10:7], nzuimm\[2] = inst\[6], nzuimm\[3] =
/// inst\[5]
const fn make_addi4spn(rd_prime: u16, nzuimm: u16) -> u16 {
    let imm5_4 = (nzuimm >> 4u8) & 0b11;
    let imm9_6 = (nzuimm >> 6u8) & 0xf;
    let imm2 = (nzuimm >> 2u8) & 1;
    let imm3 = (nzuimm >> 3u8) & 1;
    (imm5_4 << 11u8) | (imm9_6 << 7u8) | (imm2 << 6u8) | (imm3 << 5u8) | (rd_prime << 2u8)
}

/// Build a Q00 CL/CS-type 16-bit instruction.
/// `funct3` = bits\[15:13], `rs1p` = bits\[9:7], `imm6_bits` = bits\[12:10],
/// `bit6` = bit\[6], `bit5` = bit\[5], `rd_rs2p` = bits\[4:2].
const fn make_cl_cs(
    funct3: u16,
    rs1p: u16,
    imm6_bits: u16,
    bit6: u16,
    bit5: u16,
    rd_rs2p: u16,
) -> u16 {
    (funct3 << 13u8)
        | (imm6_bits << 10u8)
        | (rs1p << 7u8)
        | (bit6 << 6u8)
        | (bit5 << 5u8)
        | (rd_rs2p << 2u8)
}

/// Build a CI-type Q01 instruction with 6-bit immediate.
/// imm\[5] = inst\[12], imm\[4:0] = inst\[6:2]
const fn make_ci_q01(funct3: u16, rd: u16, imm6: u16) -> u16 {
    let imm5 = (imm6 >> 5u8) & 1;
    let imm4_0 = imm6 & 0x1f;
    (funct3 << 13u8) | (imm5 << 12u8) | (rd << 7u8) | (imm4_0 << 2u8) | 0b01
}

/// Build a Q01 CJ-type instruction (C.JAL / C.J).
/// imm\[11]=inst\[12], imm\[4]=inst\[11], imm\[9:8]=inst\[10:9], imm\[10]=inst\[8],
/// imm\[6]=inst\[7], imm\[7]=inst\[6], imm\[3:1]=inst\[5:3], imm\[5]=inst\[2]
const fn make_cj(funct3: u16, imm: i16) -> u16 {
    let imm = imm.cast_unsigned();
    let imm11 = (imm >> 11u8) & 1;
    let imm4 = (imm >> 4u8) & 1;
    let imm9_8 = (imm >> 8u8) & 0b11;
    let imm10 = (imm >> 10u8) & 1;
    let imm6 = (imm >> 6u8) & 1;
    let imm7 = (imm >> 7u8) & 1;
    let imm3_1 = (imm >> 1u8) & 0b111;
    let imm5 = (imm >> 5u8) & 1;
    (funct3 << 13u8)
        | (imm11 << 12u8)
        | (imm4 << 11u8)
        | (imm9_8 << 9u8)
        | (imm10 << 8u8)
        | (imm6 << 7u8)
        | (imm7 << 6u8)
        | (imm3_1 << 3u8)
        | (imm5 << 2u8)
        | 0b01
}

/// Build a Q01 CB-type branch instruction (C.BEQZ / C.BNEZ).
/// imm\[8]=inst\[12], imm\[4:3]=inst\[11:10], imm\[7:6]=inst\[6:5],
/// imm\[2:1]=inst\[4:3], imm\[5]=inst\[2]
const fn make_cb_branch(funct3: u16, rs1p: u16, imm: i16) -> u16 {
    let imm = imm.cast_unsigned();
    let imm8 = (imm >> 8u8) & 1;
    let imm4_3 = (imm >> 3u8) & 0b11;
    let imm7_6 = (imm >> 6u8) & 0b11;
    let imm2_1 = (imm >> 1u8) & 0b11;
    let imm5 = (imm >> 5u8) & 1;
    (funct3 << 13u8)
        | (imm8 << 12u8)
        | (imm4_3 << 10u8)
        | (rs1p << 7u8)
        | (imm7_6 << 5u8)
        | (imm5 << 2u8)
        | (imm2_1 << 3u8)
        | 0b01
}

/// Build a Q01 CA-type arithmetic instruction (C.SUB/XOR/OR/AND).
const fn make_ca_arith(bit12: u16, rd_prime: u16, funct2b: u16, rs2_prime: u16) -> u16 {
    (0b100 << 13u8)
        | (bit12 << 12u8)
        | (0b11 << 10u8)
        | (rd_prime << 7u8)
        | (funct2b << 5u8)
        | (rs2_prime << 2u8)
        | 0b01
}

/// Build a Q10 CR-type instruction (C.JR/MV/JALR/ADD/EBREAK).
const fn make_cr_q10(funct3: u16, bit12: u16, rs1: u16, rs2: u16) -> u16 {
    (funct3 << 13u8) | (bit12 << 12u8) | (rs1 << 7u8) | (rs2 << 2u8) | 0b10
}

/// Build a Q10 C.SWSP instruction.
/// uimm[5:2]=inst[12:9], uimm[7:6]=inst[8:7]
const fn make_swsp(rs2: u16, uimm: u8) -> u16 {
    let uimm52 = ((uimm >> 2u8) & 0xf) as u16;
    let uimm76 = ((uimm >> 6u8) & 0b11) as u16;
    (0b110 << 13u8) | (uimm52 << 9u8) | (uimm76 << 7u8) | (rs2 << 2u8) | 0b10
}

// Quadrant 00

#[test]
fn test_caddi4spn_basic() {
    let inst = make_addi4spn(0, 4);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CAddi4spn {
            rd: Reg::S0,
            nzuimm: 4,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_caddi4spn_large_uimm() {
    // nzuimm=1020 (max: 255*4)
    let inst = make_addi4spn(1, 1020);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CAddi4spn {
            rd: Reg::S1,
            nzuimm: 1020,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_caddi4spn_nzuimm0_nonzero_rd_reserved() {
    // nzuimm=0 but rd'=1 (bits[4:2]=001): inst != 0, so reserved -> None
    // bits[15:13]=000, bits[12:5]=0, bits[4:2]=001, bits[1:0]=00 => 0x0004
    let inst = 0x0004;
    assert!(Rv32ZcaInstruction::<Reg<u32>>::try_decode(inst).is_none());
}

#[test]
fn test_clw_basic() {
    // uimm=4: uimm2=1, uimm5_3=0, uimm6=0
    let inst = make_cl_cs(0b010, 1, 0b000, 1, 0, 0);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CLw {
            rd: Reg::S0,
            rs1: Reg::S1,
            uimm: 4,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_clw_max_uimm() {
    // uimm=124: uimm6=1, uimm5_3=111, uimm2=1
    let inst = make_cl_cs(0b010, 0, 0b111, 1, 1, 0);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CLw {
            rd: Reg::S0,
            rs1: Reg::S0,
            uimm: 124,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_csw_basic() {
    // uimm=8: uimm5_3=001, uimm2=0, uimm6=0
    let inst = make_cl_cs(0b110, 0, 0b001, 0, 0, 1);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CSw {
            rs1: Reg::S0,
            rs2: Reg::S1,
            uimm: 8,
        }
    );
}

#[test]
fn test_csw_max_uimm() {
    // uimm=124: uimm6=1, uimm5_3=111, uimm2=1
    let inst = make_cl_cs(0b110, 0, 0b111, 1, 1, 1);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CSw {
            rs1: Reg::S0,
            rs2: Reg::S1,
            uimm: 124,
        }
    );
}

#[test]
fn test_q00_funct3_001_reserved() {
    // Zcb slot - must not decode as Zca
    let inst = (0b001 << 13u8) | 0b00;
    assert!(Rv32ZcaInstruction::<Reg<u32>>::try_decode(inst).is_none());
}

#[test]
fn test_q00_funct3_011_reserved_in_rv32() {
    // C.LD in RV64, reserved in RV32
    let inst = make_cl_cs(0b011, 0, 0b000, 0, 0, 0);
    assert!(Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).is_none());
}

#[test]
fn test_q00_funct3_100_reserved() {
    // Zcb slot - must not decode as Zca
    let inst = (0b100 << 13u8) | 0b00;
    assert!(Rv32ZcaInstruction::<Reg<u32>>::try_decode(inst).is_none());
}

#[test]
fn test_q00_funct3_111_reserved_in_rv32() {
    // C.SD in RV64, reserved in RV32
    let inst = make_cl_cs(0b111, 0, 0b000, 0, 0, 0);
    assert!(Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).is_none());
}

// Quadrant 01

#[test]
fn test_cnop() {
    let inst = 0b000_0_00000_00000_01;
    assert_eq!(
        Rv32ZcaInstruction::<Reg<u32>>::try_decode(inst).unwrap(),
        Rv32ZcaInstruction::CNop {
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_caddi_positive() {
    let inst = make_ci_q01(0b000, 10, 7);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CAddi {
            rd: Reg::A0,
            nzimm: 7,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_caddi_negative() {
    // imm6=0b11_1111 = -1 in 6-bit signed
    let inst = make_ci_q01(0b000, 10, 0b11_1111);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CAddi {
            rd: Reg::A0,
            nzimm: -1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_caddi_most_negative() {
    // imm6=0b10_0000 = -32 in 6-bit signed (minimum value)
    let inst = make_ci_q01(0b000, 10, 0b10_0000);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CAddi {
            rd: Reg::A0,
            nzimm: -32,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_caddi_hint_rd0() {
    // rd=0, nzimm≠0 is a hint - must be accepted
    let inst = make_ci_q01(0b000, 0, 5);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CAddi {
            rd: Reg::Zero,
            nzimm: 5,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_cjal_positive() {
    let inst = make_cj(0b001, 256);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CJal {
            imm: 256,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_cjal_negative() {
    let inst = make_cj(0b001, -128);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CJal {
            imm: -128,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_caddiw_does_not_exist_in_rv32() {
    // funct3=001 is C.JAL in RV32, not C.ADDIW
    let inst = make_cj(0b001, 4);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert!(matches!(decoded, Rv32ZcaInstruction::CJal { .. }));
}

#[test]
fn test_cli() {
    let inst = make_ci_q01(0b010, 10, 5);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CLi {
            rd: Reg::A0,
            imm: 5,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_cli_negative() {
    // imm6=0b11_1000 = -8 in 6-bit signed
    let inst = make_ci_q01(0b010, 10, 0b11_1000);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CLi {
            rd: Reg::A0,
            imm: -8,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_cli_most_negative() {
    // imm6=0b10_0000 = -32 in 6-bit signed (minimum value)
    let inst = make_ci_q01(0b010, 10, 0b10_0000);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CLi {
            rd: Reg::A0,
            imm: -32,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_cli_hint_rd0() {
    // rd=0 is a hint - must be accepted
    let inst = make_ci_q01(0b010, 0, 3);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CLi {
            rd: Reg::Zero,
            imm: 3,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_caddi16sp() {
    // nzimm=16: imm4=1, others=0
    let inst = (0b011 << 13u8) | (0 << 12u8) | (2 << 7u8) | (1 << 6u8) | 0b01;
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CAddi16sp {
            nzimm: 16,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_caddi16sp_negative() {
    // nzimm=-16: imm9=1, imm8_7=11, imm6=1, imm5=1, imm4=1
    let inst = (0b011 << 13u8)
        | (1 << 12u8)
        | (2 << 7u8)
        | (1 << 6u8)
        | (1 << 5u8)
        | (0b11 << 3u8)
        | (1 << 2u8)
        | 0b01;
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CAddi16sp {
            nzimm: -16,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_caddi16sp_reserved_zero() {
    let inst = (0b011 << 13u8) | (2 << 7u8) | 0b01;
    assert!(Rv32ZcaInstruction::<Reg<u32>>::try_decode(inst).is_none());
}

#[test]
fn test_clui() {
    let inst = (0b011 << 13u8) | (0 << 12u8) | (10 << 7u8) | (1 << 2u8) | 0b01;
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CLui {
            rd: Reg::A0,
            nzimm: I24::from_i32(0x1000),
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_clui_negative() {
    // imm17=1, imm16:12=1_1111 -> sign-extended = -4096
    let inst = (0b011 << 13u8) | (1 << 12u8) | (10 << 7u8) | (0b1_1111 << 2u8) | 0b01;
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CLui {
            rd: Reg::A0,
            nzimm: I24::from_i32(-4096),
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_clui_reserved_zero() {
    let inst = (0b011 << 13u8) | (0 << 12u8) | (10 << 7u8) | (0 << 2u8) | 0b01;
    assert!(Rv32ZcaInstruction::<Reg<u32>>::try_decode(inst).is_none());
}

#[test]
fn test_clui_hint_rd0_positive() {
    // rd=0, nzimm>0 is a HINT - must be accepted
    let inst = (0b011 << 13u8) | (0 << 12u8) | (0 << 7u8) | (1 << 2u8) | 0b01;
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CLui {
            rd: Reg::Zero,
            nzimm: I24::from_i32(0x1000),
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_clui_hint_rd0_negative() {
    // rd=0, nzimm<0 is a HINT - must be accepted
    let inst = (0b011 << 13u8) | (1 << 12u8) | (0 << 7u8) | (0b11111 << 2u8) | 0b01;
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CLui {
            rd: Reg::Zero,
            nzimm: I24::from_i32(-4096),
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_csrli_basic() {
    let inst = (0b100 << 13u8) | (0 << 12u8) | (0b00 << 10u8) | (0 << 7u8) | (4 << 2u8) | 0b01;
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CSrli {
            rd: Reg::S0,
            shamt: 4,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_csrli_hint_shamt0() {
    // shamt=0 is a hint - must be accepted
    let inst = (0b100 << 13u8) | (0 << 12u8) | (0b00 << 10u8) | (0 << 7u8) | (0 << 2u8) | 0b01;
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CSrli {
            rd: Reg::S0,
            shamt: 0,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_csrli_shamt5_reserved_in_rv32() {
    // inst[12]=1 means shamt[5]=1, NSE in RV32
    let inst = (0b100 << 13u8) | (1 << 12u8) | (0b00 << 10u8) | (0 << 7u8) | (4 << 2u8) | 0b01;
    assert!(Rv32ZcaInstruction::<Reg<u32>>::try_decode(inst).is_none());
}

#[test]
fn test_csrai_basic() {
    let inst = (0b100 << 13u8) | (0 << 12u8) | (0b01 << 10u8) | (0 << 7u8) | (8 << 2u8) | 0b01;
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CSrai {
            rd: Reg::S0,
            shamt: 8,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_csrai_hint_shamt0() {
    // shamt=0 is a hint - must be accepted
    let inst = (0b100 << 13u8) | (0 << 12u8) | (0b01 << 10u8) | (0 << 7u8) | (0 << 2u8) | 0b01;
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CSrai {
            rd: Reg::S0,
            shamt: 0,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_csrai_shamt5_reserved_in_rv32() {
    let inst = (0b100 << 13u8) | (1 << 12u8) | (0b01 << 10u8) | (0 << 7u8) | (4 << 2u8) | 0b01;
    assert!(Rv32ZcaInstruction::<Reg<u32>>::try_decode(inst).is_none());
}

#[test]
fn test_candi() {
    let inst =
        (0b100 << 13u8) | (1 << 12u8) | (0b10 << 10u8) | (0 << 7u8) | (0b1_1111 << 2u8) | 0b01;
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CAndi {
            rd: Reg::S0,
            imm: -1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_candi_most_negative() {
    // imm6=0b100000 = -32 in 6-bit signed (minimum value)
    let inst =
        (0b100 << 13u8) | (1 << 12u8) | (0b10 << 10u8) | (0 << 7u8) | (0b0_0000 << 2u8) | 0b01;
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CAndi {
            rd: Reg::S0,
            imm: -32,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_csub() {
    let inst = make_ca_arith(0, 0, 0b00, 1);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CSub {
            rd: Reg::S0,
            rs2: Reg::S1,
            rs1: Reg::Zero,
        }
    );
}

#[test]
fn test_cxor() {
    let inst = make_ca_arith(0, 0, 0b01, 1);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CXor {
            rd: Reg::S0,
            rs2: Reg::S1,
            rs1: Reg::Zero,
        }
    );
}

#[test]
fn test_cor() {
    let inst = make_ca_arith(0, 0, 0b10, 1);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::COr {
            rd: Reg::S0,
            rs2: Reg::S1,
            rs1: Reg::Zero,
        }
    );
}

#[test]
fn test_cand() {
    let inst = make_ca_arith(0, 0, 0b11, 1);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CAnd {
            rd: Reg::S0,
            rs2: Reg::S1,
            rs1: Reg::Zero,
        }
    );
}

#[test]
fn test_csubw_reserved_in_rv32() {
    // bit12=1 CA arithmetic does not exist in RV32
    let inst = make_ca_arith(1, 0, 0b00, 1);
    assert!(Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).is_none());
}

#[test]
fn test_caddw_reserved_in_rv32() {
    let inst = make_ca_arith(1, 0, 0b01, 1);
    assert!(Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).is_none());
}

#[test]
fn test_cj_positive() {
    let inst = make_cj(0b101, 256);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CJ {
            imm: 256,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_cj_negative() {
    let inst = make_cj(0b101, -64);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CJ {
            imm: -64,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_cbeqz_positive() {
    let inst = make_cb_branch(0b110, 0, 8);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CBeqz {
            rs1: Reg::S0,
            imm: 8,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_cbeqz_negative() {
    let inst = make_cb_branch(0b110, 0, -8);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CBeqz {
            rs1: Reg::S0,
            imm: -8,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_cbnez_positive() {
    let inst = make_cb_branch(0b111, 1, 16);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CBnez {
            rs1: Reg::S1,
            imm: 16,
            rs2: Reg::Zero,
        }
    );
}

// Quadrant 10

#[test]
fn test_cslli_basic() {
    let inst = (0b000 << 13u8) | (0 << 12u8) | (10 << 7u8) | (3 << 2u8) | 0b10;
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CSlli {
            rd: Reg::A0,
            shamt: 3,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_cslli_hint_shamt0() {
    // shamt=0, rd≠0: hint - must be accepted
    let inst = (0b000 << 13u8) | (0 << 12u8) | (10 << 7u8) | (0 << 2u8) | 0b10;
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CSlli {
            rd: Reg::A0,
            shamt: 0,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_cslli_hint_rd0() {
    // rd=0, shamt≠0: hint - must be accepted
    let inst = (0b000 << 13u8) | (0 << 12u8) | (0 << 7u8) | (3 << 2u8) | 0b10;
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CSlli {
            rd: Reg::Zero,
            shamt: 3,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_cslli_shamt5_reserved_in_rv32() {
    // inst[12]=1 means shamt[5]=1, NSE in RV32
    let inst = (0b000 << 13u8) | (1 << 12u8) | (10 << 7u8) | (3 << 2u8) | 0b10;
    assert!(Rv32ZcaInstruction::<Reg<u32>>::try_decode(inst).is_none());
}

#[test]
fn test_clwsp_basic() {
    // uimm=4: uimm5=0, uimm4:2=001, uimm7:6=00
    let inst = (0b010 << 13u8) | (0 << 12u8) | (10 << 7u8) | (0b001 << 4u8) | (0 << 2u8) | 0b10;
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CLwsp {
            rd: Reg::A0,
            uimm: 4,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_clwsp_max_uimm() {
    // uimm=252: uimm5=1, uimm4:2=111, uimm7:6=11
    let inst = (0b010 << 13u8) | (1 << 12u8) | (10 << 7u8) | (0b111 << 4u8) | (0b11 << 2u8) | 0b10;
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CLwsp {
            rd: Reg::A0,
            uimm: 252,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_clwsp_reserved_rd0() {
    let inst = (0b010 << 13u8) | (0 << 12u8) | (0 << 7u8) | (1 << 4u8) | 0b10;
    assert!(Rv32ZcaInstruction::<Reg<u32>>::try_decode(inst).is_none());
}

#[test]
fn test_q10_funct3_001_reserved() {
    // C.FLWSP (Zcf) - not in Zca
    let inst = (0b001 << 13u8) | (0 << 12u8) | (10 << 7u8) | (1 << 4u8) | 0b10;
    assert!(Rv32ZcaInstruction::<Reg<u32>>::try_decode(inst).is_none());
}

#[test]
fn test_q10_funct3_011_reserved() {
    // C.FLDSP (Zcd) - not in Zca; also C.LDSP slot in RV64
    let inst = (0b011 << 13u8) | (0 << 12u8) | (10 << 7u8) | (1 << 4u8) | 0b10;
    assert!(Rv32ZcaInstruction::<Reg<u32>>::try_decode(inst).is_none());
}

#[test]
fn test_cjr() {
    let inst = make_cr_q10(0b100, 0, 10, 0);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CJr {
            rs1: Reg::A0,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_cjr_reserved_rs1_0() {
    let inst = make_cr_q10(0b100, 0, 0, 0);
    assert!(Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).is_none());
}

#[test]
fn test_cmv() {
    let inst = make_cr_q10(0b100, 0, 10, 11);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CMv {
            rd: Reg::A0,
            rs2: Reg::A1,
            rs1: Reg::Zero,
        }
    );
}

#[test]
fn test_cmv_hint_rd0() {
    // rd=0, rs2≠0: hint - must be accepted
    let inst = make_cr_q10(0b100, 0, 0, 11);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CMv {
            rd: Reg::Zero,
            rs2: Reg::A1,
            rs1: Reg::Zero,
        }
    );
}

#[test]
fn test_cmv_rs2_0_decodes_as_cjr() {
    // rs2=0 with bit12=0 is C.JR, not C.MV
    let inst = make_cr_q10(0b100, 0, 10, 0);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert!(matches!(decoded, Rv32ZcaInstruction::CJr { .. }));
}

#[test]
fn test_cebreak() {
    let inst = make_cr_q10(0b100, 1, 0, 0);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CEbreak {
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_cjalr() {
    let inst = make_cr_q10(0b100, 1, 10, 0);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CJalr {
            rs1: Reg::A0,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_cadd() {
    let inst = make_cr_q10(0b100, 1, 10, 11);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CAdd {
            rd: Reg::A0,
            rs2: Reg::A1,
            rs1: Reg::Zero,
        }
    );
}

#[test]
fn test_cadd_hint_rd0() {
    // rd=0, rs2≠0: hint - must be accepted
    let inst = make_cr_q10(0b100, 1, 0, 11);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CAdd {
            rd: Reg::Zero,
            rs2: Reg::A1,
            rs1: Reg::Zero,
        }
    );
}

#[test]
fn test_cswsp_basic() {
    let inst = make_swsp(10, 4);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CSwsp {
            rs2: Reg::A0,
            uimm: 4,
            rs1: Reg::Zero,
        }
    );
}

#[test]
fn test_cswsp_max_uimm() {
    // uimm=252: uimm[7:6]=11, uimm[5:2]=1111
    let inst = make_swsp(10, 252);
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CSwsp {
            rs2: Reg::A0,
            uimm: 252,
            rs1: Reg::Zero,
        }
    );
}

#[test]
fn test_q10_funct3_111_reserved() {
    // C.FSWSP (Zcf) - not in Zca
    let inst = (0b111 << 13u8) | (0 << 12u8) | (10 << 7u8) | 0b10;
    assert!(Rv32ZcaInstruction::<Reg<u32>>::try_decode(inst).is_none());
}

// Unimplemented/illegal

#[test]
fn test_cunimp() {
    let inst = 0;
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CUnimp {
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_cunimp_ignores_upper_16_bits() {
    let inst = 0xABCD_0000;
    let decoded = Rv32ZcaInstruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CUnimp {
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

// Invalid / Reserved

#[test]
fn test_quadrant_11_invalid() {
    let inst = 0x0000_0033;
    assert!(Rv32ZcaInstruction::<Reg<u32>>::try_decode(inst).is_none());
}

// RV64E variant

#[test]
fn test_ereg_clw_valid() {
    // C.LW with prime registers (x8-x15) - valid for RV64E
    let inst = make_cl_cs(0b010, 0, 0b000, 1, 0, 0);
    let decoded = Rv32ZcaInstruction::<EReg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcaInstruction::CLw {
            rd: EReg::S0,
            rs1: EReg::S0,
            uimm: 4,
            rs2: EReg::Zero,
        }
    );
}

#[test]
fn test_ereg_cslli_invalid_high_reg() {
    // C.SLLI rd=x16 - x16 does not exist in EReg, from_bits must fail
    let inst = (0b000 << 13u8) | (0 << 12u8) | (16 << 7u8) | (3 << 2u8) | 0b10;
    assert!(Rv32ZcaInstruction::<EReg<u32>>::try_decode(inst).is_none());
}
