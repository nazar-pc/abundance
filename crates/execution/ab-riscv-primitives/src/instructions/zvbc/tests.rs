extern crate alloc;

use crate::instructions::Instruction;
use crate::instructions::test_utils::{TestVectorHart, TestZve32Hart, make_r_type};
use crate::instructions::zvbc::ZvbcInstruction;
use crate::registers::general_purpose::Reg;
use crate::registers::vector::VReg;
use alloc::format;
use core::assert_matches;

/// Build an OP-V instruction word.
///
/// Format: `funct6[31:26] | vm[25] | vs2[24:20] | vs1[19:15] | funct3[14:12] | vd[11:7] |
/// opcode[6:0]`
fn make_vop(funct6: u8, vm: u8, vs2: u8, vs1: u8, funct3: u8, vd: u8) -> u32 {
    let funct7 = (funct6 << 1u8) | (vm & 1);
    make_r_type(0b101_0111, vd, funct3, vs1, vs2, funct7)
}

const OPIVI: u8 = 0b011;
const OPMVV: u8 = 0b010;
const OPMVX: u8 = 0b110;

// Negative: structural rejections

// Wrong major opcode must not decode.
#[test]
fn wrong_opcode_not_decoded() {
    // Use OP (0b011_0011) instead of OP-V (0b101_0111); funct6=0b001100 (vclmul)
    let funct7 = 0b00_1100u8 << 1;
    let inst = make_r_type(0b011_0011, 1, OPMVV, 2, 3, funct7);
    assert_eq!(ZvbcInstruction::<TestVectorHart>::try_decode(inst), None);
}

// OPMVV with funct6=0b000000 must not decode as any Zvbc variant.
#[test]
fn unrelated_funct6_in_opmvv_not_claimed() {
    let inst = make_vop(0b00_0000, 1, 2, 3, OPMVV, 1);
    assert!(!matches!(
        ZvbcInstruction::<TestVectorHart>::try_decode(inst),
        Some(
            ZvbcInstruction::VclmulVv { .. }
                | ZvbcInstruction::VclmulVx { .. }
                | ZvbcInstruction::VclmulhVv { .. }
                | ZvbcInstruction::VclmulhVx { .. }
        )
    ));
}

// OPMVV with funct6=0b001110 (gap immediately above vclmulh) must not decode as any Zvbc variant.
#[test]
fn funct6_above_vclmulh_not_claimed_in_opmvv() {
    let inst = make_vop(0b00_1110, 1, 4, 5, OPMVV, 2);
    assert!(!matches!(
        ZvbcInstruction::<TestVectorHart>::try_decode(inst),
        Some(
            ZvbcInstruction::VclmulVv { .. }
                | ZvbcInstruction::VclmulVx { .. }
                | ZvbcInstruction::VclmulhVv { .. }
                | ZvbcInstruction::VclmulhVx { .. }
        )
    ));
}

// OPMVV with funct6=0b001011 (gap immediately below vclmul) must not decode as any Zvbc variant.
#[test]
fn funct6_below_vclmul_not_claimed_in_opmvv() {
    let inst = make_vop(0b00_1011, 1, 4, 5, OPMVV, 2);
    assert!(!matches!(
        ZvbcInstruction::<TestVectorHart>::try_decode(inst),
        Some(
            ZvbcInstruction::VclmulVv { .. }
                | ZvbcInstruction::VclmulVx { .. }
                | ZvbcInstruction::VclmulhVv { .. }
                | ZvbcInstruction::VclmulhVx { .. }
        )
    ));
}

// OPMVX with an unrelated funct6 must not decode as any Zvbc variant.
#[test]
fn unrelated_funct6_in_opmvx_not_claimed() {
    // funct6=0b000001 in OPMVX slot
    let inst = make_vop(0b00_0001, 1, 3, 10, OPMVX, 5);
    assert!(!matches!(
        ZvbcInstruction::<TestVectorHart>::try_decode(inst),
        Some(
            ZvbcInstruction::VclmulVv { .. }
                | ZvbcInstruction::VclmulVx { .. }
                | ZvbcInstruction::VclmulhVv { .. }
                | ZvbcInstruction::VclmulhVx { .. }
        )
    ));
}

// Zvbc has no immediate forms; funct6=0b001100 in OPIVI must not decode as any Zvbc variant.
#[test]
fn vclmul_has_no_immediate_form() {
    let inst = make_vop(0b00_1100, 1, 4, 7, OPIVI, 2);
    assert!(!matches!(
        ZvbcInstruction::<TestVectorHart>::try_decode(inst),
        Some(
            ZvbcInstruction::VclmulVv { .. }
                | ZvbcInstruction::VclmulVx { .. }
                | ZvbcInstruction::VclmulhVv { .. }
                | ZvbcInstruction::VclmulhVx { .. }
        )
    ));
}

