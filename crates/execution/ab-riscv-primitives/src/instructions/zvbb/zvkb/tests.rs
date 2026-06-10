extern crate alloc;

use crate::instructions::Instruction;
use crate::instructions::test_utils::make_r_type;
use crate::instructions::zvbb::zvkb::ZvkbInstruction;
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
    // Use OP (0b011_0011) instead of OP-V (0b101_0111); funct6=0b000001 (vandn)
    let funct7 = 0b00_0001u8 << 1;
    let inst = make_r_type(0b011_0011, 1, OPIVV, 2, 3, funct7);
    assert_eq!(ZvkbInstruction::<Reg<u64>>::try_decode(inst), None);
}

// Negative: unrelated funct6 in the same funct3 slots

#[test]
fn vadd_funct6_not_claimed() {
    // vadd.vv (funct6=0b000000) may decode as a non-Zvkb variant in a combined instruction set;
    // it must not decode as any Zvkb-specific variant.
    let inst = make_vop(0b00_0000, 1, 2, 3, OPIVV, 1);
    assert!(!matches!(
        ZvkbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvkbInstruction::VandnVv { .. })
            | Some(ZvkbInstruction::VandnVx { .. })
            | Some(ZvkbInstruction::Vbrev8V { .. })
            | Some(ZvkbInstruction::Vrev8V { .. })
            | Some(ZvkbInstruction::VrolVv { .. })
            | Some(ZvkbInstruction::VrolVx { .. })
            | Some(ZvkbInstruction::VrorVv { .. })
            | Some(ZvkbInstruction::VrorVx { .. })
            | Some(ZvkbInstruction::VrorVi { .. })
    ));
}

#[test]
fn vadc_funct6_not_claimed() {
    // vadc.vvm (funct6=0b010000) may decode as a non-Zvkb variant in a combined instruction set;
    // it must not decode as any Zvkb-specific variant.
    let inst = make_vop(0b01_0000, 0, 2, 3, OPIVV, 1);
    assert!(!matches!(
        ZvkbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvkbInstruction::VandnVv { .. })
            | Some(ZvkbInstruction::VandnVx { .. })
            | Some(ZvkbInstruction::Vbrev8V { .. })
            | Some(ZvkbInstruction::Vrev8V { .. })
            | Some(ZvkbInstruction::VrolVv { .. })
            | Some(ZvkbInstruction::VrolVx { .. })
            | Some(ZvkbInstruction::VrorVv { .. })
            | Some(ZvkbInstruction::VrorVx { .. })
            | Some(ZvkbInstruction::VrorVi { .. })
    ));
}

// funct6=0b010110 (undefined gap between vror/vrol and vmerge) must not decode.
#[test]
fn undefined_funct6_between_vror_and_vmerge_not_claimed() {
    let inst = make_vop(0b01_0110, 1, 4, 5, OPIVV, 2);
    assert_eq!(ZvkbInstruction::<Reg<u64>>::try_decode(inst), None);
}

#[test]
fn opmvv_wrong_funct6_not_claimed() {
    // OPMVV funct6=0b000001 may decode as a non-Zvkb variant in a combined instruction set
    // (e.g. vredand.vs); it must not decode as any Zvkb-specific variant.
    let inst = make_vop(0b00_0001, 1, 3, 0b01000, OPMVV, 2);
    assert!(!matches!(
        ZvkbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvkbInstruction::VandnVv { .. })
            | Some(ZvkbInstruction::VandnVx { .. })
            | Some(ZvkbInstruction::Vbrev8V { .. })
            | Some(ZvkbInstruction::Vrev8V { .. })
            | Some(ZvkbInstruction::VrolVv { .. })
            | Some(ZvkbInstruction::VrolVx { .. })
            | Some(ZvkbInstruction::VrorVv { .. })
            | Some(ZvkbInstruction::VrorVx { .. })
            | Some(ZvkbInstruction::VrorVi { .. })
    ));
}

// OPMVV with funct6=0b010010 but vs1=0b00000 (vzext.vf8, not a Zvkb op).
#[test]
fn opmvv_vzext_vs1_not_claimed() {
    let inst = make_vop(0b01_0010, 1, 5, 0b00000, OPMVV, 3);
    assert_eq!(ZvkbInstruction::<Reg<u64>>::try_decode(inst), None);
}

