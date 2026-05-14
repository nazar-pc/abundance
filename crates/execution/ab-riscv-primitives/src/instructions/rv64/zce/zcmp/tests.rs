#![expect(clippy::identity_op, reason = "Test readability")]

use crate::instructions::Instruction;
use crate::instructions::rv64::zce::zcmp::{Rv64ZcmpInstruction, ZcmpUrlist};
use crate::registers::general_purpose::{EReg, Reg};

/// Build a Zcmp push/pop instruction word.
///
/// funct3=101 at bits\[15:13], funct2_12_11=11 at bits\[12:11],
/// op_sel at bits\[10:9] (00=push, 01=pop, 10=popretz, 11=popret),
/// urlist at bits\[7:4], spimm at bits\[3:2], quadrant=10.
const fn make_push_pop(op_sel: u16, urlist: u16, spimm: u16) -> u32 {
    let inst: u16 =
        (0b101 << 13) | (0b11 << 11) | (op_sel << 9) | (urlist << 4) | (spimm << 2) | 0b10;
    u32::from(inst)
}

/// op_sel values from the Zcmp spec / binutils MATCH_CM_* constants.
const OP_PUSH: u16 = 0b00;
const OP_POP: u16 = 0b01;
const OP_POPRETZ: u16 = 0b10;
const OP_POPRET: u16 = 0b11;

/// Build a CM.MVA01S or CM.MVSA01 instruction word.
///
/// Encoding (common): funct3=101 at bits\[15:13], bits\[12:10]=011, bit\[1:0]=10.
/// r1s' occupies bits\[9:7], r2s' occupies bits\[4:2], and the funct2 field at
/// bits\[6:5] discriminates: 0b11 -> CM.MVA01S, 0b01 -> CM.MVSA01.
const fn make_mv_pair(funct2: u16, r1s: u16, r2s: u16) -> u32 {
    let inst: u16 = (0b101 << 13) | (0b011 << 10) | (r1s << 7) | (funct2 << 5) | (r2s << 2) | 0b10;
    u32::from(inst)
}

const MV_FUNCT2_MVA01S: u16 = 0b11;
const MV_FUNCT2_MVSA01: u16 = 0b01;

/// Compute the expected stack_adj for a given urlist raw value and spimm.
fn expected_stack_adj(urlist_raw: u8, spimm: u8) -> u8 {
    ZcmpUrlist::<Reg<u64>>::try_from_raw(urlist_raw)
        .unwrap()
        .stack_adj_base()
        + spimm * 16
}

// CM.PUSH

