extern crate alloc;

use crate::instructions::Instruction;
use crate::instructions::rv64::v::zve64x::fixed_point::Rv64Zve64xFixedPointInstruction;
use crate::instructions::test_utils::make_r_type;
use crate::registers::general_purpose::Reg;
use crate::registers::vector::VReg;
use alloc::format;

/// Build a vector arithmetic instruction using make_r_type.
///
/// Vector arithmetic encoding:
/// `[funct6(6)|vm(1)|vs2(5)|vs1_or_rs1_or_imm(5)|funct3(3)|vd(5)|0b1010111]`
///
/// maps to make_r_type with funct7 = (funct6 << 1) | vm_bit
/// where vm_bit=1 means unmasked, vm_bit=0 means masked.
fn make_v_arith(funct6: u8, vm: bool, vs2: u8, vs1: u8, funct3: u8, vd: u8) -> u32 {
    let vm_bit = if vm { 1u8 } else { 0u8 };
    let funct7 = (funct6 << 1) | vm_bit;
    make_r_type(0b1010111, vd, funct3, vs1, vs2, funct7)
}

/// funct3 constants
const OPIVV: u8 = 0b000;
const OPMVV: u8 = 0b010;
const OPIVI: u8 = 0b011;
const OPIVX: u8 = 0b100;
const OPMVX: u8 = 0b110;

// Saturating add/subtract

