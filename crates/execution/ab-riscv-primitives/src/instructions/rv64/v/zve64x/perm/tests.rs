extern crate alloc;

use crate::instructions::Instruction;
use crate::instructions::rv64::v::zve64x::perm::Rv64Zve64xPermInstruction;
use crate::instructions::test_utils::make_r_type;
use crate::registers::general_purpose::Reg;
use crate::registers::vector::VReg;
use alloc::format;

/// Build an OP-V instruction word from vector fields.
///
/// Maps to make_r_type where funct7 = (funct6 << 1) | vm.
/// Fields: vs2 maps to rs2 position, vs1/rs1/uimm maps to rs1 position,
/// vd/rd maps to rd position.
fn make_v_type(vd: u8, funct3: u8, vs1: u8, vs2: u8, vm: bool, funct6: u8) -> u32 {
    let funct7 = (funct6 << 1) | (vm as u8);
    make_r_type(0b1010111, vd, funct3, vs1, vs2, funct7)
}

// funct3 constants matching the spec
const OPIVV: u8 = 0b000;
const OPMVV: u8 = 0b010;
const OPIVI: u8 = 0b011;
const OPIVX: u8 = 0b100;
const OPMVX: u8 = 0b110;

// vmv.x.s

#[test]
fn test_vmv_x_s() {
    // funct6=010000, OPMVV, vs1=0, vm=1
    let inst = make_v_type(1, OPMVV, 0, 2, true, 0b010000);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xPermInstruction::VmvXS {
            rd: Reg::Ra,
            vs2: VReg::V2,
        })
    );
}

#[test]
fn test_vmv_x_s_different_regs() {
    let inst = make_v_type(10, OPMVV, 0, 16, true, 0b010000);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xPermInstruction::VmvXS {
            rd: Reg::A0,
            vs2: VReg::V16,
        })
    );
}

#[test]
fn test_vmv_x_s_rejects_nonzero_vs1() {
    let inst = make_v_type(1, OPMVV, 1, 2, true, 0b010000);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_vmv_x_s_rejects_vm_zero() {
    let inst = make_v_type(1, OPMVV, 0, 2, false, 0b010000);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

// vmv.s.x

#[test]
fn test_vmv_s_x() {
    // funct6=010000, OPMVX, vs2=0, vm=1
    let inst = make_v_type(3, OPMVX, 2, 0, true, 0b010000);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xPermInstruction::VmvSX {
            vd: VReg::V3,
            rs1: Reg::Sp,
        })
    );
}

#[test]
fn test_vmv_s_x_rejects_nonzero_vs2() {
    let inst = make_v_type(3, OPMVX, 2, 1, true, 0b010000);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_vmv_s_x_rejects_vm_zero() {
    let inst = make_v_type(3, OPMVX, 2, 0, false, 0b010000);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

// vrgather.vv

#[test]
fn test_vrgather_vv() {
    // funct6=001100, OPIVV
    let inst = make_v_type(1, OPIVV, 2, 3, true, 0b001100);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xPermInstruction::VrgatherVv {
            vd: VReg::V1,
            vs2: VReg::V3,
            vs1: VReg::V2,
            vm: true,
        })
    );
}

#[test]
fn test_vrgather_vv_masked() {
    let inst = make_v_type(8, OPIVV, 10, 12, false, 0b001100);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xPermInstruction::VrgatherVv {
            vd: VReg::V8,
            vs2: VReg::V12,
            vs1: VReg::V10,
            vm: false,
        })
    );
}

// vrgather.vx

#[test]
fn test_vrgather_vx() {
    // funct6=001100, OPIVX
    let inst = make_v_type(4, OPIVX, 5, 8, true, 0b001100);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xPermInstruction::VrgatherVx {
            vd: VReg::V4,
            vs2: VReg::V8,
            rs1: Reg::T0,
            vm: true,
        })
    );
}

#[test]
fn test_vrgather_vx_masked() {
    let inst = make_v_type(4, OPIVX, 5, 8, false, 0b001100);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xPermInstruction::VrgatherVx {
            vd: VReg::V4,
            vs2: VReg::V8,
            rs1: Reg::T0,
            vm: false,
        })
    );
}

