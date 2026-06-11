extern crate alloc;
use crate::instructions::Instruction;
use crate::instructions::test_utils::make_r_type;
use crate::instructions::zvbb::ZvbbInstruction;
use crate::registers::general_purpose::Reg;
use crate::registers::vector::VReg;
use alloc::format;

/// Build an OP-V instruction word.
///
/// Format: `funct6[31:26] | vm[25] | vs2[24:20] | vs1[19:15] | funct3[14:12] | vd[11:7] |
/// opcode[6:0]`
fn make_vop(funct6: u8, vm: u8, vs2: u8, vs1: u8, funct3: u8, vd: u8) -> u32 {
    let funct7 = (funct6 << 1u8) | (vm & 1);
    make_r_type(0b101_0111, vd, funct3, vs1, vs2, funct7)
}

const OPIVV: u8 = 0b000;
const OPIVX: u8 = 0b100;
const OPIVI: u8 = 0b011;
const OPMVV: u8 = 0b010;

// Negative: wrong opcode
#[test]
fn wrong_opcode_not_decoded() {
    // Use OP (0b011_0011) instead of OP-V (0b101_0111); funct6=0b110101 (vwsll)
    let funct7 = 0b11_0101u8 << 1;
    let inst = make_r_type(0b011_0011, 1, OPIVV, 2, 3, funct7);
    assert_eq!(ZvbbInstruction::<Reg<u64>>::try_decode(inst), None);
}

// Negative: unrelated funct6 in the OPIVV slot; vadd.vv (funct6=0b000000) must not
// decode as any Zvbb-specific variant (it may decode as an inherited ZveXx variant).
#[test]
fn vadd_funct6_not_claimed_by_zvbb() {
    let inst = make_vop(0b00_0000, 1, 2, 3, OPIVV, 1);
    assert!(!matches!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvbbInstruction::VbrevV { .. })
            | Some(ZvbbInstruction::VclzV { .. })
            | Some(ZvbbInstruction::VctzV { .. })
            | Some(ZvbbInstruction::VcpopV { .. })
            | Some(ZvbbInstruction::VwsllVv { .. })
            | Some(ZvbbInstruction::VwsllVx { .. })
            | Some(ZvbbInstruction::VwsllVi { .. })
    ));
}

// Negative: vandn funct6 (Zvkb, 0b000001) in OPIVV must not decode as vwsll.vv.
#[test]
fn vandn_funct6_not_claimed_by_zvbb_opivv() {
    let inst = make_vop(0b00_0001, 1, 2, 3, OPIVV, 1);
    assert!(!matches!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvbbInstruction::VwsllVv { .. })
    ));
}

// Negative: undefined funct6 in OPIVV between the Zvkb range and vwsll (0b110101)
// must not decode as any Zvbb-specific variant (it may decode as an inherited instruction).
#[test]
fn undefined_opivv_funct6_not_claimed_by_zvbb() {
    let inst = make_vop(0b10_0000, 1, 4, 5, OPIVV, 2);
    assert!(!matches!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvbbInstruction::VbrevV { .. })
            | Some(ZvbbInstruction::VclzV { .. })
            | Some(ZvbbInstruction::VctzV { .. })
            | Some(ZvbbInstruction::VcpopV { .. })
            | Some(ZvbbInstruction::VwsllVv { .. })
            | Some(ZvbbInstruction::VwsllVx { .. })
            | Some(ZvbbInstruction::VwsllVi { .. })
    ));
}

// Negative: OPIVX with vandn funct6 (Zvkb, 0b000001) must not decode as vwsll.vx.
#[test]
fn vandn_vx_funct6_not_claimed_by_zvbb() {
    let inst = make_vop(0b00_0001, 1, 4, 10, OPIVX, 6);
    assert!(!matches!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvbbInstruction::VwsllVx { .. })
    ));
}

// Negative: OPIVI with vror funct6 (Zvkb, 0b010100) must not decode as vwsll.vi.
#[test]
fn vror_vi_funct6_not_claimed_by_zvbb() {
    let inst = make_vop(0b01_0100, 0, 8, 0b00111, OPIVI, 2);
    assert!(!matches!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvbbInstruction::VwsllVi { .. })
    ));
}