#[test]
#[cfg_attr(miri, ignore)]
fn test_vsaddu_vv() {
    let inst = make_v_arith(0b100000, true, 2, 3, OPIVV, 1);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VsadduVv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vsaddu_vv_masked() {
    let inst = make_v_arith(0b100000, false, 4, 5, OPIVV, 6);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VsadduVv {
            vd: VReg::V6,
            vs2: VReg::V4,
            vs1: VReg::V5,
            vm: false
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vsaddu_vx() {
    let inst = make_v_arith(0b100000, true, 2, 5, OPIVX, 1);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VsadduVx {
            vd: VReg::V1,
            vs2: VReg::V2,
            rs1: Reg::T0,
            vm: true
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vsaddu_vi() {
    // imm = 5 (positive 5-bit signed)
    let inst = make_v_arith(0b100000, true, 2, 5, OPIVI, 1);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VsadduVi {
            vd: VReg::V1,
            vs2: VReg::V2,
            imm: 5,
            vm: true
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vsaddu_vi_negative() {
    // imm = -1 (0b11111 sign-extended)
    let inst = make_v_arith(0b100000, true, 2, 0b11111, OPIVI, 1);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VsadduVi {
            vd: VReg::V1,
            vs2: VReg::V2,
            imm: -1,
            vm: true
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vsadd_vv() {
    let inst = make_v_arith(0b100001, true, 8, 9, OPIVV, 10);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VsaddVv {
            vd: VReg::V10,
            vs2: VReg::V8,
            vs1: VReg::V9,
            vm: true
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vsadd_vx() {
    let inst = make_v_arith(0b100001, true, 8, 10, OPIVX, 12);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VsaddVx {
            vd: VReg::V12,
            vs2: VReg::V8,
            rs1: Reg::A0,
            vm: true
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vsadd_vi() {
    let inst = make_v_arith(0b100001, true, 8, 15, OPIVI, 12);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VsaddVi {
            vd: VReg::V12,
            vs2: VReg::V8,
            imm: 15,
            vm: true
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vssubu_vv() {
    let inst = make_v_arith(0b100010, true, 2, 3, OPIVV, 1);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VssubuVv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vssubu_vx() {
    let inst = make_v_arith(0b100010, true, 2, 5, OPIVX, 1);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VssubuVx {
            vd: VReg::V1,
            vs2: VReg::V2,
            rs1: Reg::T0,
            vm: true
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vssubu_vi_rejected() {
    // vssubu has no VI form
    let inst = make_v_arith(0b100010, true, 2, 5, OPIVI, 1);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vssub_vv() {
    let inst = make_v_arith(0b100011, true, 2, 3, OPIVV, 1);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VssubVv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vssub_vx_masked() {
    let inst = make_v_arith(0b100011, false, 2, 5, OPIVX, 1);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VssubVx {
            vd: VReg::V1,
            vs2: VReg::V2,
            rs1: Reg::T0,
            vm: false
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vssub_vi_rejected() {
    // vssub has no VI form
    let inst = make_v_arith(0b100011, true, 2, 5, OPIVI, 1);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

// Averaging add/subtract

#[test]
#[cfg_attr(miri, ignore)]
fn test_vaaddu_vv() {
    let inst = make_v_arith(0b001000, true, 2, 3, OPMVV, 1);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VaadduVv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vaaddu_vx() {
    let inst = make_v_arith(0b001000, true, 2, 10, OPMVX, 1);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VaadduVx {
            vd: VReg::V1,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vaaddu_wrong_funct3() {
    // vaaddu uses OPMVV/OPMVX, not OPIVV
    let inst = make_v_arith(0b001000, true, 2, 3, OPIVV, 1);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vaadd_vv() {
    let inst = make_v_arith(0b001001, true, 4, 5, OPMVV, 6);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VaaddVv {
            vd: VReg::V6,
            vs2: VReg::V4,
            vs1: VReg::V5,
            vm: true
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vaadd_vx_masked() {
    let inst = make_v_arith(0b001001, false, 4, 11, OPMVX, 6);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VaaddVx {
            vd: VReg::V6,
            vs2: VReg::V4,
            rs1: Reg::A1,
            vm: false
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vasubu_vv() {
    let inst = make_v_arith(0b001010, true, 2, 3, OPMVV, 1);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VasubuVv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vasubu_vx() {
    let inst = make_v_arith(0b001010, true, 2, 5, OPMVX, 1);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VasubuVx {
            vd: VReg::V1,
            vs2: VReg::V2,
            rs1: Reg::T0,
            vm: true
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vasub_vv() {
    let inst = make_v_arith(0b001011, true, 2, 3, OPMVV, 1);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VasubVv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vasub_vx() {
    let inst = make_v_arith(0b001011, true, 2, 5, OPMVX, 1);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VasubVx {
            vd: VReg::V1,
            vs2: VReg::V2,
            rs1: Reg::T0,
            vm: true
        })
    );
}

// Fractional multiply

#[test]
#[cfg_attr(miri, ignore)]
fn test_vsmul_vv() {
    let inst = make_v_arith(0b100111, true, 2, 3, OPMVV, 1);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VsmulVv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vsmul_vx_masked() {
    let inst = make_v_arith(0b100111, false, 8, 10, OPMVX, 12);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VsmulVx {
            vd: VReg::V12,
            vs2: VReg::V8,
            rs1: Reg::A0,
            vm: false
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vsmul_vi_rejected() {
    // vsmul has no VI form
    let inst = make_v_arith(0b100111, true, 2, 3, OPIVI, 1);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

// Scaling shifts

#[test]
#[cfg_attr(miri, ignore)]
fn test_vssrl_vv() {
    let inst = make_v_arith(0b101000, true, 2, 3, OPIVV, 1);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VssrlVv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vssrl_vx() {
    let inst = make_v_arith(0b101000, true, 2, 5, OPIVX, 1);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VssrlVx {
            vd: VReg::V1,
            vs2: VReg::V2,
            rs1: Reg::T0,
            vm: true
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vssrl_vi() {
    // imm=7 (unsigned shift amount)
    let inst = make_v_arith(0b101000, true, 2, 7, OPIVI, 1);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VssrlVi {
            vd: VReg::V1,
            vs2: VReg::V2,
            imm: 7,
            vm: true
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vssra_vv() {
    let inst = make_v_arith(0b101001, true, 2, 3, OPIVV, 1);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VssraVv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vssra_vx_masked() {
    let inst = make_v_arith(0b101001, false, 8, 10, OPIVX, 12);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VssraVx {
            vd: VReg::V12,
            vs2: VReg::V8,
            rs1: Reg::A0,
            vm: false
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vssra_vi() {
    let inst = make_v_arith(0b101001, true, 4, 31, OPIVI, 8);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VssraVi {
            vd: VReg::V8,
            vs2: VReg::V4,
            imm: 31,
            vm: true
        })
    );
}

// Narrowing clips

#[test]
#[cfg_attr(miri, ignore)]
fn test_vnclipu_wv() {
    let inst = make_v_arith(0b101110, true, 2, 3, OPIVV, 1);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VnclipuWv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vnclipu_wx() {
    let inst = make_v_arith(0b101110, true, 2, 5, OPIVX, 1);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VnclipuWx {
            vd: VReg::V1,
            vs2: VReg::V2,
            rs1: Reg::T0,
            vm: true
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vnclipu_wi() {
    let inst = make_v_arith(0b101110, true, 2, 3, OPIVI, 1);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VnclipuWi {
            vd: VReg::V1,
            vs2: VReg::V2,
            imm: 3,
            vm: true
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vnclip_wv() {
    let inst = make_v_arith(0b101111, true, 4, 5, OPIVV, 6);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VnclipWv {
            vd: VReg::V6,
            vs2: VReg::V4,
            vs1: VReg::V5,
            vm: true
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vnclip_wx_masked() {
    let inst = make_v_arith(0b101111, false, 4, 11, OPIVX, 6);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VnclipWx {
            vd: VReg::V6,
            vs2: VReg::V4,
            rs1: Reg::A1,
            vm: false
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vnclip_wi() {
    let inst = make_v_arith(0b101111, true, 4, 0, OPIVI, 6);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VnclipWi {
            vd: VReg::V6,
            vs2: VReg::V4,
            imm: 0,
            vm: true
        })
    );
}

// Negative tests

#[test]
#[cfg_attr(miri, ignore)]
fn test_wrong_opcode() {
    // Use OP (0b0110011) instead of OP-V
    let funct7 = (0b100000u8 << 1) | 1;
    let inst = make_r_type(0b0110011, 1, OPIVV, 3, 2, funct7);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_unknown_funct6() {
    // funct6=0b111111 is not a fixed-point instruction
    let inst = make_v_arith(0b111111, true, 2, 3, OPIVV, 1);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vsmul_opivv_rejected() {
    // vsmul uses OPMVV/OPMVX, not OPIVV
    let inst = make_v_arith(0b100111, true, 2, 3, OPIVV, 1);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

// Display tests

#[test]
#[cfg_attr(miri, ignore)]
fn test_display_vsaddu_vv_unmasked() {
    let inst = make_v_arith(0b100000, true, 2, 3, OPIVV, 1);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vsaddu.vv v1, v2, v3");
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_display_vsaddu_vv_masked() {
    let inst = make_v_arith(0b100000, false, 2, 3, OPIVV, 1);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vsaddu.vv v1, v2, v3, v0.t");
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_display_vsadd_vx() {
    let inst = make_v_arith(0b100001, true, 8, 10, OPIVX, 12);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vsadd.vx v12, v8, a0");
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_display_vsaddu_vi_negative() {
    let inst = make_v_arith(0b100000, true, 2, 0b10000, OPIVI, 1);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vsaddu.vi v1, v2, -16");
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_display_vaadd_vv() {
    let inst = make_v_arith(0b001001, true, 4, 5, OPMVV, 6);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vaadd.vv v6, v4, v5");
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_display_vsmul_vx_masked() {
    let inst = make_v_arith(0b100111, false, 8, 10, OPMVX, 12);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vsmul.vx v12, v8, a0, v0.t");
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_display_vssrl_vi() {
    let inst = make_v_arith(0b101000, true, 2, 7, OPIVI, 1);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vssrl.vi v1, v2, 7");
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_display_vnclipu_wv() {
    let inst = make_v_arith(0b101110, true, 2, 3, OPIVV, 1);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vnclipu.wv v1, v2, v3");
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_display_vnclip_wx_masked() {
    let inst = make_v_arith(0b101111, false, 4, 11, OPIVX, 6);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vnclip.wx v6, v4, a1, v0.t");
}

// High register number tests

#[test]
#[cfg_attr(miri, ignore)]
fn test_vsaddu_vv_high_regs() {
    let inst = make_v_arith(0b100000, true, 31, 30, OPIVV, 29);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VsadduVv {
            vd: VReg::V29,
            vs2: VReg::V31,
            vs1: VReg::V30,
            vm: true
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vssra_vi_max_shift() {
    // max 5-bit unsigned immediate = 31
    let inst = make_v_arith(0b101001, true, 16, 31, OPIVI, 0);
    let decoded = Rv64Zve64xFixedPointInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xFixedPointInstruction::VssraVi {
            vd: VReg::V0,
            vs2: VReg::V16,
            imm: 31,
            vm: true
        })
    );
}
