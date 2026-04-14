#![expect(clippy::identity_op, reason = "Test readability")]
#![expect(clippy::unusual_byte_groupings, reason = "Test readability")]

use crate::instructions::Instruction;
use crate::instructions::rv64::c::zca::Rv64ZcaInstruction;
use crate::registers::general_purpose::{EReg, Reg};

/// Build a CIW (C.ADDI4SPN) encoding.
/// nzuimm\[5:4] = inst\[12:11], nzuimm\[9:6] = inst\[10:7], nzuimm\[2] = inst\[6],
/// nzuimm\[3] = inst\[5]
const fn make_addi4spn(rd_prime: u16, nzuimm: u16) -> u16 {
    let imm5_4 = (nzuimm >> 4) & 0b11;
    let imm9_6 = (nzuimm >> 6) & 0xf;
    let imm2 = (nzuimm >> 2) & 1;
    let imm3 = (nzuimm >> 3) & 1;
    (imm5_4 << 11) | (imm9_6 << 7) | (imm2 << 6) | (imm3 << 5) | (rd_prime << 2)
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
    (funct3 << 13) | (imm6_bits << 10) | (rs1p << 7) | (bit6 << 6) | (bit5 << 5) | (rd_rs2p << 2)
}

/// Build a CI-type Q01 instruction with 6-bit immediate.
/// imm\[5] = inst\[12], imm\[4:0] = inst\[6:2]
const fn make_ci_q01(funct3: u16, rd: u16, imm6: u16) -> u16 {
    let imm5 = (imm6 >> 5) & 1;
    let imm4_0 = imm6 & 0x1f;
    (funct3 << 13) | (imm5 << 12) | (rd << 7) | (imm4_0 << 2) | 0b01
}

/// Build a Q01 CJ-type instruction (C.J).
/// imm\[11]=inst\[12], imm\[4]=inst\[11], imm\[9:8]=inst\[10:9], imm\[10]=inst\[8],
/// imm\[6]=inst\[7], imm\[7]=inst\[6], imm\[3:1]=inst\[5:3], imm\[5]=inst\[2]
const fn make_cj(imm: i16) -> u16 {
    let imm = imm.cast_unsigned();
    let imm11 = (imm >> 11) & 1;
    let imm4 = (imm >> 4) & 1;
    let imm9_8 = (imm >> 8) & 0b11;
    let imm10 = (imm >> 10) & 1;
    let imm6 = (imm >> 6) & 1;
    let imm7 = (imm >> 7) & 1;
    let imm3_1 = (imm >> 1) & 0b111;
    let imm5 = (imm >> 5) & 1;
    (0b101 << 13)
        | (imm11 << 12)
        | (imm4 << 11)
        | (imm9_8 << 9)
        | (imm10 << 8)
        | (imm6 << 7)
        | (imm7 << 6)
        | (imm3_1 << 3)
        | (imm5 << 2)
        | 0b01
}

/// Build a Q01 CB-type branch instruction (C.BEQZ / C.BNEZ).
/// imm\[8]=inst\[12], imm\[4:3]=inst\[11:10], imm\[7:6]=inst\[6:5],
/// imm\[2:1]=inst\[4:3], imm\[5]=inst\[2]
const fn make_cb_branch(funct3: u16, rs1p: u16, imm: i16) -> u16 {
    let imm = imm.cast_unsigned();
    let imm8 = (imm >> 8) & 1;
    let imm4_3 = (imm >> 3) & 0b11;
    let imm7_6 = (imm >> 6) & 0b11;
    let imm2_1 = (imm >> 1) & 0b11;
    let imm5 = (imm >> 5) & 1;
    (funct3 << 13)
        | (imm8 << 12)
        | (imm4_3 << 10)
        | (rs1p << 7)
        | (imm7_6 << 5)
        | (imm5 << 2)
        | (imm2_1 << 3)
        | 0b01
}

/// Build a Q01 CA-type arithmetic instruction (C.SUB/XOR/OR/AND/SUBW/ADDW).
const fn make_ca_arith(bit12: u16, rd_prime: u16, funct2b: u16, rs2_prime: u16) -> u16 {
    (0b100 << 13)
        | (bit12 << 12)
        | (0b11 << 10)
        | (rd_prime << 7)
        | (funct2b << 5)
        | (rs2_prime << 2)
        | 0b01
}