// vrgather.vi

#[test]
fn test_vrgather_vi() {
    // funct6=001100, OPIVI, uimm=7
    let inst = make_v_type(4, OPIVI, 7, 8, true, 0b001100);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xPermInstruction::VrgatherVi {
            vd: VReg::V4,
            vs2: VReg::V8,
            uimm: 7,
            vm: true,
        })
    );
}

#[test]
fn test_vrgather_vi_max_uimm() {
    let inst = make_v_type(4, OPIVI, 31, 8, true, 0b001100);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xPermInstruction::VrgatherVi {
            vd: VReg::V4,
            vs2: VReg::V8,
            uimm: 31,
            vm: true,
        })
    );
}

// vrgatherei16.vv

#[test]
fn test_vrgatherei16_vv() {
    // funct6=001110, OPIVV
    let inst = make_v_type(1, OPIVV, 2, 3, true, 0b001110);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xPermInstruction::Vrgatherei16Vv {
            vd: VReg::V1,
            vs2: VReg::V3,
            vs1: VReg::V2,
            vm: true,
        })
    );
}

#[test]
fn test_vrgatherei16_vv_masked() {
    let inst = make_v_type(16, OPIVV, 20, 24, false, 0b001110);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xPermInstruction::Vrgatherei16Vv {
            vd: VReg::V16,
            vs2: VReg::V24,
            vs1: VReg::V20,
            vm: false,
        })
    );
}

// vslideup.vx

#[test]
fn test_vslideup_vx() {
    // funct6=001110, OPIVX
    let inst = make_v_type(4, OPIVX, 5, 8, true, 0b001110);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xPermInstruction::VslideupVx {
            vd: VReg::V4,
            vs2: VReg::V8,
            rs1: Reg::T0,
            vm: true,
        })
    );
}

#[test]
fn test_vslideup_vx_masked() {
    let inst = make_v_type(4, OPIVX, 5, 8, false, 0b001110);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xPermInstruction::VslideupVx {
            vd: VReg::V4,
            vs2: VReg::V8,
            rs1: Reg::T0,
            vm: false,
        })
    );
}

// vslideup.vi

#[test]
fn test_vslideup_vi() {
    // funct6=001110, OPIVI
    let inst = make_v_type(4, OPIVI, 3, 8, true, 0b001110);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xPermInstruction::VslideupVi {
            vd: VReg::V4,
            vs2: VReg::V8,
            uimm: 3,
            vm: true,
        })
    );
}

#[test]
fn test_vslideup_vi_masked() {
    let inst = make_v_type(4, OPIVI, 3, 8, false, 0b001110);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xPermInstruction::VslideupVi {
            vd: VReg::V4,
            vs2: VReg::V8,
            uimm: 3,
            vm: false,
        })
    );
}

// vslide1up.vx

#[test]
fn test_vslide1up_vx() {
    // funct6=001110, OPMVX
    let inst = make_v_type(4, OPMVX, 10, 8, true, 0b001110);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xPermInstruction::Vslide1upVx {
            vd: VReg::V4,
            vs2: VReg::V8,
            rs1: Reg::A0,
            vm: true,
        })
    );
}

#[test]
fn test_vslide1up_vx_masked() {
    let inst = make_v_type(4, OPMVX, 10, 8, false, 0b001110);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xPermInstruction::Vslide1upVx {
            vd: VReg::V4,
            vs2: VReg::V8,
            rs1: Reg::A0,
            vm: false,
        })
    );
}

// vslidedown.vx

#[test]
fn test_vslidedown_vx() {
    // funct6=001111, OPIVX
    let inst = make_v_type(4, OPIVX, 5, 8, true, 0b001111);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xPermInstruction::VslidedownVx {
            vd: VReg::V4,
            vs2: VReg::V8,
            rs1: Reg::T0,
            vm: true,
        })
    );
}

#[test]
fn test_vslidedown_vx_masked() {
    let inst = make_v_type(4, OPIVX, 5, 8, false, 0b001111);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xPermInstruction::VslidedownVx {
            vd: VReg::V4,
            vs2: VReg::V8,
            rs1: Reg::T0,
            vm: false,
        })
    );
}