#[test]
fn test_cm_push_ra_only() {
    // urlist=4 = {ra}, spimm=0 -> stack_adj = 16 + 0 = 16
    let inst = make_push_pop(OP_PUSH, 4, 0);
    let decoded = Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcmpInstruction::CmPush {
            urlist: ZcmpUrlist::try_from_raw(4).unwrap(),
            stack_adj: 16,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_cm_push_ra_s0_s11() {
    // urlist=15 = {ra, s0-s11}, spimm=3 -> stack_adj = 112 + 48 = 160
    let inst = make_push_pop(OP_PUSH, 15, 3);
    let decoded = Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcmpInstruction::CmPush {
            urlist: ZcmpUrlist::try_from_raw(15).unwrap(),
            stack_adj: 160,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_cm_push_all_valid_urlists() {
    for urlist in 4u16..=15 {
        let inst = make_push_pop(OP_PUSH, urlist, 0);
        let decoded = Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv64ZcmpInstruction::CmPush {
                urlist: ZcmpUrlist::try_from_raw(urlist as u8).unwrap(),
                stack_adj: expected_stack_adj(urlist as u8, 0),
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "urlist={urlist}"
        );
    }
}

#[test]
fn test_cm_push_reserved_urlist() {
    // urlist 0..=3 are reserved; decoder must return None
    for urlist in 0u16..4 {
        let inst = make_push_pop(OP_PUSH, urlist, 0);
        assert!(
            Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).is_none(),
            "urlist={urlist} should be reserved"
        );
    }
}

/// Reference encodings anchored to binutils MATCH_CM_PUSH=0xb802
/// and a real RP2350 firmware disassembly sample (0xb842 -> cm.push {ra},-16).
#[test]
fn test_cm_push_binutils_reference_encodings() {
    // (raw, urlist_raw, stack_adj_rv64)
    let cases = &[
        (0xb842u32, 4, 16),
        (0xb84e, 4, 64),
        (0xb882, 8, 48),
        (0xb88e, 8, 96),
        (0xb8f2, 15, 112),
        (0xb8fe, 15, 160),
    ];
    for &(raw, urlist_raw, stack_adj) in cases {
        let decoded = Rv64ZcmpInstruction::<Reg<u64>>::try_decode(raw).unwrap();
        assert_eq!(
            decoded,
            Rv64ZcmpInstruction::CmPush {
                urlist: ZcmpUrlist::try_from_raw(urlist_raw).unwrap(),
                stack_adj,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "raw={raw:#06x}"
        );
    }
}

// CM.POP

#[test]
fn test_cm_pop_basic() {
    // urlist=5 = {ra, s0}, spimm=1 -> stack_adj = 16 + 16 = 32
    let inst = make_push_pop(OP_POP, 5, 1);
    let decoded = Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcmpInstruction::CmPop {
            urlist: ZcmpUrlist::try_from_raw(5).unwrap(),
            stack_adj: 32,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_cm_pop_ra_s0_s9() {
    // urlist=14 = {ra, s0-s9}, spimm=2 -> stack_adj = 96 + 32 = 128
    let inst = make_push_pop(OP_POP, 14, 2);
    let decoded = Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcmpInstruction::CmPop {
            urlist: ZcmpUrlist::try_from_raw(14).unwrap(),
            stack_adj: 128,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

/// Reference encodings anchored to binutils MATCH_CM_POP=0xba02.
#[test]
fn test_cm_pop_binutils_reference_encodings() {
    let cases = &[(0xba56u32, 5, 32), (0xbaea, 14, 128)];
    for &(raw, urlist_raw, stack_adj) in cases {
        let decoded = Rv64ZcmpInstruction::<Reg<u64>>::try_decode(raw).unwrap();
        assert_eq!(
            decoded,
            Rv64ZcmpInstruction::CmPop {
                urlist: ZcmpUrlist::try_from_raw(urlist_raw).unwrap(),
                stack_adj,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "raw={raw:#06x}"
        );
    }
}

// CM.POPRETZ

#[test]
fn test_cm_popretz_basic() {
    // urlist=4 = {ra}, spimm=0 -> stack_adj = 16
    let inst = make_push_pop(OP_POPRETZ, 4, 0);
    let decoded = Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcmpInstruction::CmPopretz {
            urlist: ZcmpUrlist::try_from_raw(4).unwrap(),
            stack_adj: 16,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

/// Reference encoding anchored to binutils MATCH_CM_POPRETZ=0xbc02.
#[test]
fn test_cm_popretz_binutils_reference_encodings() {
    let decoded = Rv64ZcmpInstruction::<Reg<u64>>::try_decode(0xbc42).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcmpInstruction::CmPopretz {
            urlist: ZcmpUrlist::try_from_raw(4).unwrap(),
            stack_adj: 16,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

// CM.POPRET

#[test]
fn test_cm_popret_basic() {
    // urlist=6 = {ra, s0-s1}, spimm=0 -> stack_adj = 32
    let inst = make_push_pop(OP_POPRET, 6, 0);
    let decoded = Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcmpInstruction::CmPopret {
            urlist: ZcmpUrlist::try_from_raw(6).unwrap(),
            stack_adj: 32,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_cm_popret_all_spimm_values() {
    // urlist=8 = {ra, s0-s3}, stack_adj_base = 48
    // spimm=0 -> 48, spimm=1 -> 64, spimm=2 -> 80, spimm=3 -> 96
    let expected_adjs = [48, 64, 80, 96];
    for (spimm, &expected) in expected_adjs.iter().enumerate() {
        let inst = make_push_pop(OP_POPRET, 8, spimm as u16);
        let decoded = Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv64ZcmpInstruction::CmPopret {
                urlist: ZcmpUrlist::try_from_raw(8).unwrap(),
                stack_adj: expected,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "spimm={spimm}"
        );
    }
}

/// Reference encodings anchored to binutils MATCH_CM_POPRET=0xbe02
/// and a real RP2350 firmware sample (0xbe42 -> cm.popret {ra},16).
#[test]
fn test_cm_popret_binutils_reference_encodings() {
    // (raw, urlist_raw, stack_adj_rv64)
    let cases = &[
        (0xbe42u32, 4, 16),
        (0xbe62, 6, 32),
        (0xbe66, 6, 48),
        (0xbe82, 8, 48),
        (0xbe8e, 8, 96),
    ];
    for &(raw, urlist_raw, stack_adj) in cases {
        let decoded = Rv64ZcmpInstruction::<Reg<u64>>::try_decode(raw).unwrap();
        assert_eq!(
            decoded,
            Rv64ZcmpInstruction::CmPopret {
                urlist: ZcmpUrlist::try_from_raw(urlist_raw).unwrap(),
                stack_adj,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "raw={raw:#06x}"
        );
    }
}

// Op_sel cross-check: distinct instructions at each op_sel value

#[test]
fn test_push_pop_op_sel_distinct_variants() {
    // Same urlist/spimm, each op_sel must decode to a distinct variant.
    let urlist = 4u16;
    let spimm = 0u16;

    assert!(matches!(
        Rv64ZcmpInstruction::<Reg<u64>>::try_decode(make_push_pop(OP_PUSH, urlist, spimm)).unwrap(),
        Rv64ZcmpInstruction::CmPush { .. }
    ));
    assert!(matches!(
        Rv64ZcmpInstruction::<Reg<u64>>::try_decode(make_push_pop(OP_POP, urlist, spimm)).unwrap(),
        Rv64ZcmpInstruction::CmPop { .. }
    ));
    assert!(matches!(
        Rv64ZcmpInstruction::<Reg<u64>>::try_decode(make_push_pop(OP_POPRETZ, urlist, spimm))
            .unwrap(),
        Rv64ZcmpInstruction::CmPopretz { .. }
    ));
    assert!(matches!(
        Rv64ZcmpInstruction::<Reg<u64>>::try_decode(make_push_pop(OP_POPRET, urlist, spimm))
            .unwrap(),
        Rv64ZcmpInstruction::CmPopret { .. }
    ));
}

// CM.MVA01S

#[test]
fn test_cm_mva01s_s0_s1() {
    // r1s field=0 -> s0(x8), r2s field=1 -> s1(x9)
    let inst = make_mv_pair(MV_FUNCT2_MVA01S, 0, 1);
    let decoded = Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcmpInstruction::CmMva01s {
            rs1: Reg::S0,
            rs2: Reg::S1,
        }
    );
}

#[test]
fn test_cm_mva01s_same_reg() {
    // r1s == r2s is allowed for CM.MVA01S
    let inst = make_mv_pair(MV_FUNCT2_MVA01S, 2, 2);
    let decoded = Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcmpInstruction::CmMva01s {
            rs1: Reg::S2,
            rs2: Reg::S2,
        }
    );
}

#[test]
fn test_cm_mva01s_all_s_regs() {
    // Verify the s-register mapping: field 0->s0(x8), 1->s1(x9), 2->s2(x18)..7->s7(x23)
    let expected_regs = [
        Reg::S0,
        Reg::S1,
        Reg::S2,
        Reg::S3,
        Reg::S4,
        Reg::S5,
        Reg::S6,
        Reg::S7,
    ];
    for (field, &expected) in expected_regs.iter().enumerate() {
        let inst = make_mv_pair(MV_FUNCT2_MVA01S, field as u16, 0);
        let decoded = Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).unwrap();
        if let Rv64ZcmpInstruction::CmMva01s { rs1, .. } = decoded {
            assert_eq!(rs1, expected, "field={field}");
        } else {
            panic!("wrong variant for field={field}");
        }
    }
}

/// Reference encodings from the binutils gas test suite (zcmp-mv.d).
/// Confirms the decoder matches the canonical binutils/gas disassembly.
#[test]
fn test_cm_mva01s_binutils_reference_encodings() {
    let cases: &[(u32, Reg<u64>, Reg<u64>)] = &[
        (0xac7e, Reg::S0, Reg::S7),
        (0xac7a, Reg::S0, Reg::S6),
        (0xacfe, Reg::S1, Reg::S7),
        (0xacfa, Reg::S1, Reg::S6),
        (0xafee, Reg::S7, Reg::S3),
        (0xade2, Reg::S3, Reg::S0),
        (0xaef2, Reg::S5, Reg::S4),
        (0xaefa, Reg::S5, Reg::S6),
    ];
    for &(raw, r1s, r2s) in cases {
        let decoded = Rv64ZcmpInstruction::<Reg<u64>>::try_decode(raw).unwrap();
        assert_eq!(
            decoded,
            Rv64ZcmpInstruction::CmMva01s { rs1: r1s, rs2: r2s },
            "raw={raw:#06x}"
        );
    }
}

// CM.MVSA01

#[test]
fn test_cm_mvsa01_distinct_regs() {
    // r1s=s0, r2s=s2 (distinct)
    let inst = make_mv_pair(MV_FUNCT2_MVSA01, 0, 2);
    let decoded = Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcmpInstruction::CmMvsa01 {
            rs1: Reg::S0,
            rs2: Reg::S2,
        }
    );
}

#[test]
fn test_cm_mvsa01_reserved_same_reg() {
    // r1s == r2s is reserved for CM.MVSA01; decoder must return None
    let inst = make_mv_pair(MV_FUNCT2_MVSA01, 3, 3);
    assert!(Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).is_none());
}

/// Reference encodings from the binutils gas test suite (zcmp-mv.d).
#[test]
fn test_cm_mvsa01_binutils_reference_encodings() {
    let cases: &[(u32, Reg<u64>, Reg<u64>)] = &[
        (0xafa2, Reg::S7, Reg::S0),
        (0xaf22, Reg::S6, Reg::S0),
        (0xafa6, Reg::S7, Reg::S1),
        (0xaf26, Reg::S6, Reg::S1),
        (0xadbe, Reg::S3, Reg::S7),
        (0xada2, Reg::S3, Reg::S0),
        (0xaeb2, Reg::S5, Reg::S4),
        (0xaeba, Reg::S5, Reg::S6),
    ];
    for &(raw, r1s, r2s) in cases {
        let decoded = Rv64ZcmpInstruction::<Reg<u64>>::try_decode(raw).unwrap();
        assert_eq!(
            decoded,
            Rv64ZcmpInstruction::CmMvsa01 { rs1: r1s, rs2: r2s },
            "raw={raw:#06x}"
        );
    }
}

#[test]
fn test_cm_mv_reserved_funct2_00() {
    // funct2[6:5]=00 is reserved for the mv-pair family
    let inst = make_mv_pair(0b00, 0, 1);
    assert!(Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).is_none());
}

#[test]
fn test_cm_mv_reserved_funct2_10() {
    // funct2[6:5]=10 is reserved for the mv-pair family
    let inst = make_mv_pair(0b10, 0, 1);
    assert!(Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).is_none());
}

#[test]
fn test_cm_mv_reserved_bit10_zero() {
    // funct2_12_11=01 with bit 10 = 0 is not a defined Zcmp encoding
    // (funct6 must be 101_011 for mv-pair)
    let inst: u16 = (0b101 << 13) | (0b01 << 11) | (0b11 << 5) | 0b10;
    assert!(Rv64ZcmpInstruction::<Reg<u64>>::try_decode(u32::from(inst)).is_none());
}

// Wrong quadrant / funct3

#[test]
fn test_non_zcmp_q00_returns_none() {
    // Quadrant 00 is not Zcmp
    let inst = (0b101 << 13) | 0b00;
    assert!(Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).is_none());
}

#[test]
fn test_non_zcmp_q01_returns_none() {
    // Quadrant 01 is not Zcmp
    let inst = (0b101 << 13) | 0b01;
    assert!(Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).is_none());
}

#[test]
fn test_non_zcmp_funct3_mismatch() {
    // Q10 funct3=100 (not 101) -> not Zcmp
    let inst = (0b100 << 13) | (0b11 << 11) | (0b00 << 9) | (4 << 4) | 0b10;
    assert!(Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).is_none());
}

// Reserved funct2_12_11 values

#[test]
fn test_reserved_funct2_00_returns_none() {
    // funct2_12_11=0b00 is not defined by Zcmp
    let inst = (0b101 << 13) | (0b00 << 11) | 0b10;
    assert!(Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).is_none());
}

#[test]
fn test_reserved_funct2_10_returns_none() {
    // funct2_12_11=0b10 is not defined by Zcmp
    let inst = (0b101 << 13) | (0b10 << 11) | 0b10;
    assert!(Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).is_none());
}

// ZcmpUrlist::stack_adj_base (RV64)

#[test]
fn test_stack_adj_base_rv64() {
    // Values from Zcmp spec Table 3 (RV64 column)
    let cases = &[
        (4, 16),
        (5, 16),
        (6, 32),
        (7, 32),
        (8, 48),
        (9, 48),
        (10, 64),
        (11, 64),
        (12, 80),
        (13, 80),
        (14, 96),
        (15, 112),
    ];
    for &(raw, expected) in cases {
        let urlist = ZcmpUrlist::<Reg<u64>>::try_from_raw(raw).unwrap();
        assert_eq!(urlist.stack_adj_base(), expected, "urlist={raw}");
    }
}

// ZcmpUrlist::reg_list

#[test]
fn test_reg_list_ra_only() {
    let urlist = ZcmpUrlist::<Reg<u64>>::try_from_raw(4).unwrap();
    assert!(urlist.reg_list().eq([Reg::Ra]));
}

#[test]
fn test_reg_list_ra_s0_s11() {
    // urlist=15: ra + s0-s9 + s11 (s10 is absent per spec)
    let urlist = ZcmpUrlist::<Reg<u64>>::try_from_raw(15).unwrap();
    assert!(urlist.reg_list().eq([
        Reg::Ra,
        Reg::S0,
        Reg::S1,
        Reg::S2,
        Reg::S3,
        Reg::S4,
        Reg::S5,
        Reg::S6,
        Reg::S7,
        Reg::S8,
        Reg::S9,
        Reg::S10,
        Reg::S11,
    ]));
}

#[test]
fn test_reg_list_count_matches_urlist() {
    // urlist=4: 1 reg (ra), urlist=5: 2 regs (ra, s0), ..., urlist=14: 11 regs (ra, s0-s9),
    // urlist=15: 13 regs (ra, s0-s11) - jumps by 2 because {ra, s0-s10} has no encoding
    let expected_counts = [1usize, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 13];
    for (i, &expected) in expected_counts.iter().enumerate() {
        let raw = (i + 4) as u8;
        let urlist = ZcmpUrlist::<Reg<u64>>::try_from_raw(raw).unwrap();
        let count = urlist.reg_list().count();
        assert_eq!(count, expected, "urlist={raw}");
    }
}

// RVE restrictions

#[test]
fn test_rve_urlist_max_is_ra_s0_s1() {
    // Under RVE, urlist > 6 names inaccessible s-registers and must be rejected
    assert!(ZcmpUrlist::<EReg<u64>>::try_from_raw(6).is_some());
    assert!(ZcmpUrlist::<EReg<u64>>::try_from_raw(7).is_none());
}

#[test]
fn test_rve_push_reserved_urlist() {
    // urlist=7 names s2(x18) which does not exist in RVE
    let inst = make_push_pop(OP_PUSH, 7, 0);
    assert!(Rv64ZcmpInstruction::<EReg<u64>>::try_decode(inst).is_none());
}

#[test]
fn test_rve_mva01s_accessible_regs() {
    // Under RVE only s0(field=0) and s1(field=1) are accessible
    let inst = make_mv_pair(MV_FUNCT2_MVA01S, 0, 1);
    assert!(Rv64ZcmpInstruction::<EReg<u64>>::try_decode(inst).is_some());
}

#[test]
fn test_rve_mva01s_inaccessible_reg_returns_none() {
    // field=2 maps to s2(x18) which does not exist in RVE;
    // corresponds to r1sc > 1 in the spec reserved() pseudocode
    let inst = make_mv_pair(MV_FUNCT2_MVA01S, 2, 0);
    assert!(Rv64ZcmpInstruction::<EReg<u64>>::try_decode(inst).is_none());
}

#[test]
fn test_rve_mvsa01_accessible_regs() {
    // Under RVE, s0 and s1 are distinct and accessible
    let inst = make_mv_pair(MV_FUNCT2_MVSA01, 0, 1);
    assert!(Rv64ZcmpInstruction::<EReg<u64>>::try_decode(inst).is_some());
}

#[test]
fn test_rve_mvsa01_inaccessible_reg_returns_none() {
    // field=2 maps to s2(x18) which does not exist in RVE;
    // corresponds to r2sc > 1 in the spec reserved() pseudocode
    let inst = make_mv_pair(MV_FUNCT2_MVSA01, 0, 2);
    assert!(Rv64ZcmpInstruction::<EReg<u64>>::try_decode(inst).is_none());
}