/// Build a Q10 CR-type instruction (C.JR/MV/JALR/ADD/EBREAK).
const fn make_cr_q10(funct3: u16, bit12: u16, rs1: u16, rs2: u16) -> u16 {
    (funct3 << 13) | (bit12 << 12) | (rs1 << 7) | (rs2 << 2) | 0b10
}

/// Build a Q10 C.SWSP instruction.
/// uimm\[5:2]=inst\[12:9], uimm\[7:6]=inst\[8:7]
const fn make_swsp(rs2: u16, uimm: u8) -> u16 {
    let uimm52 = ((uimm >> 2) & 0xf) as u16;
    let uimm76 = ((uimm >> 6) & 0b11) as u16;
    (0b110 << 13) | (uimm52 << 9) | (uimm76 << 7) | (rs2 << 2) | 0b10
}

/// Build a Q10 C.SDSP instruction.
/// uimm\[5:3]=inst\[12:10], uimm\[8:6]=inst\[9:7]
const fn make_sdsp(rs2: u16, uimm: u16) -> u16 {
    let uimm53 = (uimm >> 3) & 0b111;
    let uimm86 = (uimm >> 6) & 0b111;
    (0b111 << 13) | (uimm53 << 10) | (uimm86 << 7) | (rs2 << 2) | 0b10
}

// Quadrant 00

#[test]
fn test_caddi4spn_basic() {
    // rd'=s0 (prime 0 -> x8), nzuimm=4
    let inst = make_addi4spn(0, 4);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CAddi4spn {
            rd: Reg::S0,
            nzuimm: 4
        }
    );
}

#[test]
fn test_caddi4spn_large_uimm() {
    // nzuimm=1020 (max: 255*4)
    let inst = make_addi4spn(1, 1020);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CAddi4spn {
            rd: Reg::S1,
            nzuimm: 1020
        }
    );
}

#[test]
fn test_caddi4spn_reserved_zero() {
    let inst = make_addi4spn(0, 0);
    assert!(Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).is_none());
}

#[test]
fn test_clw_basic() {
    // rd'=s0, rs1'=s1, uimm=4: uimm[5:3]=0, uimm[2]=1, uimm[6]=0
    let inst = make_cl_cs(0b010, 1, 0b000, 1, 0, 0);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CLw {
            rd: Reg::S0,
            rs1: Reg::S1,
            uimm: 4
        }
    );
}

#[test]
fn test_clw_max_uimm() {
    // uimm=124: uimm[6]=1, uimm[5:3]=111, uimm[2]=1 -> 64+56+4=124
    let inst = make_cl_cs(0b010, 0, 0b111, 1, 1, 0);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CLw {
            rd: Reg::S0,
            rs1: Reg::S0,
            uimm: 124
        }
    );
}

#[test]
fn test_cld_basic() {
    // rd'=s0, rs1'=s0, uimm=8: uimm[5:3]=001, uimm[7:6]=00
    let inst = make_cl_cs(0b011, 0, 0b001, 0, 0, 0);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CLd {
            rd: Reg::S0,
            rs1: Reg::S0,
            uimm: 8
        }
    );
}

#[test]
fn test_cld_max_uimm() {
    // uimm=248: uimm[7:6]=11, uimm[5:3]=111
    let inst = make_cl_cs(0b011, 0, 0b111, 1, 1, 0);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CLd {
            rd: Reg::S0,
            rs1: Reg::S0,
            uimm: 248
        }
    );
}

#[test]
fn test_csw_basic() {
    // rs1'=s0, rs2'=s1, uimm=4
    let inst = make_cl_cs(0b110, 0, 0b000, 1, 0, 1);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CSw {
            rs1: Reg::S0,
            rs2: Reg::S1,
            uimm: 4
        }
    );
}

#[test]
fn test_csd_basic() {
    // rs1'=s0, rs2'=s1, uimm=24: uimm[7:6]=00, uimm[5:3]=011
    let inst = make_cl_cs(0b111, 0, 0b011, 0, 0, 1);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CSd {
            rs1: Reg::S0,
            rs2: Reg::S1,
            uimm: 24
        }
    );
}