// vslidedown.vi

#[test]
fn test_vslidedown_vi() {
    // funct6=001111, OPIVI
    let inst = make_v_type(4, OPIVI, 15, 8, true, 0b001111);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xPermInstruction::VslidedownVi {
            vd: VReg::V4,
            vs2: VReg::V8,
            uimm: 15,
            vm: true,
        })
    );
}

// vslide1down.vx

#[test]
fn test_vslide1down_vx() {
    // funct6=001111, OPMVX
    let inst = make_v_type(4, OPMVX, 10, 8, true, 0b001111);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xPermInstruction::Vslide1downVx {
            vd: VReg::V4,
            vs2: VReg::V8,
            rs1: Reg::A0,
            vm: true,
        })
    );
}

#[test]
fn test_vslide1down_vx_masked() {
    let inst = make_v_type(4, OPMVX, 10, 8, false, 0b001111);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xPermInstruction::Vslide1downVx {
            vd: VReg::V4,
            vs2: VReg::V8,
            rs1: Reg::A0,
            vm: false,
        })
    );
}

// vcompress.vm

#[test]
fn test_vcompress_vm() {
    // funct6=010111, OPMVV, vm=1
    let inst = make_v_type(1, OPMVV, 2, 3, true, 0b010111);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xPermInstruction::VcompressVm {
            vd: VReg::V1,
            vs2: VReg::V3,
            vs1: VReg::V2,
        })
    );
}

#[test]
fn test_vcompress_vm_different_regs() {
    let inst = make_v_type(16, OPMVV, 0, 24, true, 0b010111);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xPermInstruction::VcompressVm {
            vd: VReg::V16,
            vs2: VReg::V24,
            vs1: VReg::V0,
        })
    );
}

#[test]
fn test_vcompress_vm_rejects_vm_zero() {
    // vcompress requires vm=1
    let inst = make_v_type(1, OPMVV, 2, 3, false, 0b010111);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

// vmv1r.v

#[test]
fn test_vmv1r_v() {
    // funct6=100111, OPIVI, simm5=0, vm=1
    let inst = make_v_type(1, OPIVI, 0b00000, 2, true, 0b100111);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xPermInstruction::Vmv1rV {
            vd: VReg::V1,
            vs2: VReg::V2,
        })
    );
}

// vmv2r.v

#[test]
fn test_vmv2r_v() {
    // funct6=100111, OPIVI, simm5=1, vm=1
    let inst = make_v_type(2, OPIVI, 0b00001, 4, true, 0b100111);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xPermInstruction::Vmv2rV {
            vd: VReg::V2,
            vs2: VReg::V4,
        })
    );
}

// vmv4r.v

#[test]
fn test_vmv4r_v() {
    // funct6=100111, OPIVI, simm5=3, vm=1
    let inst = make_v_type(4, OPIVI, 0b00011, 8, true, 0b100111);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xPermInstruction::Vmv4rV {
            vd: VReg::V4,
            vs2: VReg::V8,
        })
    );
}

// vmv8r.v

#[test]
fn test_vmv8r_v() {
    // funct6=100111, OPIVI, simm5=7, vm=1
    let inst = make_v_type(8, OPIVI, 0b00111, 16, true, 0b100111);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xPermInstruction::Vmv8rV {
            vd: VReg::V8,
            vs2: VReg::V16,
        })
    );
}

