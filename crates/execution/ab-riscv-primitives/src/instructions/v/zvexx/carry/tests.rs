extern crate alloc;

use crate::instructions::Instruction;
use crate::instructions::test_utils::make_r_type;
use crate::instructions::v::zvexx::carry::ZveXxCarryInstruction;
use crate::registers::general_purpose::Reg;
use crate::registers::vector::VReg;
use alloc::format;

/// Build a carry-class OP-V instruction word.
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

// vadd (funct6=0b00_0000) must NOT decode as any carry instruction - regression guard
// for the false-positive that triggered this fix.

#[test]
fn non_carry_funct6_not_decoded() {
    let inst = make_vop(0b00_0000, 1, 2, 3, OPIVV, 1);
    assert_eq!(ZveXxCarryInstruction::<Reg<u64>>::try_decode(inst), None);
}

// False-decode regression: funct6=0b010000, OPIVV with vs1=v17 (0b10001)
// previously matched vfirst.m in the mask decoder.

#[test]
fn vadc_vvm_not_falsely_decoded_as_vfirst_m() {
    // funct6=0b01_0000, vm=0, vs2=v9, vs1=v17 (0b10001), OPIVV, vd=v12
    let inst = make_vop(0b01_0000, 0, 9, 17, OPIVV, 12);
    let decoded = ZveXxCarryInstruction::<Reg<u64>>::try_decode(inst);
    assert!(matches!(
        decoded,
        Some(ZveXxCarryInstruction::VadcVvm {
            vd: VReg::V12,
            vs2: VReg::V9,
            vs1: VReg::V17,
            ..
        })
    ));
}

// vadc