#[test]
fn test_q00_funct3_001_reserved() {
    // Zcb slot — must not decode as Zca
    let inst = (0b001 << 13) | 0b00;
    assert!(Rv64ZcaInstruction::<Reg<u64>>::try_decode(inst).is_none());
}

#[test]
fn test_q00_funct3_100_reserved() {
    // Zcb slot — must not decode as Zca
    let inst = (0b100 << 13) | 0b00;
    assert!(Rv64ZcaInstruction::<Reg<u64>>::try_decode(inst).is_none());
}

// Quadrant 01

#[test]
fn test_cnop() {
    let inst = 0b000_0_00000_00000_01;
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(decoded, Rv64ZcaInstruction::CNop);
}

#[test]
fn test_caddi_positive() {
    let inst = make_ci_q01(0b000, 10, 5);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CAddi {
            rd: Reg::A0,
            nzimm: 5
        }
    );
}

#[test]
fn test_caddi_negative() {
    // imm6=0b111111 = -1 in 6-bit signed
    let inst = make_ci_q01(0b000, 10, 0b111111);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CAddi {
            rd: Reg::A0,
            nzimm: -1
        }
    );
}

#[test]
fn test_caddi_most_negative() {
    // imm6=0b100000 = -32 in 6-bit signed (minimum value)
    let inst = make_ci_q01(0b000, 10, 0b100000);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CAddi {
            rd: Reg::A0,
            nzimm: -32
        }
    );
}

#[test]
fn test_caddi_hint_rd0() {
    // rd=0, nzimm≠0 is a hint — must be accepted
    let inst = make_ci_q01(0b000, 0, 5);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CAddi {
            rd: Reg::Zero,
            nzimm: 5
        }
    );
}

#[test]
fn test_caddiw_positive() {
    let inst = make_ci_q01(0b001, 10, 10);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CAddiw {
            rd: Reg::A0,
            imm: 10
        }
    );
}

#[test]
fn test_caddiw_negative() {
    // imm6=0b111110 = -2 in 6-bit signed
    let inst = make_ci_q01(0b001, 10, 0b111110);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CAddiw {
            rd: Reg::A0,
            imm: -2
        }
    );
}

#[test]
fn test_caddiw_reserved_rd0() {
    let inst = make_ci_q01(0b001, 0, 5);
    assert!(Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).is_none());
}

#[test]
fn test_cli_basic() {
    let inst = make_ci_q01(0b010, 10, 7);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CLi {
            rd: Reg::A0,
            imm: 7
        }
    );
}

#[test]
fn test_cli_negative() {
    // imm6=0b111000 = -8 in 6-bit signed
    let inst = make_ci_q01(0b010, 10, 0b111000);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CLi {
            rd: Reg::A0,
            imm: -8
        }
    );
}

#[test]
fn test_cli_most_negative() {
    // imm6=0b100000 = -32 in 6-bit signed (minimum value)
    let inst = make_ci_q01(0b010, 10, 0b100000);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CLi {
            rd: Reg::A0,
            imm: -32
        }
    );
}

#[test]
fn test_cli_hint_rd0() {
    // rd=0 is a hint — must be accepted
    let inst = make_ci_q01(0b010, 0, 3);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CLi {
            rd: Reg::Zero,
            imm: 3
        }
    );
}

#[test]
fn test_caddi16sp_positive() {
    // nzimm=16: imm4=1, others=0
    let inst = (0b011 << 13) | (0 << 12) | (2 << 7) | (1 << 6) | 0b01;
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(decoded, Rv64ZcaInstruction::CAddi16sp { nzimm: 16 });
}

#[test]
fn test_caddi16sp_negative() {
    // nzimm=-16: imm9=1, imm8_7=11, imm6=1, imm5=1, imm4=1
    let inst =
        (0b011 << 13) | (1 << 12) | (2 << 7) | (1 << 6) | (1 << 5) | (0b11 << 3) | (1 << 2) | 0b01;
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(decoded, Rv64ZcaInstruction::CAddi16sp { nzimm: -16 });
}

#[test]
fn test_caddi16sp_reserved_zero() {
    let inst = (0b011 << 13) | (0 << 12) | (2 << 7) | 0b01;
    assert!(Rv64ZcaInstruction::<Reg<u64>>::try_decode(inst).is_none());
}