#[test]
fn test_vmvnr_rejects_vm_zero() {
    // vmvNr.v requires vm=1
    let inst = make_v_type(1, OPIVI, 0b00000, 2, false, 0b100111);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_vmvnr_rejects_invalid_nr_hint() {
    // simm5=0b00010 is not a valid nr encoding
    let inst = make_v_type(1, OPIVI, 0b00010, 2, true, 0b100111);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_vmvnr_rejects_nr_hint_5() {
    // simm5=0b00101 is not valid
    let inst = make_v_type(1, OPIVI, 0b00101, 2, true, 0b100111);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_vmvnr_rejects_nr_hint_15() {
    // simm5=0b01111 is not valid
    let inst = make_v_type(1, OPIVI, 0b01111, 2, true, 0b100111);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

// Negative: wrong opcode

#[test]
fn test_wrong_opcode() {
    // Use LOAD-FP opcode instead of OP-V
    let inst = make_r_type(0b0000111, 1, OPMVV, 0, 2, (0b010000 << 1) | 1);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

// Negative: wrong funct3 for funct6

#[test]
fn test_vrgather_wrong_funct3() {
    // funct6=001100 with OPMVV should not match
    let inst = make_v_type(1, OPMVV, 2, 3, true, 0b001100);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_vcompress_wrong_funct3() {
    // funct6=010111 with OPIVV should not match (only OPMVV)
    let inst = make_v_type(1, OPIVV, 2, 3, true, 0b010111);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_vmv_x_s_wrong_funct3() {
    // funct6=010000 with OPIVV should not match
    let inst = make_v_type(1, OPIVV, 0, 2, true, 0b010000);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

// Negative: unrelated funct6

#[test]
fn test_unrelated_funct6() {
    // funct6=000000 (vadd) should not decode as perm
    let inst = make_v_type(1, OPIVV, 2, 3, true, 0b000000);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

// Display tests

#[test]
fn test_display_vmv_x_s() {
    let inst = make_v_type(1, OPMVV, 0, 8, true, 0b010000);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vmv.x.s ra, v8");
}

#[test]
fn test_display_vmv_s_x() {
    let inst = make_v_type(8, OPMVX, 1, 0, true, 0b010000);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vmv.s.x v8, ra");
}

#[test]
fn test_display_vrgather_vv_unmasked() {
    let inst = make_v_type(1, OPIVV, 2, 3, true, 0b001100);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vrgather.vv v1, v3, v2");
}

#[test]
fn test_display_vrgather_vv_masked() {
    let inst = make_v_type(1, OPIVV, 2, 3, false, 0b001100);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vrgather.vv v1, v3, v2, v0.t");
}

#[test]
fn test_display_vslideup_vx_unmasked() {
    let inst = make_v_type(4, OPIVX, 5, 8, true, 0b001110);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vslideup.vx v4, v8, t0");
}

#[test]
fn test_display_vslideup_vi_masked() {
    let inst = make_v_type(4, OPIVI, 3, 8, false, 0b001110);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vslideup.vi v4, v8, 3, v0.t");
}

#[test]
fn test_display_vslide1up_vx() {
    let inst = make_v_type(4, OPMVX, 10, 8, true, 0b001110);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vslide1up.vx v4, v8, a0");
}

#[test]
fn test_display_vslidedown_vi() {
    let inst = make_v_type(4, OPIVI, 15, 8, true, 0b001111);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vslidedown.vi v4, v8, 15");
}

#[test]
fn test_display_vslide1down_vx() {
    let inst = make_v_type(4, OPMVX, 10, 8, true, 0b001111);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vslide1down.vx v4, v8, a0");
}

#[test]
fn test_display_vcompress_vm() {
    let inst = make_v_type(1, OPMVV, 2, 3, true, 0b010111);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vcompress.vm v1, v3, v2");
}

#[test]
fn test_display_vmv1r_v() {
    let inst = make_v_type(1, OPIVI, 0, 2, true, 0b100111);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vmv1r.v v1, v2");
}

#[test]
fn test_display_vmv2r_v() {
    let inst = make_v_type(2, OPIVI, 1, 4, true, 0b100111);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vmv2r.v v2, v4");
}

#[test]
fn test_display_vmv4r_v() {
    let inst = make_v_type(4, OPIVI, 3, 8, true, 0b100111);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vmv4r.v v4, v8");
}

#[test]
fn test_display_vmv8r_v() {
    let inst = make_v_type(8, OPIVI, 7, 16, true, 0b100111);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vmv8r.v v8, v16");
}

#[test]
fn test_display_vrgather_vx_masked() {
    let inst = make_v_type(4, OPIVX, 5, 8, false, 0b001100);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vrgather.vx v4, v8, t0, v0.t");
}

#[test]
fn test_display_vrgatherei16_vv() {
    let inst = make_v_type(1, OPIVV, 2, 3, true, 0b001110);
    let decoded = Rv64Zve64xPermInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vrgatherei16.vv v1, v3, v2");
}