#[test]
fn vadc_vvm_basic() {
    let inst = make_vop(0b01_0000, 0, 2, 3, OPIVV, 1);
    let decoded = ZveXxCarryInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxCarryInstruction::VadcVvm {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn vadc_vxm_basic() {
    let inst = make_vop(0b01_0000, 0, 4, 10, OPIVX, 6);
    let decoded = ZveXxCarryInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxCarryInstruction::VadcVxm {
            vd: VReg::V6,
            vs2: VReg::V4,
            rs1: Reg::A0,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn vadc_vim_positive() {
    // imm = 5 (0b00101)
    let inst = make_vop(0b01_0000, 0, 8, 5, OPIVI, 2);
    let decoded = ZveXxCarryInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxCarryInstruction::VadcVim {
            vd: VReg::V2,
            vs2: VReg::V8,
            imm: 5,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn vadc_vim_negative() {
    // imm = -1 -> 5-bit = 0b11111 = 31
    let inst = make_vop(0b01_0000, 0, 8, 0b11111, OPIVI, 2);
    let decoded = ZveXxCarryInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxCarryInstruction::VadcVim {
            vd: VReg::V2,
            vs2: VReg::V8,
            imm: -1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

// vadc vm=1 is reserved - must not decode

#[test]
fn vadc_vvm_vm1_is_reserved() {
    let inst = make_vop(0b01_0000, 1, 2, 3, OPIVV, 1);
    assert_eq!(ZveXxCarryInstruction::<Reg<u64>>::try_decode(inst), None);
}

#[test]
fn vadc_vxm_vm1_is_reserved() {
    let inst = make_vop(0b01_0000, 1, 2, 5, OPIVX, 1);
    assert_eq!(ZveXxCarryInstruction::<Reg<u64>>::try_decode(inst), None);
}

// vmadc

#[test]
fn vmadc_vvm_with_carry() {
    let inst = make_vop(0b01_0001, 0, 4, 5, OPIVV, 3);
    let decoded = ZveXxCarryInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxCarryInstruction::VmadcVvm {
            vd: VReg::V3,
            vs2: VReg::V4,
            vs1: VReg::V5,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn vmadc_vv_no_carry() {
    let inst = make_vop(0b01_0001, 1, 4, 5, OPIVV, 3);
    let decoded = ZveXxCarryInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxCarryInstruction::VmadcVv {
            vd: VReg::V3,
            vs2: VReg::V4,
            vs1: VReg::V5,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn vmadc_vxm_with_carry() {
    let inst = make_vop(0b01_0001, 0, 8, 11, OPIVX, 0);
    let decoded = ZveXxCarryInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxCarryInstruction::VmadcVxm {
            vd: VReg::V0,
            vs2: VReg::V8,
            rs1: Reg::A1,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn vmadc_vx_no_carry() {
    let inst = make_vop(0b01_0001, 1, 8, 11, OPIVX, 0);
    let decoded = ZveXxCarryInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxCarryInstruction::VmadcVx {
            vd: VReg::V0,
            vs2: VReg::V8,
            rs1: Reg::A1,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn vmadc_vim_with_carry() {
    // imm = -16 -> 5-bit = 0b10000 = 16
    let inst = make_vop(0b01_0001, 0, 6, 0b10000, OPIVI, 2);
    let decoded = ZveXxCarryInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxCarryInstruction::VmadcVim {
            vd: VReg::V2,
            vs2: VReg::V6,
            imm: -16,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn vmadc_vi_no_carry() {
    let inst = make_vop(0b01_0001, 1, 6, 7, OPIVI, 2);
    let decoded = ZveXxCarryInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxCarryInstruction::VmadcVi {
            vd: VReg::V2,
            vs2: VReg::V6,
            imm: 7,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

// vsbc - no immediate form

#[test]
fn vsbc_vvm_basic() {
    let inst = make_vop(0b01_0010, 0, 10, 11, OPIVV, 8);
    let decoded = ZveXxCarryInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxCarryInstruction::VsbcVvm {
            vd: VReg::V8,
            vs2: VReg::V10,
            vs1: VReg::V11,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn vsbc_vxm_basic() {
    let inst = make_vop(0b01_0010, 0, 12, 5, OPIVX, 16);
    let decoded = ZveXxCarryInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxCarryInstruction::VsbcVxm {
            vd: VReg::V16,
            vs2: VReg::V12,
            rs1: Reg::T0,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn vsbc_has_no_immediate_form() {
    let inst = make_vop(0b01_0010, 0, 2, 5, OPIVI, 1);
    assert_eq!(ZveXxCarryInstruction::<Reg<u64>>::try_decode(inst), None);
}

#[test]
fn vsbc_vm1_is_reserved() {
    let inst = make_vop(0b01_0010, 1, 2, 3, OPIVV, 1);
    assert_eq!(ZveXxCarryInstruction::<Reg<u64>>::try_decode(inst), None);
}

// vmsbc

#[test]
fn vmsbc_vvm_with_borrow() {
    let inst = make_vop(0b01_0011, 0, 14, 15, OPIVV, 0);
    let decoded = ZveXxCarryInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxCarryInstruction::VmsbcVvm {
            vd: VReg::V0,
            vs2: VReg::V14,
            vs1: VReg::V15,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn vmsbc_vv_no_borrow() {
    let inst = make_vop(0b01_0011, 1, 14, 15, OPIVV, 0);
    let decoded = ZveXxCarryInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxCarryInstruction::VmsbcVv {
            vd: VReg::V0,
            vs2: VReg::V14,
            vs1: VReg::V15,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn vmsbc_vxm_with_borrow() {
    let inst = make_vop(0b01_0011, 0, 16, 10, OPIVX, 0);
    let decoded = ZveXxCarryInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxCarryInstruction::VmsbcVxm {
            vd: VReg::V0,
            vs2: VReg::V16,
            rs1: Reg::A0,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn vmsbc_vx_no_borrow() {
    let inst = make_vop(0b01_0011, 1, 16, 10, OPIVX, 0);
    let decoded = ZveXxCarryInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxCarryInstruction::VmsbcVx {
            vd: VReg::V0,
            vs2: VReg::V16,
            rs1: Reg::A0,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn vmsbc_has_no_immediate_form() {
    let inst = make_vop(0b01_0011, 0, 2, 5, OPIVI, 1);
    assert_eq!(ZveXxCarryInstruction::<Reg<u64>>::try_decode(inst), None);
}

// High register numbers

#[test]
fn vadc_high_regs() {
    let inst = make_vop(0b01_0000, 0, 31, 30, OPIVV, 29);
    let decoded = ZveXxCarryInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxCarryInstruction::VadcVvm {
            vd: VReg::V29,
            vs2: VReg::V31,
            vs1: VReg::V30,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

// Wrong opcode

#[test]
fn wrong_opcode_not_decoded() {
    let funct7 = 0b01_0000 << 1u8;
    let inst = make_r_type(0b011_0011, 1, OPIVV, 2, 3, funct7);
    assert_eq!(ZveXxCarryInstruction::<Reg<u64>>::try_decode(inst), None);
}

// OPMVV (funct3=0b010) - carry decoder must NOT claim these (mask decoder owns them)

#[test]
fn opmvv_funct3_not_claimed() {
    // funct6=0b010000, OPMVV - this is vcpop/vfirst territory
    let inst = make_vop(0b01_0000, 0, 4, 0b10000, 0b010, 1);
    assert_eq!(ZveXxCarryInstruction::<Reg<u64>>::try_decode(inst), None);
}

// Display

#[test]
fn display_vadc_vvm() {
    let inst = make_vop(0b01_0000, 0, 2, 3, OPIVV, 1);
    let decoded = ZveXxCarryInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vadc.vvm v1, v2, v3, v0");
}

#[test]
fn display_vadc_vxm() {
    let inst = make_vop(0b01_0000, 0, 4, 10, OPIVX, 6);
    let decoded = ZveXxCarryInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vadc.vxm v6, v4, a0, v0");
}

#[test]
fn display_vadc_vim_negative() {
    let inst = make_vop(0b01_0000, 0, 8, 0b11111, OPIVI, 2);
    let decoded = ZveXxCarryInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vadc.vim v2, v8, -1, v0");
}

#[test]
fn display_vmadc_vvm() {
    let inst = make_vop(0b01_0001, 0, 4, 5, OPIVV, 3);
    let decoded = ZveXxCarryInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vmadc.vvm v3, v4, v5, v0");
}

#[test]
fn display_vmadc_vv_no_carry() {
    let inst = make_vop(0b01_0001, 1, 4, 5, OPIVV, 3);
    let decoded = ZveXxCarryInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vmadc.vv v3, v4, v5");
}

#[test]
fn display_vsbc_vvm() {
    let inst = make_vop(0b01_0010, 0, 10, 11, OPIVV, 8);
    let decoded = ZveXxCarryInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vsbc.vvm v8, v10, v11, v0");
}

#[test]
fn display_vmsbc_vv_no_borrow() {
    let inst = make_vop(0b01_0011, 1, 14, 15, OPIVV, 0);
    let decoded = ZveXxCarryInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vmsbc.vv v0, v14, v15");
}