#[test]
fn test_clui_basic() {
    // nzimm=0x1000: imm16:12=1, imm17=0
    let inst = (0b011 << 13) | (0 << 12) | (10 << 7) | (1 << 2) | 0b01;
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CLui {
            rd: Reg::A0,
            nzimm: 0x1000
        }
    );
}

#[test]
fn test_clui_negative() {
    // imm17=1, imm16:12=11111 -> sign-extended = -4096
    let inst = (0b011 << 13) | (1 << 12) | (10 << 7) | (0b11111 << 2) | 0b01;
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CLui {
            rd: Reg::A0,
            nzimm: -4096
        }
    );
}

#[test]
fn test_clui_reserved_zero() {
    let inst = (0b011 << 13) | (0 << 12) | (10 << 7) | (0 << 2) | 0b01;
    assert!(Rv64ZcaInstruction::<Reg<u64>>::try_decode(inst).is_none());
}

#[test]
fn test_csrli_basic() {
    let inst = (0b100 << 13) | (0 << 12) | (0b00 << 10) | (0 << 7) | (4 << 2) | 0b01;
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CSrli {
            rd: Reg::S0,
            shamt: 4
        }
    );
}

#[test]
fn test_csrli_shamt63() {
    // shamt=63 (6-bit max): shamt5=1, shamt4:0=11111
    let inst = (0b100 << 13) | (1 << 12) | (0b00 << 10) | (0 << 7) | (0b11111 << 2) | 0b01;
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CSrli {
            rd: Reg::S0,
            shamt: 63
        }
    );
}

#[test]
fn test_csrli_hint_shamt0() {
    // shamt=0 is a hint — must be accepted
    let inst = (0b100 << 13) | (0 << 12) | (0b00 << 10) | (0 << 7) | (0 << 2) | 0b01;
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CSrli {
            rd: Reg::S0,
            shamt: 0
        }
    );
}

#[test]
fn test_csrai_basic() {
    let inst = (0b100 << 13) | (0 << 12) | (0b01 << 10) | (0 << 7) | (8 << 2) | 0b01;
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CSrai {
            rd: Reg::S0,
            shamt: 8
        }
    );
}

#[test]
fn test_csrai_hint_shamt0() {
    // shamt=0 is a hint — must be accepted
    let inst = (0b100 << 13) | (0 << 12) | (0b01 << 10) | (0 << 7) | (0 << 2) | 0b01;
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CSrai {
            rd: Reg::S0,
            shamt: 0
        }
    );
}

#[test]
fn test_candi() {
    // imm=-1: imm5=1, imm4:0=11111
    let inst = (0b100 << 13) | (1 << 12) | (0b10 << 10) | (0 << 7) | (0b11111 << 2) | 0b01;
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CAndi {
            rd: Reg::S0,
            imm: -1
        }
    );
}

#[test]
fn test_candi_most_negative() {
    // imm6=0b100000 = -32 in 6-bit signed (minimum value)
    let inst = (0b100 << 13) | (1 << 12) | (0b10 << 10) | (0 << 7) | (0b00000 << 2) | 0b01;
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CAndi {
            rd: Reg::S0,
            imm: -32
        }
    );
}

#[test]
fn test_csub() {
    let inst = make_ca_arith(0, 0, 0b00, 1);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CSub {
            rd: Reg::S0,
            rs2: Reg::S1
        }
    );
}

#[test]
fn test_cxor() {
    let inst = make_ca_arith(0, 0, 0b01, 1);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CXor {
            rd: Reg::S0,
            rs2: Reg::S1
        }
    );
}

#[test]
fn test_cor() {
    let inst = make_ca_arith(0, 0, 0b10, 1);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::COr {
            rd: Reg::S0,
            rs2: Reg::S1
        }
    );
}

#[test]
fn test_cand() {
    let inst = make_ca_arith(0, 0, 0b11, 1);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CAnd {
            rd: Reg::S0,
            rs2: Reg::S1
        }
    );
}

#[test]
fn test_csubw() {
    let inst = make_ca_arith(1, 0, 0b00, 1);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CSubw {
            rd: Reg::S0,
            rs2: Reg::S1
        }
    );
}

#[test]
fn test_caddw() {
    let inst = make_ca_arith(1, 0, 0b01, 1);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CAddw {
            rd: Reg::S0,
            rs2: Reg::S1
        }
    );
}

