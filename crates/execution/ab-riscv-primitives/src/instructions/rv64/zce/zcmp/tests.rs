#![expect(clippy::identity_op, reason = "Test readability")]

use crate::instructions::Instruction;
use crate::instructions::rv64::zce::zcmp::{Rv64ZcmpInstruction, ZcmpUrlist};
use crate::registers::general_purpose::{EReg, Reg};

/// Build a Zcmp push/pop instruction word.
/// funct3=101, funct2_12_11=11, op_sel=bits\[10:9], urlist=bits\[7:4], spimm=bits\[3:2].
const fn make_push_pop(op_sel: u16, urlist: u16, spimm: u16) -> u32 {
    let inst: u16 =
        (0b101 << 13) | (0b11 << 11) | (op_sel << 9) | (urlist << 4) | (spimm << 2) | 0b10;
    u32::from(inst)
}

/// Build a CM.MVA01S or CM.MVSA01 instruction word.
/// which=1 -> CM.MVA01S, which=0 -> CM.MVSA01.
const fn make_mv_pair(which: u16, r1s: u16, r2s: u16) -> u32 {
    let inst: u16 =
        (0b101 << 13) | (0b01 << 11) | (which << 10) | (r1s << 7) | (0b11 << 5) | (r2s << 2) | 0b10;
    u32::from(inst)
}

/// Compute the expected stack_adj for a given urlist raw value and spimm.
fn expected_stack_adj(urlist_raw: u8, spimm: u32) -> u32 {
    ZcmpUrlist::<Reg<u64>>::try_from_raw(urlist_raw)
        .unwrap()
        .stack_adj_base()
        + spimm * 16
}

// CM.PUSH

#[test]
fn test_cm_push_ra_only() {
    // urlist=4 = {ra}, spimm=0 -> stack_adj = 16 + 0 = 16
    let inst = make_push_pop(0b00, 4, 0);
    let decoded = Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcmpInstruction::CmPush {
            urlist: ZcmpUrlist::try_from_raw(4).unwrap(),
            stack_adj: 16,
        }
    );
}

#[test]
fn test_cm_push_ra_s0_s11() {
    // urlist=15 = {ra, s0-s11}, spimm=3 -> stack_adj = 112 + 48 = 160
    let inst = make_push_pop(0b00, 15, 3);
    let decoded = Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcmpInstruction::CmPush {
            urlist: ZcmpUrlist::try_from_raw(15).unwrap(),
            stack_adj: 160,
        }
    );
}

#[test]
fn test_cm_push_all_valid_urlists() {
    for urlist in 4u16..=15 {
        let inst = make_push_pop(0b00, urlist, 0);
        let decoded = Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv64ZcmpInstruction::CmPush {
                urlist: ZcmpUrlist::try_from_raw(urlist as u8).unwrap(),
                stack_adj: expected_stack_adj(urlist as u8, 0),
            },
            "urlist={urlist}"
        );
    }
}

#[test]
fn test_cm_push_reserved_urlist() {
    // urlist 0..=3 are reserved; decoder must return None
    for urlist in 0u16..4 {
        let inst = make_push_pop(0b00, urlist, 0);
        assert!(
            Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).is_none(),
            "urlist={urlist} should be reserved"
        );
    }
}

// CM.POP

#[test]
fn test_cm_pop_basic() {
    // urlist=5 = {ra, s0}, spimm=1 -> stack_adj = 16 + 16 = 32
    let inst = make_push_pop(0b10, 5, 1);
    let decoded = Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcmpInstruction::CmPop {
            urlist: ZcmpUrlist::try_from_raw(5).unwrap(),
            stack_adj: 32,
        }
    );
}

#[test]
fn test_cm_pop_ra_s0_s9() {
    // urlist=14 = {ra, s0-s9}, spimm=2 -> stack_adj = 96 + 32 = 128
    let inst = make_push_pop(0b10, 14, 2);
    let decoded = Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcmpInstruction::CmPop {
            urlist: ZcmpUrlist::try_from_raw(14).unwrap(),
            stack_adj: 128,
        }
    );
}

// CM.POPRETZ

#[test]
fn test_cm_popretz_basic() {
    // urlist=4 = {ra}, spimm=0 -> stack_adj = 16
    let inst = make_push_pop(0b01, 4, 0);
    let decoded = Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcmpInstruction::CmPopretz {
            urlist: ZcmpUrlist::try_from_raw(4).unwrap(),
            stack_adj: 16,
        }
    );
}

// CM.POPRET

