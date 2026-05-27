#![expect(clippy::identity_op, reason = "Test readability")]

use crate::instructions::Instruction;
use crate::instructions::rv32::zce::zcb::Rv32ZcbOnlyInstruction;
use crate::registers::general_purpose::Reg;

/// Build a Q00 Zcb load/store: funct3=100, sub=bits\[12:10].
/// `rs1p` and `rd_rs2p` are 3-bit prime register fields (0=x8).
/// `bit6` and `bit5` carry the uimm or funct1 bits.
const fn make_zcb_q00(sub: u16, rs1p: u16, bit6: u16, bit5: u16, rd_rs2p: u16) -> u16 {
    (0b100 << 13u8)
        | (sub << 10u8)
        | (rs1p << 7u8)
        | (bit6 << 6u8)
        | (bit5 << 5u8)
        | (rd_rs2p << 2u8)
}

/// Build the Q01 Zcb unary / C.MUL encoding.
/// funct3=100, funct2_11_10=11, bit12=1, rd_rs1p (prime), funct2b, rs2_sub.
///
/// Per the ratified Zcb spec:
///   funct2b=0b11 => unary ops (rs2_sub selects which)
///   funct2b=0b10 => C.MUL (rs2_sub is rs2' field)
///   funct2b=0b00, 0b01 => reserved
const fn make_zcb_q01(rd_rs1p: u16, funct2b: u16, rs2_sub: u16) -> u16 {
    (0b100 << 13u8)
        | (1 << 12u8)
        | (0b11 << 10u8)
        | (rd_rs1p << 7u8)
        | (funct2b << 5u8)
        | (rs2_sub << 2u8)
        | 0b01
}

// C.LBU

#[test]
fn test_clbu_all_uimm_values() {
    for (bit6, bit5, expected_uimm) in [(0, 0, 0), (1, 0, 1), (0, 1, 2), (1, 1, 3)] {
        let inst = make_zcb_q00(0b000, 0, bit6, bit5, 0);
        let decoded = Rv32ZcbOnlyInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
        assert_eq!(
            decoded,
            Rv32ZcbOnlyInstruction::CLbu {
                rd: Reg::S0,
                rs1: Reg::S0,
                uimm: expected_uimm,
                rs2: Reg::Zero,
            }
        );
    }
}

#[test]
fn test_clbu_all_prime_regs() {
    let inst = make_zcb_q00(0b000, 7, 0, 0, 6);
    let decoded = Rv32ZcbOnlyInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcbOnlyInstruction::CLbu {
            rd: Reg::A4,
            rs1: Reg::A5,
            uimm: 0,
            rs2: Reg::Zero,
        }
    );
}

// C.LHU / C.LH