#[test]
fn test_ca_arith_bit12_1_funct2b_10_reserved() {
    // bit12=1, funct2b=10 is used by Zcb — must not decode as Zca
    let inst = make_ca_arith(1, 0, 0b10, 1);
    assert!(Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).is_none());
}

#[test]
fn test_ca_arith_bit12_1_funct2b_11_reserved() {
    // bit12=1, funct2b=11 is used by Zcb — must not decode as Zca
    let inst = make_ca_arith(1, 0, 0b11, 1);
    assert!(Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).is_none());
}

#[test]
fn test_cj_positive() {
    let inst = make_cj(256);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(decoded, Rv64ZcaInstruction::CJ { imm: 256 });
}

#[test]
fn test_cj_negative() {
    let inst = make_cj(-256);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(decoded, Rv64ZcaInstruction::CJ { imm: -256 });
}

#[test]
fn test_cbeqz_positive() {
    let inst = make_cb_branch(0b110, 0, 16);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CBeqz {
            rs1: Reg::S0,
            imm: 16
        }
    );
}

#[test]
fn test_cbeqz_negative() {
    let inst = make_cb_branch(0b110, 0, -8);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CBeqz {
            rs1: Reg::S0,
            imm: -8
        }
    );
}

#[test]
fn test_cbnez_positive() {
    let inst = make_cb_branch(0b111, 1, 32);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CBnez {
            rs1: Reg::S1,
            imm: 32
        }
    );
}

// Quadrant 10

#[test]
fn test_cslli_basic() {
    let inst = (0b000 << 13) | (0 << 12) | (10 << 7) | (3 << 2) | 0b10;
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CSlli {
            rd: Reg::A0,
            shamt: 3
        }
    );
}

#[test]
fn test_cslli_shamt63() {
    // shamt=63 (6-bit max): shamt5=1, shamt4:0=11111
    let inst = (0b000 << 13) | (1 << 12) | (10 << 7) | (0b11111 << 2) | 0b10;
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CSlli {
            rd: Reg::A0,
            shamt: 63
        }
    );
}

#[test]
fn test_cslli_hint_shamt0() {
    // shamt=0, rd≠0: hint — must be accepted
    let inst = (0b000 << 13) | (0 << 12) | (10 << 7) | (0 << 2) | 0b10;
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CSlli {
            rd: Reg::A0,
            shamt: 0
        }
    );
}

#[test]
fn test_cslli_hint_rd0() {
    // rd=0, shamt≠0: hint — must be accepted
    let inst = (0b000 << 13) | (0 << 12) | (0 << 7) | (3 << 2) | 0b10;
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CSlli {
            rd: Reg::Zero,
            shamt: 3
        }
    );
}

#[test]
fn test_clwsp_basic() {
    // uimm=4: uimm5=0, uimm4:2=001, uimm7:6=00
    let inst = (0b010 << 13) | (0 << 12) | (10 << 7) | (0b001 << 4) | (0 << 2) | 0b10;
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CLwsp {
            rd: Reg::A0,
            uimm: 4
        }
    );
}

#[test]
fn test_clwsp_max_uimm() {
    // uimm=252: uimm5=1, uimm4:2=111, uimm7:6=11
    let inst = (0b010 << 13) | (1 << 12) | (10 << 7) | (0b111 << 4) | (0b11 << 2) | 0b10;
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CLwsp {
            rd: Reg::A0,
            uimm: 252
        }
    );
}

#[test]
fn test_clwsp_reserved_rd0() {
    let inst = (0b010 << 13) | (0 << 12) | (0 << 7) | (1 << 4) | 0b10;
    assert!(Rv64ZcaInstruction::<Reg<u64>>::try_decode(inst).is_none());
}

#[test]
fn test_cldsp_basic() {
    // uimm=8: uimm5=0, uimm4:3=01, uimm8:6=000
    let inst = (0b011 << 13) | (0 << 12) | (10 << 7) | (0b01 << 5) | (0b000 << 2) | 0b10;
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CLdsp {
            rd: Reg::A0,
            uimm: 8
        }
    );
}

#[test]
fn test_cldsp_reserved_rd0() {
    let inst = (0b011 << 13) | (0 << 12) | (0 << 7) | (1 << 5) | 0b10;
    assert!(Rv64ZcaInstruction::<Reg<u64>>::try_decode(inst).is_none());
}

