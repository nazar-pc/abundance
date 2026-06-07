extern crate alloc;

use crate::instructions::Instruction;
use crate::instructions::test_utils::make_r_type;
use crate::instructions::v::zvexx::arith::ZveXxArithInstruction;
use crate::registers::general_purpose::Reg;
use crate::registers::vector::VReg;
use alloc::format;

/// Construct a vector arithmetic instruction word.
///
/// The vector arithmetic format maps onto make_r_type as:
/// `make_r_type(opcode=0b101_0111, vd, funct3, vs1_or_rs1_or_imm, vs2, (funct6<<1)|vm)`
///
/// vm=1 means unmasked, vm=0 means masked (v0.t).
fn make_vop(funct6: u8, vm: u8, vs2: u8, vs1: u8, funct3: u8, vd: u8) -> u32 {
    let funct7 = (funct6 << 1u8) | vm;
    make_r_type(0b101_0111, vd, funct3, vs1, vs2, funct7)
}

const OPIVV: u8 = 0b000;
const OPIVX: u8 = 0b100;
const OPIVI: u8 = 0b011;

// vadd

#[test]
fn test_vadd_vv() {
    let inst = make_vop(0b00_0000, 1, 2, 3, OPIVV, 1);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VaddVv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vadd_vv_masked() {
    let inst = make_vop(0b00_0000, 0, 4, 5, OPIVV, 6);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VaddVv {
            vd: VReg::V6,
            vs2: VReg::V4,
            vs1: VReg::V5,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vadd_vx() {
    let inst = make_vop(0b00_0000, 1, 2, 5, OPIVX, 1);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VaddVx {
            vd: VReg::V1,
            vs2: VReg::V2,
            rs1: Reg::T0,
            vm: true,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vadd_vi_positive() {
    // imm = 5 (0b0_0101)
    let inst = make_vop(0b00_0000, 1, 8, 5, OPIVI, 1);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VaddVi {
            vd: VReg::V1,
            vs2: VReg::V8,
            imm: 5,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vadd_vi_negative() {
    // imm = -1 => 5-bit = 0b11111 = 31
    let inst = make_vop(0b00_0000, 1, 8, 0b11111, OPIVI, 1);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VaddVi {
            vd: VReg::V1,
            vs2: VReg::V8,
            imm: -1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vadd_vi_min_imm() {
    // imm = -16 => 5-bit = 0b10000 = 16
    let inst = make_vop(0b00_0000, 1, 4, 0b10000, OPIVI, 2);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VaddVi {
            vd: VReg::V2,
            vs2: VReg::V4,
            imm: -16,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

// vsub

#[test]
fn test_vsub_vv() {
    let inst = make_vop(0b00_0010, 1, 2, 3, OPIVV, 1);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VsubVv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vsub_vx() {
    let inst = make_vop(0b00_0010, 1, 2, 10, OPIVX, 1);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VsubVx {
            vd: VReg::V1,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        })
    );
}

// vrsub

#[test]
fn test_vrsub_vx() {
    let inst = make_vop(0b00_0011, 1, 2, 5, OPIVX, 1);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VrsubVx {
            vd: VReg::V1,
            vs2: VReg::V2,
            rs1: Reg::T0,
            vm: true,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vrsub_vi() {
    let inst = make_vop(0b00_0011, 1, 2, 0, OPIVI, 1);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VrsubVi {
            vd: VReg::V1,
            vs2: VReg::V2,
            imm: 0,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

// vand

#[test]
fn test_vand_vv() {
    let inst = make_vop(0b00_1001, 1, 8, 9, OPIVV, 10);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VandVv {
            vd: VReg::V10,
            vs2: VReg::V8,
            vs1: VReg::V9,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vand_vx() {
    let inst = make_vop(0b00_1001, 1, 8, 7, OPIVX, 10);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VandVx {
            vd: VReg::V10,
            vs2: VReg::V8,
            rs1: Reg::T2,
            vm: true,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vand_vi() {
    let inst = make_vop(0b00_1001, 1, 4, 0b01111, OPIVI, 2);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VandVi {
            vd: VReg::V2,
            vs2: VReg::V4,
            imm: 15,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

// vor

#[test]
fn test_vor_vv() {
    let inst = make_vop(0b00_1010, 1, 2, 3, OPIVV, 1);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VorVv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vor_vx() {
    let inst = make_vop(0b00_1010, 1, 2, 3, OPIVX, 1);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VorVx {
            vd: VReg::V1,
            vs2: VReg::V2,
            rs1: Reg::Gp,
            vm: true,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vor_vi() {
    let inst = make_vop(0b00_1010, 1, 2, 0b11111, OPIVI, 1);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VorVi {
            vd: VReg::V1,
            vs2: VReg::V2,
            imm: -1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

// vxor

#[test]
fn test_vxor_vv() {
    let inst = make_vop(0b00_1011, 1, 2, 3, OPIVV, 1);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VxorVv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vxor_vx() {
    let inst = make_vop(0b00_1011, 1, 2, 3, OPIVX, 1);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VxorVx {
            vd: VReg::V1,
            vs2: VReg::V2,
            rs1: Reg::Gp,
            vm: true,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vxor_vi() {
    let inst = make_vop(0b00_1011, 1, 2, 0b11111, OPIVI, 1);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VxorVi {
            vd: VReg::V1,
            vs2: VReg::V2,
            imm: -1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

// vsll

#[test]
fn test_vsll_vv() {
    let inst = make_vop(0b10_0101, 1, 2, 3, OPIVV, 1);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VsllVv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vsll_vx() {
    let inst = make_vop(0b10_0101, 1, 2, 5, OPIVX, 1);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VsllVx {
            vd: VReg::V1,
            vs2: VReg::V2,
            rs1: Reg::T0,
            vm: true,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vsll_vi() {
    // uimm = 8
    let inst = make_vop(0b10_0101, 1, 16, 8, OPIVI, 24);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VsllVi {
            vd: VReg::V24,
            vs2: VReg::V16,
            uimm: 8,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vsll_vi_max_uimm() {
    // uimm = 31 (max 5-bit unsigned)
    let inst = make_vop(0b10_0101, 1, 4, 31, OPIVI, 2);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VsllVi {
            vd: VReg::V2,
            vs2: VReg::V4,
            uimm: 31,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

// vsrl

#[test]
fn test_vsrl_vv() {
    let inst = make_vop(0b10_1000, 1, 2, 3, OPIVV, 1);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VsrlVv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vsrl_vx() {
    let inst = make_vop(0b10_1000, 1, 8, 6, OPIVX, 8);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VsrlVx {
            vd: VReg::V8,
            vs2: VReg::V8,
            rs1: Reg::T1,
            vm: true,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vsrl_vi() {
    // uimm = 3
    let inst = make_vop(0b10_1000, 1, 8, 3, OPIVI, 8);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VsrlVi {
            vd: VReg::V8,
            vs2: VReg::V8,
            uimm: 3,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

// vsra

#[test]
fn test_vsra_vv() {
    let inst = make_vop(0b10_1001, 1, 2, 3, OPIVV, 1);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VsraVv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vsra_vi() {
    let inst = make_vop(0b10_1001, 1, 4, 7, OPIVI, 2);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VsraVi {
            vd: VReg::V2,
            vs2: VReg::V4,
            uimm: 7,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

// vminu/vmin/vmaxu/vmax

#[test]
fn test_vminu_vv() {
    let inst = make_vop(0b00_0100, 1, 2, 3, OPIVV, 1);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VminuVv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vminu_vx() {
    let inst = make_vop(0b00_0100, 1, 2, 10, OPIVX, 1);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VminuVx {
            vd: VReg::V1,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vmin_vv() {
    let inst = make_vop(0b00_0101, 1, 2, 3, OPIVV, 1);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VminVv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vmin_vx() {
    let inst = make_vop(0b00_0101, 1, 2, 10, OPIVX, 1);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VminVx {
            vd: VReg::V1,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vmaxu_vv() {
    let inst = make_vop(0b00_0110, 1, 2, 3, OPIVV, 1);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VmaxuVv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vmaxu_vx_masked() {
    let inst = make_vop(0b00_0110, 0, 2, 10, OPIVX, 1);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VmaxuVx {
            vd: VReg::V1,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: false,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vmax_vv() {
    let inst = make_vop(0b00_0111, 1, 2, 3, OPIVV, 1);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VmaxVv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vmax_vx() {
    let inst = make_vop(0b00_0111, 1, 2, 10, OPIVX, 1);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VmaxVx {
            vd: VReg::V1,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        })
    );
}

// vmseq/vmsne

#[test]
fn test_vmseq_vv() {
    let inst = make_vop(0b01_1000, 1, 2, 3, OPIVV, 0);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VmseqVv {
            vd: VReg::V0,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vmseq_vx() {
    let inst = make_vop(0b01_1000, 1, 2, 5, OPIVX, 0);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VmseqVx {
            vd: VReg::V0,
            vs2: VReg::V2,
            rs1: Reg::T0,
            vm: true,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vmseq_vi() {
    let inst = make_vop(0b01_1000, 1, 2, 0, OPIVI, 0);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VmseqVi {
            vd: VReg::V0,
            vs2: VReg::V2,
            imm: 0,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vmsne_vv() {
    let inst = make_vop(0b01_1001, 1, 8, 6, OPIVV, 16);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VmsneVv {
            vd: VReg::V16,
            vs2: VReg::V8,
            vs1: VReg::V6,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vmsne_vi() {
    // imm = 0b10 = 2 (5-bit sign-extended)
    let inst = make_vop(0b01_1001, 1, 8, 0b00010, OPIVI, 16);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VmsneVi {
            vd: VReg::V16,
            vs2: VReg::V8,
            imm: 2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

// vmsltu/vmslt

#[test]
fn test_vmsltu_vv() {
    let inst = make_vop(0b01_1010, 1, 2, 3, OPIVV, 0);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VmsltuVv {
            vd: VReg::V0,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vmsltu_vx() {
    let inst = make_vop(0b01_1010, 1, 2, 5, OPIVX, 0);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VmsltuVx {
            vd: VReg::V0,
            vs2: VReg::V2,
            rs1: Reg::T0,
            vm: true,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vmslt_vv() {
    let inst = make_vop(0b01_1011, 1, 2, 3, OPIVV, 0);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VmsltVv {
            vd: VReg::V0,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vmslt_vx() {
    let inst = make_vop(0b01_1011, 1, 2, 5, OPIVX, 0);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VmsltVx {
            vd: VReg::V0,
            vs2: VReg::V2,
            rs1: Reg::T0,
            vm: true,
            rs2: Reg::Zero,
        })
    );
}

// vmsleu/vmsle

#[test]
fn test_vmsleu_vv() {
    let inst = make_vop(0b01_1100, 1, 2, 3, OPIVV, 0);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VmsleuVv {
            vd: VReg::V0,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vmsleu_vx() {
    let inst = make_vop(0b01_1100, 1, 2, 5, OPIVX, 0);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VmsleuVx {
            vd: VReg::V0,
            vs2: VReg::V2,
            rs1: Reg::T0,
            vm: true,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vmsleu_vi() {
    let inst = make_vop(0b01_1100, 1, 2, 15, OPIVI, 0);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VmsleuVi {
            vd: VReg::V0,
            vs2: VReg::V2,
            imm: 15,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vmsle_vv() {
    let inst = make_vop(0b01_1101, 1, 2, 3, OPIVV, 0);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VmsleVv {
            vd: VReg::V0,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vmsle_vi() {
    let inst = make_vop(0b01_1101, 1, 2, 0b11110, OPIVI, 0);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VmsleVi {
            vd: VReg::V0,
            vs2: VReg::V2,
            imm: -2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

// vmsgtu/vmsgt (OPIVX and OPIVI only)

#[test]
fn test_vmsgtu_vx() {
    let inst = make_vop(0b01_1110, 1, 2, 10, OPIVX, 0);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VmsgtuVx {
            vd: VReg::V0,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vmsgtu_vi() {
    let inst = make_vop(0b01_1110, 1, 2, 9, OPIVI, 0);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VmsgtuVi {
            vd: VReg::V0,
            vs2: VReg::V2,
            imm: 9,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vmsgt_vx() {
    let inst = make_vop(0b01_1111, 1, 2, 10, OPIVX, 0);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VmsgtVx {
            vd: VReg::V0,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_vmsgt_vi() {
    let inst = make_vop(0b01_1111, 1, 2, 0b11100, OPIVI, 0);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VmsgtVi {
            vd: VReg::V0,
            vs2: VReg::V2,
            imm: -4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

// Negative tests

#[test]
fn test_wrong_opcode() {
    // Use OP (0b011_0011) instead of OP-V
    let funct7 = 1;
    let inst = make_r_type(0b011_0011, 1, OPIVV, 2, 3, funct7);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_wrong_funct3_opcfg() {
    // funct3=0b111 (OPCFG) should not be decoded as arith
    let funct7 = 1;
    let inst = make_r_type(0b101_0111, 1, 0b111, 2, 3, funct7);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_unknown_funct6_opivv() {
    // funct6=0b111111 is not assigned in OPIVV
    let inst = make_vop(0b11_1111, 1, 2, 3, OPIVV, 1);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_vsub_has_no_vi() {
    // vsub only has .vv and .vx, not .vi
    let inst = make_vop(0b00_0010, 1, 2, 3, OPIVI, 1);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_vmsltu_has_no_vi() {
    // vmsltu only has .vv and .vx per spec
    let inst = make_vop(0b01_1010, 1, 2, 3, OPIVI, 1);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_vmsgtu_has_no_vv() {
    // vmsgtu only has .vx and .vi, not .vv
    let inst = make_vop(0b01_1110, 1, 2, 3, OPIVV, 0);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_vmsgt_has_no_vv() {
    let inst = make_vop(0b01_1111, 1, 2, 3, OPIVV, 0);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

// High vector register numbers

#[test]
fn test_vadd_vv_high_regs() {
    let inst = make_vop(0b00_0000, 1, 31, 30, OPIVV, 29);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZveXxArithInstruction::VaddVv {
            vd: VReg::V29,
            vs2: VReg::V31,
            vs1: VReg::V30,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

// Display tests

#[test]
fn test_display_vadd_vv_unmasked() {
    let inst = make_vop(0b00_0000, 1, 2, 3, OPIVV, 1);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vadd.vv v1, v2, v3");
}

#[test]
fn test_display_vadd_vv_masked() {
    let inst = make_vop(0b00_0000, 0, 2, 3, OPIVV, 1);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vadd.vv v1, v2, v3, v0.t");
}

#[test]
fn test_display_vadd_vx() {
    let inst = make_vop(0b00_0000, 1, 2, 5, OPIVX, 1);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vadd.vx v1, v2, t0");
}

#[test]
fn test_display_vadd_vi() {
    let inst = make_vop(0b00_0000, 1, 2, 0b11111, OPIVI, 1);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vadd.vi v1, v2, -1");
}

#[test]
fn test_display_vsll_vi() {
    let inst = make_vop(0b10_0101, 1, 16, 8, OPIVI, 24);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vsll.vi v24, v16, 8");
}

#[test]
fn test_display_vmseq_vi_masked() {
    let inst = make_vop(0b01_1000, 0, 2, 0, OPIVI, 0);
    let decoded = ZveXxArithInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vmseq.vi v0, v2, 0, v0.t");
}