#[test]
fn test_clhu_uimm0() {
    let inst = make_zcb_q00(0b001, 0, 0, 0, 0);
    let decoded = Rv32ZcbOnlyInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcbOnlyInstruction::CLhu {
            rd: Reg::S0,
            rs1: Reg::S0,
            uimm: 0,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_clhu_uimm2() {
    let inst = make_zcb_q00(0b001, 0, 0, 1, 0);
    let decoded = Rv32ZcbOnlyInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcbOnlyInstruction::CLhu {
            rd: Reg::S0,
            rs1: Reg::S0,
            uimm: 2,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_clh_uimm0() {
    let inst = make_zcb_q00(0b001, 0, 1, 0, 0);
    let decoded = Rv32ZcbOnlyInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcbOnlyInstruction::CLh {
            rd: Reg::S0,
            rs1: Reg::S0,
            uimm: 0,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_clh_uimm2() {
    let inst = make_zcb_q00(0b001, 0, 1, 1, 0);
    let decoded = Rv32ZcbOnlyInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcbOnlyInstruction::CLh {
            rd: Reg::S0,
            rs1: Reg::S0,
            uimm: 2,
            rs2: Reg::Zero,
        }
    );
}

// C.SB

#[test]
fn test_csb_uimm0() {
    let inst = make_zcb_q00(0b010, 0, 0, 0, 0);
    let decoded = Rv32ZcbOnlyInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcbOnlyInstruction::CSb {
            rs1: Reg::S0,
            rs2: Reg::S0,
            uimm: 0
        }
    );
}

#[test]
fn test_csb_uimm3() {
    let inst = make_zcb_q00(0b010, 0, 1, 1, 1);
    let decoded = Rv32ZcbOnlyInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcbOnlyInstruction::CSb {
            rs1: Reg::S0,
            rs2: Reg::S1,
            uimm: 3
        }
    );
}

// C.SH

#[test]
fn test_csh_uimm0() {
    let inst = make_zcb_q00(0b011, 0, 0, 0, 0);
    let decoded = Rv32ZcbOnlyInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcbOnlyInstruction::CSh {
            rs1: Reg::S0,
            rs2: Reg::S0,
            uimm: 0
        }
    );
}

#[test]
fn test_csh_uimm2() {
    let inst = make_zcb_q00(0b011, 0, 0, 1, 1);
    let decoded = Rv32ZcbOnlyInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcbOnlyInstruction::CSh {
            rs1: Reg::S0,
            rs2: Reg::S1,
            uimm: 2
        }
    );
}

#[test]
fn test_csh_funct1_1_reserved() {
    let inst = make_zcb_q00(0b011, 0, 1, 0, 0);
    assert!(Rv32ZcbOnlyInstruction::<Reg<u32>>::try_decode(u32::from(inst)).is_none());
}

// Unary ops - funct2b=0b11

#[test]
fn test_czext_b() {
    let inst = make_zcb_q01(0, 0b11, 0b000);
    let decoded = Rv32ZcbOnlyInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcbOnlyInstruction::CZextB {
            rd: Reg::S0,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_csext_b() {
    let inst = make_zcb_q01(0, 0b11, 0b001);
    let decoded = Rv32ZcbOnlyInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcbOnlyInstruction::CSextB {
            rd: Reg::S0,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_czext_h() {
    let inst = make_zcb_q01(0, 0b11, 0b010);
    let decoded = Rv32ZcbOnlyInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcbOnlyInstruction::CZextH {
            rd: Reg::S0,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_csext_h() {
    let inst = make_zcb_q01(0, 0b11, 0b011);
    let decoded = Rv32ZcbOnlyInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcbOnlyInstruction::CSextH {
            rd: Reg::S0,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

// C.ZEXT.W (sub=0b100) must be absent in RV32
#[test]
fn test_czext_w_absent_in_rv32() {
    let inst = make_zcb_q01(0, 0b11, 0b100);
    assert!(
        Rv32ZcbOnlyInstruction::<Reg<u32>>::try_decode(u32::from(inst)).is_none(),
        "C.ZEXT.W should not exist in RV32"
    );
}

#[test]
fn test_cnot() {
    let inst = make_zcb_q01(0, 0b11, 0b101);
    let decoded = Rv32ZcbOnlyInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcbOnlyInstruction::CNot {
            rd: Reg::S0,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_unary_reserved_sub_110() {
    let inst = make_zcb_q01(0, 0b11, 0b110);
    assert!(Rv32ZcbOnlyInstruction::<Reg<u32>>::try_decode(u32::from(inst)).is_none());
}

#[test]
fn test_unary_reserved_sub_111() {
    let inst = make_zcb_q01(0, 0b11, 0b111);
    assert!(Rv32ZcbOnlyInstruction::<Reg<u32>>::try_decode(u32::from(inst)).is_none());
}

#[test]
fn test_unary_all_prime_regs() {
    // Check all 8 prime registers decode correctly for C.NOT.
    for r in 0..8 {
        let inst = make_zcb_q01(r, 0b11, 0b101);
        let decoded = Rv32ZcbOnlyInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
        assert!(matches!(decoded, Rv32ZcbOnlyInstruction::CNot { .. }));
    }
}

// C.MUL - funct2b=0b10

#[test]
fn test_cmul() {
    // c.mul s0, s1: rd'=0(x8), rs2'=1(x9) => 0x9c45
    let inst = make_zcb_q01(0, 0b10, 0b001);
    let decoded = Rv32ZcbOnlyInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcbOnlyInstruction::CMul {
            rd: Reg::S0,
            rs2: Reg::S1,
            rs1: Reg::Zero,
        }
    );
}

#[test]
fn test_cmul_same_reg() {
    let inst = make_zcb_q01(0, 0b10, 0b000);
    let decoded = Rv32ZcbOnlyInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
    assert_eq!(
        decoded,
        Rv32ZcbOnlyInstruction::CMul {
            rd: Reg::S0,
            rs2: Reg::S0,
            rs1: Reg::Zero,
        }
    );
}

#[test]
fn test_cmul_all_prime_reg_pairs() {
    for rd in 0..8 {
        for rs2 in 0..8 {
            let inst = make_zcb_q01(rd, 0b10, rs2);
            let decoded = Rv32ZcbOnlyInstruction::<Reg<u32>>::try_decode(u32::from(inst)).unwrap();
            assert!(matches!(decoded, Rv32ZcbOnlyInstruction::CMul { .. }));
        }
    }
}

// Reserved funct2b values

#[test]
fn test_reserved_funct2b_00() {
    let inst = make_zcb_q01(0, 0b00, 0);
    assert!(Rv32ZcbOnlyInstruction::<Reg<u32>>::try_decode(u32::from(inst)).is_none());
}

#[test]
fn test_reserved_funct2b_01() {
    let inst = make_zcb_q01(0, 0b01, 0);
    assert!(Rv32ZcbOnlyInstruction::<Reg<u32>>::try_decode(u32::from(inst)).is_none());
}

// Non-Zcb quadrants return None

#[test]
fn test_non_zcb_q10_returns_none() {
    let inst = (0b000 << 13u8) | (1 << 12u8) | (10 << 7u8) | (3 << 2u8) | 0b10;
    assert!(Rv32ZcbOnlyInstruction::<Reg<u32>>::try_decode(inst).is_none());
}

#[test]
fn test_zca_q01_funct3_000_returns_none() {
    let inst = (0b000 << 13u8) | (0 << 12u8) | (10 << 7u8) | (5 << 2u8) | 0b01;
    assert!(Rv32ZcbOnlyInstruction::<Reg<u32>>::try_decode(inst).is_none());
}