// Zvbc has no immediate forms; funct6=0b001101 in OPIVI must not decode as any Zvbc variant.
#[test]
fn vclmulh_has_no_immediate_form() {
    let inst = make_vop(0b00_1101, 1, 4, 7, OPIVI, 2);
    assert!(!matches!(
        ZvbcInstruction::<TestVectorHart>::try_decode(inst),
        Some(
            ZvbcInstruction::VclmulVv { .. }
                | ZvbcInstruction::VclmulVx { .. }
                | ZvbcInstruction::VclmulhVv { .. }
                | ZvbcInstruction::VclmulhVx { .. }
        )
    ));
}

// vclmul.vv

#[test]
fn vclmul_vv_basic_unmasked() {
    // vm=1: unmasked form
    let inst = make_vop(0b00_1100, 1, 2, 3, OPMVV, 1);
    assert_eq!(
        ZvbcInstruction::<TestVectorHart>::try_decode(inst),
        Some(ZvbcInstruction::VclmulVv {
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
fn vclmul_vv_masked_form() {
    // vm=0: masked form
    let inst = make_vop(0b00_1100, 0, 2, 3, OPMVV, 1);
    assert_eq!(
        ZvbcInstruction::<TestVectorHart>::try_decode(inst),
        Some(ZvbcInstruction::VclmulVv {
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
fn vclmul_vv_high_regs() {
    let inst = make_vop(0b00_1100, 1, 31, 30, OPMVV, 29);
    assert_eq!(
        ZvbcInstruction::<TestVectorHart>::try_decode(inst),
        Some(ZvbcInstruction::VclmulVv {
            vd: VReg::V29,
            vs2: VReg::V31,
            vs1: VReg::V30,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

// vclmul.vx

#[test]
fn vclmul_vx_basic_unmasked() {
    // rs1 = a0 (x10)
    let inst = make_vop(0b00_1100, 1, 4, 10, OPMVX, 6);
    assert_eq!(
        ZvbcInstruction::<TestVectorHart>::try_decode(inst),
        Some(ZvbcInstruction::VclmulVx {
            vd: VReg::V6,
            vs2: VReg::V4,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        }),
    );
}

#[test]
fn vclmul_vx_masked_form() {
    // rs1 = a0 (x10)
    let inst = make_vop(0b00_1100, 0, 4, 10, OPMVX, 6);
    assert_eq!(
        ZvbcInstruction::<TestVectorHart>::try_decode(inst),
        Some(ZvbcInstruction::VclmulVx {
            vd: VReg::V6,
            vs2: VReg::V4,
            rs1: Reg::A0,
            vm: false,
            rs2: Reg::Zero,
        }),
    );
}

#[test]
fn vclmul_vx_t1_register() {
    // rs1 = t1 (x6)
    let inst = make_vop(0b00_1100, 1, 8, 6, OPMVX, 3);
    assert_eq!(
        ZvbcInstruction::<TestVectorHart>::try_decode(inst),
        Some(ZvbcInstruction::VclmulVx {
            vd: VReg::V3,
            vs2: VReg::V8,
            rs1: Reg::T1,
            vm: true,
            rs2: Reg::Zero,
        }),
    );
}

// vclmulh.vv

#[test]
fn vclmulh_vv_basic_unmasked() {
    // vm=1: unmasked form
    let inst = make_vop(0b00_1101, 1, 5, 6, OPMVV, 7);
    assert_eq!(
        ZvbcInstruction::<TestVectorHart>::try_decode(inst),
        Some(ZvbcInstruction::VclmulhVv {
            vd: VReg::V7,
            vs2: VReg::V5,
            vs1: VReg::V6,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

#[test]
fn vclmulh_vv_masked_form() {
    // vm=0: masked form
    let inst = make_vop(0b00_1101, 0, 5, 6, OPMVV, 7);
    assert_eq!(
        ZvbcInstruction::<TestVectorHart>::try_decode(inst),
        Some(ZvbcInstruction::VclmulhVv {
            vd: VReg::V7,
            vs2: VReg::V5,
            vs1: VReg::V6,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

#[test]
fn vclmulh_vv_high_regs() {
    let inst = make_vop(0b00_1101, 1, 28, 27, OPMVV, 26);
    assert_eq!(
        ZvbcInstruction::<TestVectorHart>::try_decode(inst),
        Some(ZvbcInstruction::VclmulhVv {
            vd: VReg::V26,
            vs2: VReg::V28,
            vs1: VReg::V27,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

// vclmulh.vx

#[test]
fn vclmulh_vx_basic_unmasked() {
    // rs1 = a1 (x11)
    let inst = make_vop(0b00_1101, 1, 12, 11, OPMVX, 0);
    assert_eq!(
        ZvbcInstruction::<TestVectorHart>::try_decode(inst),
        Some(ZvbcInstruction::VclmulhVx {
            vd: VReg::V0,
            vs2: VReg::V12,
            rs1: Reg::A1,
            vm: true,
            rs2: Reg::Zero,
        }),
    );
}

#[test]
fn vclmulh_vx_masked_form() {
    // rs1 = a1 (x11)
    let inst = make_vop(0b00_1101, 0, 12, 11, OPMVX, 8);
    assert_eq!(
        ZvbcInstruction::<TestVectorHart>::try_decode(inst),
        Some(ZvbcInstruction::VclmulhVx {
            vd: VReg::V8,
            vs2: VReg::V12,
            rs1: Reg::A1,
            vm: false,
            rs2: Reg::Zero,
        }),
    );
}

#[test]
fn vclmulh_vx_sp_register() {
    // rs1 = sp (x2)
    let inst = make_vop(0b00_1101, 1, 20, 2, OPMVX, 15);
    assert_eq!(
        ZvbcInstruction::<TestVectorHart>::try_decode(inst),
        Some(ZvbcInstruction::VclmulhVx {
            vd: VReg::V15,
            vs2: VReg::V20,
            rs1: Reg::Sp,
            vm: true,
            rs2: Reg::Zero,
        }),
    );
}

// Aliasing: vclmul and vclmulh differ only in funct6 bit[0]

// vclmul and vclmulh share the same funct3 slots but differ by one funct6 bit;
// they must not alias each other in the OPMVV space.
#[test]
fn vclmul_and_vclmulh_vv_funct6_do_not_alias() {
    let inst_clmul = make_vop(0b00_1100, 1, 2, 3, OPMVV, 1);
    let inst_clmulh = make_vop(0b00_1101, 1, 2, 3, OPMVV, 1);
    assert_matches!(
        ZvbcInstruction::<TestVectorHart>::try_decode(inst_clmul),
        Some(ZvbcInstruction::VclmulVv { .. })
    );
    assert_matches!(
        ZvbcInstruction::<TestVectorHart>::try_decode(inst_clmulh),
        Some(ZvbcInstruction::VclmulhVv { .. })
    );
}

// Same aliasing check in the OPMVX space.
#[test]
fn vclmul_and_vclmulh_vx_funct6_do_not_alias() {
    let inst_clmul = make_vop(0b00_1100, 1, 4, 10, OPMVX, 6);
    let inst_clmulh = make_vop(0b00_1101, 1, 4, 10, OPMVX, 6);
    assert_matches!(
        ZvbcInstruction::<TestVectorHart>::try_decode(inst_clmul),
        Some(ZvbcInstruction::VclmulVx { .. })
    );
    assert_matches!(
        ZvbcInstruction::<TestVectorHart>::try_decode(inst_clmulh),
        Some(ZvbcInstruction::VclmulhVx { .. })
    );
}

// VV and VX variants share funct6 but differ by funct3 (OPMVV vs OPMVX); must not alias.
#[test]
fn vclmul_vv_and_vx_funct3_do_not_alias() {
    let inst_vv = make_vop(0b00_1100, 1, 8, 3, OPMVV, 2);
    let inst_vx = make_vop(0b00_1100, 1, 8, 3, OPMVX, 2);
    assert_matches!(
        ZvbcInstruction::<TestVectorHart>::try_decode(inst_vv),
        Some(ZvbcInstruction::VclmulVv { .. })
    );
    assert_matches!(
        ZvbcInstruction::<TestVectorHart>::try_decode(inst_vx),
        Some(ZvbcInstruction::VclmulVx { .. })
    );
}

// vm field orthogonality

// vm=0 and vm=1 with identical register fields produce the same variant but
// different masking; verifies the vm bit is decoded independently of operands.
#[test]
fn vm_bit_decoded_independently_of_operands() {
    let inst_unmasked = make_vop(0b00_1100, 1, 6, 7, OPMVV, 4);
    let inst_masked = make_vop(0b00_1100, 0, 6, 7, OPMVV, 4);
    assert_eq!(
        ZvbcInstruction::<TestVectorHart>::try_decode(inst_unmasked),
        Some(ZvbcInstruction::VclmulVv {
            vd: VReg::V4,
            vs2: VReg::V6,
            vs1: VReg::V7,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
    assert_eq!(
        ZvbcInstruction::<TestVectorHart>::try_decode(inst_masked),
        Some(ZvbcInstruction::VclmulVv {
            vd: VReg::V4,
            vs2: VReg::V6,
            vs1: VReg::V7,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }),
    );
}

// Display

#[test]
fn display_vclmul_vv_unmasked() {
    let inst = make_vop(0b00_1100, 1, 2, 3, OPMVV, 1);
    let decoded = ZvbcInstruction::<TestVectorHart>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vclmul.vv v1, v2, v3");
}

#[test]
fn display_vclmul_vv_masked() {
    let inst = make_vop(0b00_1100, 0, 2, 3, OPMVV, 1);
    let decoded = ZvbcInstruction::<TestVectorHart>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vclmul.vv v1, v2, v3, v0.t");
}

#[test]
fn display_vclmul_vx_unmasked() {
    // rs1 = a0 (x10)
    let inst = make_vop(0b00_1100, 1, 4, 10, OPMVX, 6);
    let decoded = ZvbcInstruction::<TestVectorHart>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vclmul.vx v6, v4, a0");
}

#[test]
fn display_vclmul_vx_masked() {
    // rs1 = a0 (x10)
    let inst = make_vop(0b00_1100, 0, 4, 10, OPMVX, 6);
    let decoded = ZvbcInstruction::<TestVectorHart>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vclmul.vx v6, v4, a0, v0.t");
}

#[test]
fn display_vclmulh_vv_unmasked() {
    let inst = make_vop(0b00_1101, 1, 5, 6, OPMVV, 7);
    let decoded = ZvbcInstruction::<TestVectorHart>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vclmulh.vv v7, v5, v6");
}

#[test]
fn display_vclmulh_vv_masked() {
    let inst = make_vop(0b00_1101, 0, 5, 6, OPMVV, 7);
    let decoded = ZvbcInstruction::<TestVectorHart>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vclmulh.vv v7, v5, v6, v0.t");
}

#[test]
fn display_vclmulh_vx_unmasked() {
    // rs1 = a1 (x11)
    let inst = make_vop(0b00_1101, 1, 12, 11, OPMVX, 0);
    let decoded = ZvbcInstruction::<TestVectorHart>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vclmulh.vx v0, v12, a1");
}

#[test]
fn display_vclmulh_vx_masked() {
    // rs1 = a1 (x11)
    let inst = make_vop(0b00_1101, 0, 12, 11, OPMVX, 8);
    let decoded = ZvbcInstruction::<TestVectorHart>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vclmulh.vx v8, v12, a1, v0.t");
}

#[test]
fn masked_v0_data_source_is_reserved() {
    for (name, funct6, vs2, vs1, funct3) in [
        ("vclmul.vv vs2", 0b00_1100, 0, 1, 0b010),
        ("vclmul.vv vs1", 0b00_1100, 1, 0, 0b010),
        ("vclmulh.vx vs2", 0b00_1101, 0, 1, 0b110),
    ] {
        let instruction = make_vop(funct6, 0, vs2, vs1, funct3, 8);
        assert_eq!(
            ZvbcInstruction::<TestVectorHart>::try_decode(instruction),
            None,
            "{name}"
        );
        // Same instruction with `v4` in place of `v0`
        let vs2 = if vs2 == 0 { 4 } else { vs2 };
        let vs1 = if vs1 == 0 { 4 } else { vs1 };
        let instruction = make_vop(funct6, 0, vs2, vs1, funct3, 8);
        assert!(
            ZvbcInstruction::<TestVectorHart>::try_decode(instruction).is_some(),
            "{name}"
        );
    }
}

#[test]
fn masked_v0_destination_is_reserved() {
    for (name, funct6, vs2, vs1, funct3) in [
        ("vclmul.vv", 0b00_1100, 1, 2, 0b010),
        ("vclmul.vx", 0b00_1100, 1, 2, 0b110),
        ("vclmulh.vv", 0b00_1101, 1, 2, 0b010),
        ("vclmulh.vx", 0b00_1101, 1, 2, 0b110),
    ] {
        let instruction = make_vop(funct6, 0, vs2, vs1, funct3, 0);
        assert_eq!(
            ZvbcInstruction::<TestVectorHart>::try_decode(instruction),
            None,
            "{name}"
        );
        // Same instruction with `vd = v8`
        let instruction = make_vop(funct6, 0, vs2, vs1, funct3, 8);
        assert!(
            ZvbcInstruction::<TestVectorHart>::try_decode(instruction).is_some(),
            "{name}"
        );
    }
}

// All instructions require `SEW = 64`, so they don't exist with `ELEN = 32`
#[test]
fn not_decoded_with_elen_32() {
    for (funct6, funct3) in [
        (0b00_1100, OPMVV),
        (0b00_1100, OPMVX),
        (0b00_1101, OPMVV),
        (0b00_1101, OPMVX),
    ] {
        let inst = make_vop(funct6, 1, 2, 3, funct3, 1);
        assert!(ZvbcInstruction::<TestVectorHart>::try_decode(inst).is_some());
        assert_eq!(ZvbcInstruction::<TestZve32Hart>::try_decode(inst), None);
    }
}