#[test]
fn test_cjr() {
    let inst = make_cr_q10(0b100, 0, 10, 0);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(decoded, Rv64ZcaInstruction::CJr { rs1: Reg::A0 });
}

#[test]
fn test_cjr_reserved_rs1_0() {
    let inst = make_cr_q10(0b100, 0, 0, 0);
    assert!(Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).is_none());
}

#[test]
fn test_cmv() {
    let inst = make_cr_q10(0b100, 0, 10, 11);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CMv {
            rd: Reg::A0,
            rs2: Reg::A1
        }
    );
}

#[test]
fn test_cmv_hint_rd0() {
    // rd=0, rs2≠0: hint — must be accepted
    let inst = make_cr_q10(0b100, 0, 0, 11);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CMv {
            rd: Reg::Zero,
            rs2: Reg::A1
        }
    );
}

#[test]
fn test_cmv_rs2_0_decodes_as_cjr() {
    // rs2=0 with bit12=0 is C.JR, not C.MV
    let inst = make_cr_q10(0b100, 0, 10, 0);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert!(matches!(decoded, Rv64ZcaInstruction::CJr { .. }));
}

#[test]
fn test_cebreak() {
    let inst = make_cr_q10(0b100, 1, 0, 0);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(decoded, Rv64ZcaInstruction::CEbreak);
}

#[test]
fn test_cjalr() {
    let inst = make_cr_q10(0b100, 1, 10, 0);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(decoded, Rv64ZcaInstruction::CJalr { rs1: Reg::A0 });
}

#[test]
fn test_cadd() {
    let inst = make_cr_q10(0b100, 1, 10, 11);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CAdd {
            rd: Reg::A0,
            rs2: Reg::A1
        }
    );
}

#[test]
fn test_cadd_hint_rd0() {
    // rd=0, rs2≠0: hint — must be accepted
    let inst = make_cr_q10(0b100, 1, 0, 11);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CAdd {
            rd: Reg::Zero,
            rs2: Reg::A1
        }
    );
}

#[test]
fn test_cswsp_basic() {
    let inst = make_swsp(10, 4);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CSwsp {
            rs2: Reg::A0,
            uimm: 4
        }
    );
}

#[test]
fn test_cswsp_max_uimm() {
    // uimm=252: uimm[7:6]=11, uimm[5:2]=1111
    let inst = make_swsp(10, 252);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CSwsp {
            rs2: Reg::A0,
            uimm: 252
        }
    );
}

#[test]
fn test_csdsp_basic() {
    // uimm=8: uimm[5:3]=001, uimm[8:6]=000
    let inst = make_sdsp(10, 8);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CSdsp {
            rs2: Reg::A0,
            uimm: 8
        }
    );
}

#[test]
fn test_csdsp_max_uimm() {
    // uimm=504: uimm[8:6]=111, uimm[5:3]=111
    let inst = make_sdsp(10, 504);
    let decoded = Rv64ZcaInstruction::<Reg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CSdsp {
            rs2: Reg::A0,
            uimm: 504
        }
    );
}

// Invalid / Reserved

#[test]
fn test_quadrant_11_invalid() {
    // Quadrant 11 = 32-bit instruction territory
    let inst = 0x00000033;
    assert!(Rv64ZcaInstruction::<Reg<u64>>::try_decode(inst).is_none());
}

// RV64E variant

#[test]
fn test_ereg_clw_valid() {
    // C.LW with prime registers (x8-x15) — valid for RV64E
    let inst = make_cl_cs(0b010, 0, 0b000, 1, 0, 0);
    let decoded = Rv64ZcaInstruction::<EReg<u64>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcaInstruction::CLw {
            rd: EReg::S0,
            rs1: EReg::S0,
            uimm: 4
        }
    );
}

#[test]
fn test_ereg_cslli_invalid_high_reg() {
    // C.SLLI rd=x16 — x16 does not exist in EReg, from_bits must fail
    let inst = (0b000 << 13) | (0 << 12) | (16 << 7) | (3 << 2) | 0b10;
    assert!(Rv64ZcaInstruction::<EReg<u64>>::try_decode(inst).is_none());
}