// Negative: OPMVV wrong funct6 with a valid Zvbb vs1 sub-opcode must not decode.
#[test]
fn opmvv_wrong_funct6_not_claimed() {
    // funct6=0b000001 (vredand.vs sub-space) with vs1=0b01010 (vbrev sub-opcode)
    let inst = make_vop(0b00_0001, 1, 3, 0b01010, OPMVV, 2);
    assert!(!matches!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvbbInstruction::VbrevV { .. })
            | Some(ZvbbInstruction::VclzV { .. })
            | Some(ZvbbInstruction::VctzV { .. })
            | Some(ZvbbInstruction::VcpopV { .. })
    ));
}

// Negative: OPMVV funct6=0b010010 vs1=0b01011 (undefined gap between vbrev and vclz).
#[test]
fn opmvv_undefined_gap_vs1_not_decoded() {
    let inst = make_vop(0b01_0010, 1, 5, 0b01011, OPMVV, 3);
    assert_eq!(ZvbbInstruction::<Reg<u64>>::try_decode(inst), None);
}

// Negative: OPMVV funct6=0b010010 vs1=0b01111 (past vcpop, undefined).
#[test]
fn opmvv_vs1_past_vcpop_not_decoded() {
    let inst = make_vop(0b01_0010, 1, 5, 0b01111, OPMVV, 3);
    assert_eq!(ZvbbInstruction::<Reg<u64>>::try_decode(inst), None);
}

// Negative: OPMVV funct6=0b010010 vs1=0b00000 (vzext.vf8, not a Zvbb op).
#[test]
fn opmvv_vzext_vs1_not_claimed_by_zvbb() {
    let inst = make_vop(0b01_0010, 1, 5, 0b00000, OPMVV, 3);
    assert_eq!(ZvbbInstruction::<Reg<u64>>::try_decode(inst), None);
}

// Negative: OPMVV funct6=0b010010 vs1=0b01000 (vbrev8, Zvkb) must not decode
// as any Zvbb-specific variant through ZvbbInstruction::try_decode.
#[test]
fn opmvv_vbrev8_vs1_not_claimed_by_zvbb_decode_path() {
    let inst = make_vop(0b01_0010, 1, 5, 0b01000, OPMVV, 3);
    assert!(!matches!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvbbInstruction::VbrevV { .. })
            | Some(ZvbbInstruction::VclzV { .. })
            | Some(ZvbbInstruction::VctzV { .. })
            | Some(ZvbbInstruction::VcpopV { .. })
    ));
}

// vbrev.v
#[test]
fn vbrev_v_basic_unmasked() {
    // funct6=0b010010, vm=1, vs2=v8, vs1_bits=0b01010 (vbrev sub-opcode), OPMVV, vd=v5
    let inst = make_vop(0b01_0010, 1, 8, 0b01010, OPMVV, 5);
    assert_eq!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvbbInstruction::VbrevV {
            vd: VReg::V5,
            vs2: VReg::V8,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

#[test]
fn vbrev_v_masked_form() {
    let inst = make_vop(0b01_0010, 0, 8, 0b01010, OPMVV, 5);
    assert_eq!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvbbInstruction::VbrevV {
            vd: VReg::V5,
            vs2: VReg::V8,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

#[test]
fn vbrev_v_high_regs() {
    let inst = make_vop(0b01_0010, 1, 31, 0b01010, OPMVV, 0);
    assert_eq!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvbbInstruction::VbrevV {
            vd: VReg::V0,
            vs2: VReg::V31,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

// vbrev sub-opcode (0b01010) must not alias vbrev8 (0b01000) or vrev8 (0b01001), which
// are Zvkb instructions sharing the same funct6.
#[test]
fn vbrev_does_not_alias_vbrev8_or_vrev8() {
    let inst_brev8 = make_vop(0b01_0010, 1, 4, 0b01000, OPMVV, 2);
    let inst_rev8 = make_vop(0b01_0010, 1, 4, 0b01001, OPMVV, 2);
    let inst_brev = make_vop(0b01_0010, 1, 4, 0b01010, OPMVV, 2);
    assert!(!matches!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst_brev8),
        Some(ZvbbInstruction::VbrevV { .. })
    ));
    assert!(!matches!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst_rev8),
        Some(ZvbbInstruction::VbrevV { .. })
    ));
    assert!(matches!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst_brev),
        Some(ZvbbInstruction::VbrevV { .. })
    ));
}