// OPMVV with funct6=0b010010 but vs1=0b01010 (vbrev.v from Zvbb, not Zvkb).
#[test]
fn opmvv_vbrev_vs1_not_claimed_by_zvkb() {
    let inst = make_vop(0b01_0010, 1, 5, 0b01010, OPMVV, 3);
    assert_eq!(ZvkbInstruction::<Reg<u64>>::try_decode(inst), None);
}

// OPIVI with funct6=0b010101 must not decode (no vrol.vi in Zvkb).
#[test]
fn vrol_has_no_immediate_form() {
    let inst = make_vop(0b01_0101, 0, 4, 7, OPIVI, 2);
    assert_eq!(ZvkbInstruction::<Reg<u64>>::try_decode(inst), None);
}

// OPIVI with funct6=0b000001 must not decode (no vandn.vi in Zvkb).
#[test]
fn vandn_has_no_immediate_form() {
    let inst = make_vop(0b00_0001, 0, 4, 7, OPIVI, 2);
    assert_eq!(ZvkbInstruction::<Reg<u64>>::try_decode(inst), None);
}

// vandn.vv

#[test]
fn vandn_vv_basic_unmasked() {
    // vm=1: unmasked form
    let inst = make_vop(0b00_0001, 1, 2, 3, OPIVV, 1);
    assert_eq!(
        ZvkbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvkbInstruction::VandnVv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

#[test]
fn vandn_vv_masked_form() {
    // vm=0: masked form - same variant type, vm=false
    let inst = make_vop(0b00_0001, 0, 2, 3, OPIVV, 1);
    assert_eq!(
        ZvkbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvkbInstruction::VandnVv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

#[test]
fn vandn_vv_high_regs() {
    let inst = make_vop(0b00_0001, 1, 31, 30, OPIVV, 29);
    assert_eq!(
        ZvkbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvkbInstruction::VandnVv {
            vd: VReg::V29,
            vs2: VReg::V31,
            vs1: VReg::V30,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

// vandn.vx

#[test]
fn vandn_vx_basic_unmasked() {
    // rs1 = a0 (x10)
    let inst = make_vop(0b00_0001, 1, 4, 10, OPIVX, 6);
    assert_eq!(
        ZvkbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvkbInstruction::VandnVx {
            vd: VReg::V6,
            vs2: VReg::V4,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        }),
    );
}

#[test]
fn vandn_vx_masked_form() {
    let inst = make_vop(0b00_0001, 0, 4, 10, OPIVX, 6);
    assert_eq!(
        ZvkbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvkbInstruction::VandnVx {
            vd: VReg::V6,
            vs2: VReg::V4,
            rs1: Reg::A0,
            vm: false,
            rs2: Reg::Zero,
        }),
    );
}

// vbrev8.v

#[test]
fn vbrev8_v_basic_unmasked() {
    // funct6=0b010010, vm=1, vs2=v8, vs1_bits=0b01000 (sub-opcode 8), OPMVV, vd=v5
    let inst = make_vop(0b01_0010, 1, 8, 0b01000, OPMVV, 5);
    assert_eq!(
        ZvkbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvkbInstruction::Vbrev8V {
            vd: VReg::V5,
            vs2: VReg::V8,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

#[test]
fn vbrev8_v_masked_form() {
    let inst = make_vop(0b01_0010, 0, 8, 0b01000, OPMVV, 5);
    assert_eq!(
        ZvkbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvkbInstruction::Vbrev8V {
            vd: VReg::V5,
            vs2: VReg::V8,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

#[test]
fn vbrev8_v_high_regs() {
    let inst = make_vop(0b01_0010, 1, 31, 0b01000, OPMVV, 0);
    assert_eq!(
        ZvkbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvkbInstruction::Vbrev8V {
            vd: VReg::V0,
            vs2: VReg::V31,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

// vrev8.v

#[test]
fn vrev8_v_basic_unmasked() {
    // vs1_bits=0b01001 (sub-opcode 9) selects vrev8 over vbrev8
    let inst = make_vop(0b01_0010, 1, 12, 0b01001, OPMVV, 7);
    assert_eq!(
        ZvkbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvkbInstruction::Vrev8V {
            vd: VReg::V7,
            vs2: VReg::V12,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

#[test]
fn vrev8_v_masked_form() {
    let inst = make_vop(0b01_0010, 0, 12, 0b01001, OPMVV, 7);
    assert_eq!(
        ZvkbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvkbInstruction::Vrev8V {
            vd: VReg::V7,
            vs2: VReg::V12,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

// vbrev8 and vrev8 share funct6 - sub-opcode must distinguish them correctly.
#[test]
fn vbrev8_and_vrev8_sub_opcodes_do_not_alias() {
    let inst_brev8 = make_vop(0b01_0010, 1, 4, 0b01000, OPMVV, 2);
    let inst_rev8 = make_vop(0b01_0010, 1, 4, 0b01001, OPMVV, 2);
    assert!(matches!(
        ZvkbInstruction::<Reg<u64>>::try_decode(inst_brev8),
        Some(ZvkbInstruction::Vbrev8V { .. })
    ));
    assert!(matches!(
        ZvkbInstruction::<Reg<u64>>::try_decode(inst_rev8),
        Some(ZvkbInstruction::Vrev8V { .. })
    ));
}
// vrol.vv

#[test]
fn vrol_vv_basic_unmasked() {
    let inst = make_vop(0b01_0101, 1, 6, 7, OPIVV, 4);
    assert_eq!(
        ZvkbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvkbInstruction::VrolVv {
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
fn vrol_vv_masked_form() {
    let inst = make_vop(0b01_0101, 0, 6, 7, OPIVV, 4);
    assert_eq!(
        ZvkbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvkbInstruction::VrolVv {
            vd: VReg::V4,
            vs2: VReg::V6,
            vs1: VReg::V7,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

// vrol.vx

#[test]
fn vrol_vx_basic_unmasked() {
    // rs1 = t1 (x6)
    let inst = make_vop(0b01_0101, 1, 10, 6, OPIVX, 8);
    assert_eq!(
        ZvkbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvkbInstruction::VrolVx {
            vd: VReg::V8,
            vs2: VReg::V10,
            rs1: Reg::T1,
            vm: true,
            rs2: Reg::Zero,
        }),
    );
}

#[test]
fn vrol_vx_masked_form() {
    let inst = make_vop(0b01_0101, 0, 10, 6, OPIVX, 8);
    assert_eq!(
        ZvkbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvkbInstruction::VrolVx {
            vd: VReg::V8,
            vs2: VReg::V10,
            rs1: Reg::T1,
            vm: false,
            rs2: Reg::Zero,
        }),
    );
}

// vror.vv

#[test]
fn vror_vv_basic_unmasked() {
    let inst = make_vop(0b01_0100, 1, 14, 15, OPIVV, 0);
    assert_eq!(
        ZvkbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvkbInstruction::VrorVv {
            vd: VReg::V0,
            vs2: VReg::V14,
            vs1: VReg::V15,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

#[test]
fn vror_vv_masked_form() {
    let inst = make_vop(0b01_0100, 0, 14, 15, OPIVV, 0);
    assert_eq!(
        ZvkbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvkbInstruction::VrorVv {
            vd: VReg::V0,
            vs2: VReg::V14,
            vs1: VReg::V15,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

// vrol and vror share the same funct3 (OPIVV/OPIVX) but differ by funct6 bit[0];
// they must not alias each other.
#[test]
fn vrol_and_vror_funct6_do_not_alias() {
    let inst_rol = make_vop(0b01_0101, 1, 2, 3, OPIVV, 1);
    let inst_ror = make_vop(0b01_0100, 1, 2, 3, OPIVV, 1);
    assert!(matches!(
        ZvkbInstruction::<Reg<u64>>::try_decode(inst_rol),
        Some(ZvkbInstruction::VrolVv { .. })
    ));
    assert!(matches!(
        ZvkbInstruction::<Reg<u64>>::try_decode(inst_ror),
        Some(ZvkbInstruction::VrorVv { .. })
    ));
}

// vror.vx

#[test]
fn vror_vx_basic_unmasked() {
    // rs1 = a1 (x11)
    let inst = make_vop(0b01_0100, 1, 16, 11, OPIVX, 0);
    assert_eq!(
        ZvkbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvkbInstruction::VrorVx {
            vd: VReg::V0,
            vs2: VReg::V16,
            rs1: Reg::A1,
            vm: true,
            rs2: Reg::Zero,
        }),
    );
}

#[test]
fn vror_vx_masked_form() {
    let inst = make_vop(0b01_0100, 0, 16, 11, OPIVX, 0);
    assert_eq!(
        ZvkbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvkbInstruction::VrorVx {
            vd: VReg::V0,
            vs2: VReg::V16,
            rs1: Reg::A1,
            vm: false,
            rs2: Reg::Zero,
        }),
    );
}

// vror.vi

// uimm = 0 - smallest rotate amount; no vm field (bit[25] = imm[5])
#[test]
fn vror_vi_uimm_zero() {
    let inst = make_vop(0b01_0100, 0, 8, 0b00000, OPIVI, 4);
    assert_eq!(
        ZvkbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvkbInstruction::VrorVi {
            vd: VReg::V4,
            vs2: VReg::V8,
            uimm: 0,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

// uimm = 1 - imm[5]=0, imm[4:0]=0b00001
#[test]
fn vror_vi_uimm_one() {
    let inst = make_vop(0b01_0100, 0, 8, 0b00001, OPIVI, 4);
    assert_eq!(
        ZvkbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvkbInstruction::VrorVi {
            vd: VReg::V4,
            vs2: VReg::V8,
            uimm: 1,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

// uimm = 31 - largest 5-bit value; imm[5]=0
#[test]
fn vror_vi_uimm_31() {
    let inst = make_vop(0b01_0100, 0, 6, 0b11111, OPIVI, 2);
    assert_eq!(
        ZvkbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvkbInstruction::VrorVi {
            vd: VReg::V2,
            vs2: VReg::V6,
            uimm: 31,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

// vm=1 (unmasked) with imm=0: bit[25] is vm, independent of the 5-bit immediate
#[test]
fn vror_vi_unmasked_zero_imm() {
    let inst = make_vop(0b01_0100, 1, 10, 0b00000, OPIVI, 5);
    assert_eq!(
        ZvkbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvkbInstruction::VrorVi {
            vd: VReg::V5,
            vs2: VReg::V10,
            uimm: 0,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

// vm=1 (unmasked) with maximum 5-bit immediate (31)
#[test]
fn vror_vi_unmasked_max_imm_31() {
    let inst = make_vop(0b01_0100, 1, 4, 0b11111, OPIVI, 1);
    assert_eq!(
        ZvkbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvkbInstruction::VrorVi {
            vd: VReg::V1,
            vs2: VReg::V4,
            uimm: 31,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

// vm=1 (unmasked), imm=0b01001=9: bit[25]=vm is independent of the immediate
#[test]
fn vror_vi_unmasked_imm_9() {
    let inst = make_vop(0b01_0100, 1, 12, 0b01001, OPIVI, 3);
    assert_eq!(
        ZvkbInstruction::<Reg<u64>>::try_decode(inst),
        Some(ZvkbInstruction::VrorVi {
            vd: VReg::V3,
            vs2: VReg::V12,
            uimm: 9,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

// vm=0 and vm=1 with the same 5-bit immediate: same uimm, different masking
#[test]
fn vror_vi_vm_bit_independent_of_immediate() {
    let inst0 = make_vop(0b01_0100, 0, 8, 0b00111, OPIVI, 2);
    let inst1 = make_vop(0b01_0100, 1, 8, 0b00111, OPIVI, 2);
    // vm=0: masked, uimm=7
    assert_eq!(
        ZvkbInstruction::<Reg<u64>>::try_decode(inst0),
        Some(ZvkbInstruction::VrorVi {
            vd: VReg::V2,
            vs2: VReg::V8,
            uimm: 7,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
    // vm=1: unmasked, same uimm=7
    assert_eq!(
        ZvkbInstruction::<Reg<u64>>::try_decode(inst1),
        Some(ZvkbInstruction::VrorVi {
            vd: VReg::V2,
            vs2: VReg::V8,
            uimm: 7,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

// Display

#[test]
fn display_vandn_vv() {
    let inst = make_vop(0b00_0001, 1, 2, 3, OPIVV, 1);
    let decoded = ZvkbInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vandn.vv v1, v2, v3");
}

#[test]
fn display_vandn_vx() {
    let inst = make_vop(0b00_0001, 1, 4, 10, OPIVX, 6);
    let decoded = ZvkbInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vandn.vx v6, v4, a0");
}

#[test]
fn display_vbrev8_v() {
    let inst = make_vop(0b01_0010, 1, 8, 0b01000, OPMVV, 5);
    let decoded = ZvkbInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vbrev8.v v5, v8");
}

#[test]
fn display_vrev8_v() {
    let inst = make_vop(0b01_0010, 1, 12, 0b01001, OPMVV, 7);
    let decoded = ZvkbInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vrev8.v v7, v12");
}

#[test]
fn display_vrol_vv() {
    let inst = make_vop(0b01_0101, 1, 6, 7, OPIVV, 4);
    let decoded = ZvkbInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vrol.vv v4, v6, v7");
}

#[test]
fn display_vrol_vx() {
    let inst = make_vop(0b01_0101, 1, 10, 6, OPIVX, 8);
    let decoded = ZvkbInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vrol.vx v8, v10, t1");
}

#[test]
fn display_vror_vv() {
    let inst = make_vop(0b01_0100, 1, 14, 15, OPIVV, 0);
    let decoded = ZvkbInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vror.vv v0, v14, v15");
}

#[test]
fn display_vror_vx() {
    let inst = make_vop(0b01_0100, 1, 16, 11, OPIVX, 0);
    let decoded = ZvkbInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vror.vx v0, v16, a1");
}

#[test]
fn display_vror_vi_small_imm() {
    let inst = make_vop(0b01_0100, 0, 6, 0b11111, OPIVI, 2);
    let decoded = ZvkbInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vror.vi v2, v6, 31, v0.t");
}

#[test]
fn display_vror_vi_large_imm() {
    // uimm = 31 (vm=1, imm=0b11111); unmasked -> no suffix
    let inst = make_vop(0b01_0100, 1, 4, 0b11111, OPIVI, 1);
    let decoded = ZvkbInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vror.vi v1, v4, 31");
}

// Display: masked forms append ", v0.t"

#[test]
fn display_vandn_vv_masked() {
    let inst = make_vop(0b00_0001, 0, 2, 3, OPIVV, 1);
    let decoded = ZvkbInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vandn.vv v1, v2, v3, v0.t");
}

#[test]
fn display_vandn_vx_masked() {
    let inst = make_vop(0b00_0001, 0, 4, 10, OPIVX, 6);
    let decoded = ZvkbInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vandn.vx v6, v4, a0, v0.t");
}

#[test]
fn display_vbrev8_v_masked() {
    let inst = make_vop(0b01_0010, 0, 8, 0b01000, OPMVV, 5);
    let decoded = ZvkbInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vbrev8.v v5, v8, v0.t");
}

#[test]
fn display_vrev8_v_masked() {
    let inst = make_vop(0b01_0010, 0, 12, 0b01001, OPMVV, 7);
    let decoded = ZvkbInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vrev8.v v7, v12, v0.t");
}

#[test]
fn display_vrol_vv_masked() {
    let inst = make_vop(0b01_0101, 0, 6, 7, OPIVV, 4);
    let decoded = ZvkbInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vrol.vv v4, v6, v7, v0.t");
}

#[test]
fn display_vror_vv_masked() {
    let inst = make_vop(0b01_0100, 0, 14, 15, OPIVV, 0);
    let decoded = ZvkbInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vror.vv v0, v14, v15, v0.t");
}

#[test]
fn display_vror_vx_masked() {
    let inst = make_vop(0b01_0100, 0, 16, 11, OPIVX, 0);
    let decoded = ZvkbInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vror.vx v0, v16, a1, v0.t");
}

// vror.vi: uimm >= 32 (bit[25]=1) -> unmasked -> no ", v0.t" suffix
#[test]
fn display_vror_vi_large_imm_no_mask_suffix() {
    // uimm=0, vm=1 (unmasked): no suffix
    let inst = make_vop(0b01_0100, 1, 10, 0b00000, OPIVI, 5);
    let decoded = ZvkbInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vror.vi v5, v10, 0");
}

// vror.vi: uimm < 32 (bit[25]=0) -> masked -> ", v0.t" appended
#[test]
fn display_vror_vi_small_imm_has_mask_suffix() {
    // uimm=7 (bit[25]=0, imm[4:0]=7): vm=false -> suffix
    let inst = make_vop(0b01_0100, 0, 8, 0b00111, OPIVI, 2);
    let decoded = ZvkbInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vror.vi v2, v8, 7, v0.t");
}