#[test]
fn test_cm_popret_basic() {
    // urlist=6 = {ra, s0-s1}, spimm=0 -> stack_adj = 32
    let inst = make_push_pop(0b11, 6, 0);
    let decoded = Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcmpInstruction::CmPopret {
            urlist: ZcmpUrlist::try_from_raw(6).unwrap(),
            stack_adj: 32,
        }
    );
}

#[test]
fn test_cm_popret_all_spimm_values() {
    // urlist=8 = {ra, s0-s3}, stack_adj_base = 48
    // spimm=0 -> 48, spimm=1 -> 64, spimm=2 -> 80, spimm=3 -> 96
    let expected_adjs = [48, 64, 80, 96];
    for (spimm, &expected) in expected_adjs.iter().enumerate() {
        let inst = make_push_pop(0b11, 8, spimm as u16);
        let decoded = Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv64ZcmpInstruction::CmPopret {
                urlist: ZcmpUrlist::try_from_raw(8).unwrap(),
                stack_adj: expected,
            },
            "spimm={spimm}"
        );
    }
}

// CM.MVA01S

#[test]
fn test_cm_mva01s_s0_s1() {
    // r1s field=0 -> s0(x8), r2s field=1 -> s1(x9)
    let inst = make_mv_pair(1, 0, 1);
    let decoded = Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcmpInstruction::CmMva01s {
            r1s: Reg::S0,
            r2s: Reg::S1,
        }
    );
}

#[test]
fn test_cm_mva01s_same_reg() {
    // r1s == r2s is allowed for CM.MVA01S
    let inst = make_mv_pair(1, 2, 2);
    let decoded = Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcmpInstruction::CmMva01s {
            r1s: Reg::S2,
            r2s: Reg::S2,
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
        let inst = make_mv_pair(1, field as u16, 0);
        let decoded = Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).unwrap();
        if let Rv64ZcmpInstruction::CmMva01s { r1s, .. } = decoded {
            assert_eq!(r1s, expected, "field={field}");
        } else {
            panic!("wrong variant for field={field}");
        }
    }
}

// CM.MVSA01

#[test]
fn test_cm_mvsa01_distinct_regs() {
    // r1s=s0, r2s=s2 (distinct)
    let inst = make_mv_pair(0, 0, 2);
    let decoded = Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64ZcmpInstruction::CmMvsa01 {
            r1s: Reg::S0,
            r2s: Reg::S2,
        }
    );
}

#[test]
fn test_cm_mvsa01_reserved_same_reg() {
    // r1s == r2s is reserved for CM.MVSA01; decoder must return None
    let inst = make_mv_pair(0, 3, 3);
    assert!(Rv64ZcmpInstruction::<Reg<u64>>::try_decode(inst).is_none());
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
        Reg::S11,
    ]));
}

#[test]
fn test_reg_list_count_matches_urlist() {
    // urlist=4: 1 reg, urlist=5: 2 regs, ..., urlist=14: 11 regs, urlist=15: 12 regs
    // (urlist=15 has 12 regs but skips s10, so it's not simply raw-3)
    let expected_counts = [1usize, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
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
    let inst = make_push_pop(0b00, 7, 0);
    assert!(Rv64ZcmpInstruction::<EReg<u64>>::try_decode(inst).is_none());
}

#[test]
fn test_rve_mva01s_accessible_regs() {
    // Under RVE only s0(field=0) and s1(field=1) are accessible
    let inst = make_mv_pair(1, 0, 1);
    assert!(Rv64ZcmpInstruction::<EReg<u64>>::try_decode(inst).is_some());
}

#[test]
fn test_rve_mva01s_inaccessible_reg_returns_none() {
    // field=2 maps to s2(x18) which does not exist in RVE;
    // corresponds to r1sc > 1 in the spec reserved() pseudocode
    let inst = make_mv_pair(1, 2, 0);
    assert!(Rv64ZcmpInstruction::<EReg<u64>>::try_decode(inst).is_none());
}

#[test]
fn test_rve_mvsa01_accessible_regs() {
    // Under RVE, s0 and s1 are distinct and accessible
    let inst = make_mv_pair(0, 0, 1);
    assert!(Rv64ZcmpInstruction::<EReg<u64>>::try_decode(inst).is_some());
}

#[test]
fn test_rve_mvsa01_inaccessible_reg_returns_none() {
    // field=2 maps to s2(x18) which does not exist in RVE;
    // corresponds to r2sc > 1 in the spec reserved() pseudocode
    let inst = make_mv_pair(0, 0, 2);
    assert!(Rv64ZcmpInstruction::<EReg<u64>>::try_decode(inst).is_none());
}