// vclz.v
#[test]
fn vclz_v_basic_unmasked() {
    // vs1_bits=0b01100 (sub-opcode 12) selects vclz
    let inst = make_vop(0b01_0010, 1, 12, 0b01100, OPMVV, 4);
    assert_eq!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvbbInstruction::VclzV {
            vd: VReg::V4,
            vs2: VReg::V12,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

#[test]
fn vclz_v_masked_form() {
    let inst = make_vop(0b01_0010, 0, 12, 0b01100, OPMVV, 4);
    assert_eq!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvbbInstruction::VclzV {
            vd: VReg::V4,
            vs2: VReg::V12,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

#[test]
fn vclz_v_high_regs() {
    let inst = make_vop(0b01_0010, 1, 31, 0b01100, OPMVV, 0);
    assert_eq!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvbbInstruction::VclzV {
            vd: VReg::V0,
            vs2: VReg::V31,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

// vctz.v
#[test]
fn vctz_v_basic_unmasked() {
    // vs1_bits=0b01101 (sub-opcode 13) selects vctz
    let inst = make_vop(0b01_0010, 1, 16, 0b01101, OPMVV, 7);
    assert_eq!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvbbInstruction::VctzV {
            vd: VReg::V7,
            vs2: VReg::V16,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

#[test]
fn vctz_v_masked_form() {
    let inst = make_vop(0b01_0010, 0, 16, 0b01101, OPMVV, 7);
    assert_eq!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvbbInstruction::VctzV {
            vd: VReg::V7,
            vs2: VReg::V16,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

// vcpop.v
#[test]
fn vcpop_v_basic_unmasked() {
    // vs1_bits=0b01110 (sub-opcode 14) selects vcpop
    let inst = make_vop(0b01_0010, 1, 20, 0b01110, OPMVV, 9);
    assert_eq!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvbbInstruction::VcpopV {
            vd: VReg::V9,
            vs2: VReg::V20,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

#[test]
fn vcpop_v_masked_form() {
    let inst = make_vop(0b01_0010, 0, 20, 0b01110, OPMVV, 9);
    assert_eq!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvbbInstruction::VcpopV {
            vd: VReg::V9,
            vs2: VReg::V20,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

// All four Zvbb OPMVV sub-opcodes share funct6 but must decode to distinct variants.
#[test]
fn vbrev_vclz_vctz_vcpop_sub_opcodes_do_not_alias() {
    let inst_brev = make_vop(0b01_0010, 1, 4, 0b01010, OPMVV, 2);
    let inst_clz = make_vop(0b01_0010, 1, 4, 0b01100, OPMVV, 2);
    let inst_ctz = make_vop(0b01_0010, 1, 4, 0b01101, OPMVV, 2);
    let inst_cpop = make_vop(0b01_0010, 1, 4, 0b01110, OPMVV, 2);
    assert!(matches!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst_brev),
        Some(ZvbbInstruction::VbrevV { .. })
    ));
    assert!(matches!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst_clz),
        Some(ZvbbInstruction::VclzV { .. })
    ));
    assert!(matches!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst_ctz),
        Some(ZvbbInstruction::VctzV { .. })
    ));
    assert!(matches!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst_cpop),
        Some(ZvbbInstruction::VcpopV { .. })
    ));
}

// vwsll.vv
#[test]
fn vwsll_vv_basic_unmasked() {
    let inst = make_vop(0b11_0101, 1, 6, 7, OPIVV, 4);
    assert_eq!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvbbInstruction::VwsllVv {
            vd: VReg::V4,
            vs2: VReg::V6,
            vs1: VReg::V7,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

#[test]
fn vwsll_vv_masked_form() {
    let inst = make_vop(0b11_0101, 0, 6, 7, OPIVV, 4);
    assert_eq!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvbbInstruction::VwsllVv {
            vd: VReg::V4,
            vs2: VReg::V6,
            vs1: VReg::V7,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

#[test]
fn vwsll_vv_high_regs() {
    let inst = make_vop(0b11_0101, 1, 31, 30, OPIVV, 29);
    assert_eq!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvbbInstruction::VwsllVv {
            vd: VReg::V29,
            vs2: VReg::V31,
            vs1: VReg::V30,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

// vwsll.vx
#[test]
fn vwsll_vx_basic_unmasked() {
    // rs1 = a0 (x10)
    let inst = make_vop(0b11_0101, 1, 8, 10, OPIVX, 5);
    assert_eq!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvbbInstruction::VwsllVx {
            vd: VReg::V5,
            vs2: VReg::V8,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        }),
    );
}

#[test]
fn vwsll_vx_masked_form() {
    let inst = make_vop(0b11_0101, 0, 8, 10, OPIVX, 5);
    assert_eq!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvbbInstruction::VwsllVx {
            vd: VReg::V5,
            vs2: VReg::V8,
            rs1: Reg::A0,
            vm: false,
            rs2: Reg::Zero,
        }),
    );
}

// vwsll.vv and vwsll.vx share funct6 but differ by funct3; they must not alias each other.
#[test]
fn vwsll_vv_and_vx_do_not_alias() {
    let inst_vv = make_vop(0b11_0101, 1, 4, 5, OPIVV, 2);
    let inst_vx = make_vop(0b11_0101, 1, 4, 5, OPIVX, 2);
    assert!(matches!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst_vv),
        Some(ZvbbInstruction::VwsllVv { .. })
    ));
    assert!(matches!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst_vx),
        Some(ZvbbInstruction::VwsllVx { .. })
    ));
}

// vwsll.vi - uimm=0, smallest shift amount
#[test]
fn vwsll_vi_uimm_zero() {
    let inst = make_vop(0b11_0101, 1, 10, 0b00000, OPIVI, 6);
    assert_eq!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvbbInstruction::VwsllVi {
            vd: VReg::V6,
            vs2: VReg::V10,
            uimm: 0,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

// uimm=1
#[test]
fn vwsll_vi_uimm_one() {
    let inst = make_vop(0b11_0101, 1, 10, 0b00001, OPIVI, 6);
    assert_eq!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvbbInstruction::VwsllVi {
            vd: VReg::V6,
            vs2: VReg::V10,
            uimm: 1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

// uimm=31, largest 5-bit value
#[test]
fn vwsll_vi_uimm_31() {
    let inst = make_vop(0b11_0101, 1, 14, 0b11111, OPIVI, 8);
    assert_eq!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvbbInstruction::VwsllVi {
            vd: VReg::V8,
            vs2: VReg::V14,
            uimm: 31,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

// vwsll.vi: masked form (vm=0)
#[test]
fn vwsll_vi_masked_form() {
    let inst = make_vop(0b11_0101, 0, 14, 0b11111, OPIVI, 8);
    assert_eq!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvbbInstruction::VwsllVi {
            vd: VReg::V8,
            vs2: VReg::V14,
            uimm: 31,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

// vwsll.vi: vm bit (bit[25]) is the standard mask control; same uimm, different masking.
// Unlike vror.vi, vwsll.vi has a 5-bit immediate only - bit[25] is never part of the immediate.
#[test]
fn vwsll_vi_vm_is_independent_of_immediate() {
    let inst_masked = make_vop(0b11_0101, 0, 12, 0b01010, OPIVI, 3);
    let inst_unmasked = make_vop(0b11_0101, 1, 12, 0b01010, OPIVI, 3);
    assert_eq!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst_masked),
        Some(ZvbbInstruction::VwsllVi {
            vd: VReg::V3,
            vs2: VReg::V12,
            uimm: 10,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
    assert_eq!(
        ZvbbInstruction::<Reg<u64>>::try_decode(inst_unmasked),
        Some(ZvbbInstruction::VwsllVi {
            vd: VReg::V3,
            vs2: VReg::V12,
            uimm: 10,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

// Display
#[test]
fn display_vbrev_v() {
    let inst = make_vop(0b01_0010, 1, 8, 0b01010, OPMVV, 5);
    let decoded = ZvbbInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vbrev.v v5, v8");
}

#[test]
fn display_vbrev_v_masked() {
    let inst = make_vop(0b01_0010, 0, 8, 0b01010, OPMVV, 5);
    let decoded = ZvbbInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vbrev.v v5, v8, v0.t");
}

#[test]
fn display_vclz_v() {
    let inst = make_vop(0b01_0010, 1, 12, 0b01100, OPMVV, 4);
    let decoded = ZvbbInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vclz.v v4, v12");
}

#[test]
fn display_vclz_v_masked() {
    let inst = make_vop(0b01_0010, 0, 12, 0b01100, OPMVV, 4);
    let decoded = ZvbbInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vclz.v v4, v12, v0.t");
}

#[test]
fn display_vctz_v() {
    let inst = make_vop(0b01_0010, 1, 16, 0b01101, OPMVV, 7);
    let decoded = ZvbbInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vctz.v v7, v16");
}

#[test]
fn display_vctz_v_masked() {
    let inst = make_vop(0b01_0010, 0, 16, 0b01101, OPMVV, 7);
    let decoded = ZvbbInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vctz.v v7, v16, v0.t");
}

#[test]
fn display_vcpop_v() {
    let inst = make_vop(0b01_0010, 1, 20, 0b01110, OPMVV, 9);
    let decoded = ZvbbInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vcpop.v v9, v20");
}

#[test]
fn display_vcpop_v_masked() {
    let inst = make_vop(0b01_0010, 0, 20, 0b01110, OPMVV, 9);
    let decoded = ZvbbInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vcpop.v v9, v20, v0.t");
}

#[test]
fn display_vwsll_vv() {
    let inst = make_vop(0b11_0101, 1, 6, 7, OPIVV, 4);
    let decoded = ZvbbInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vwsll.vv v4, v6, v7");
}

#[test]
fn display_vwsll_vv_masked() {
    let inst = make_vop(0b11_0101, 0, 6, 7, OPIVV, 4);
    let decoded = ZvbbInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vwsll.vv v4, v6, v7, v0.t");
}

#[test]
fn display_vwsll_vx() {
    // rs1 = a0 (x10)
    let inst = make_vop(0b11_0101, 1, 8, 10, OPIVX, 5);
    let decoded = ZvbbInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vwsll.vx v5, v8, a0");
}

#[test]
fn display_vwsll_vx_masked() {
    let inst = make_vop(0b11_0101, 0, 8, 10, OPIVX, 5);
    let decoded = ZvbbInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vwsll.vx v5, v8, a0, v0.t");
}

// vwsll.vi: unmasked (vm=1) produces no suffix
#[test]
fn display_vwsll_vi_unmasked() {
    let inst = make_vop(0b11_0101, 1, 14, 0b11111, OPIVI, 8);
    let decoded = ZvbbInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vwsll.vi v8, v14, 31");
}

// vwsll.vi: masked (vm=0) appends ", v0.t"
#[test]
fn display_vwsll_vi_masked() {
    let inst = make_vop(0b11_0101, 0, 14, 0b11111, OPIVI, 8);
    let decoded = ZvbbInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vwsll.vi v8, v14, 31, v0.t");
}

#[test]
fn display_vwsll_vi_small_imm() {
    let inst = make_vop(0b11_0101, 1, 10, 0b00001, OPIVI, 6);
    let decoded = ZvbbInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vwsll.vi v6, v10, 1");
}
