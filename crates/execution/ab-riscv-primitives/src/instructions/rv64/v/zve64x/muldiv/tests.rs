extern crate alloc;

use crate::instructions::Instruction;
use crate::instructions::rv64::v::zve64x::muldiv::Rv64Zve64xMulDivInstruction;
use crate::instructions::test_utils::make_r_type;
use crate::registers::general_purpose::Reg;
use crate::registers::vector::VReg;
use alloc::format;

/// OP-V major opcode
const OP_V: u8 = 0b1010111;
/// OPMVV funct3
const OPMVV: u8 = 0b010;
/// OPMVX funct3
const OPMVX: u8 = 0b110;

/// Build funct7 from funct6 and vm bit
/// vm=true means unmasked (bit=1), vm=false means masked (bit=0)
const fn funct7(funct6: u8, vm: bool) -> u8 {
    (funct6 << 1) | (vm as u8)
}

// Single-width integer multiply (Section 12.10)

#[test]
fn test_vmul_vv() {
    let inst = make_r_type(OP_V, 1, OPMVV, 2, 3, funct7(0b100101, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VmulVv {
            vd: VReg::V1,
            vs2: VReg::V3,
            vs1: VReg::V2,
            vm: true,
        })
    );
}

#[test]
fn test_vmul_vv_masked() {
    let inst = make_r_type(OP_V, 4, OPMVV, 5, 6, funct7(0b100101, false));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VmulVv {
            vd: VReg::V4,
            vs2: VReg::V6,
            vs1: VReg::V5,
            vm: false,
        })
    );
}

#[test]
fn test_vmul_vx() {
    let inst = make_r_type(OP_V, 1, OPMVX, 2, 3, funct7(0b100101, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VmulVx {
            vd: VReg::V1,
            vs2: VReg::V3,
            rs1: Reg::Sp,
            vm: true,
        })
    );
}

#[test]
fn test_vmulh_vv() {
    let inst = make_r_type(OP_V, 8, OPMVV, 9, 10, funct7(0b100111, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VmulhVv {
            vd: VReg::V8,
            vs2: VReg::V10,
            vs1: VReg::V9,
            vm: true,
        })
    );
}

#[test]
fn test_vmulh_vx() {
    let inst = make_r_type(OP_V, 8, OPMVX, 10, 12, funct7(0b100111, false));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VmulhVx {
            vd: VReg::V8,
            vs2: VReg::V12,
            rs1: Reg::A0,
            vm: false,
        })
    );
}

#[test]
fn test_vmulhu_vv() {
    let inst = make_r_type(OP_V, 1, OPMVV, 2, 3, funct7(0b100100, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VmulhuVv {
            vd: VReg::V1,
            vs2: VReg::V3,
            vs1: VReg::V2,
            vm: true,
        })
    );
}

#[test]
fn test_vmulhu_vx() {
    let inst = make_r_type(OP_V, 1, OPMVX, 2, 3, funct7(0b100100, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VmulhuVx {
            vd: VReg::V1,
            vs2: VReg::V3,
            rs1: Reg::Sp,
            vm: true,
        })
    );
}

#[test]
fn test_vmulhsu_vv() {
    let inst = make_r_type(OP_V, 1, OPMVV, 2, 3, funct7(0b100110, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VmulhsuVv {
            vd: VReg::V1,
            vs2: VReg::V3,
            vs1: VReg::V2,
            vm: true,
        })
    );
}

#[test]
fn test_vmulhsu_vx() {
    let inst = make_r_type(OP_V, 1, OPMVX, 2, 3, funct7(0b100110, false));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VmulhsuVx {
            vd: VReg::V1,
            vs2: VReg::V3,
            rs1: Reg::Sp,
            vm: false,
        })
    );
}

// Integer divide (Section 12.11)

#[test]
fn test_vdivu_vv() {
    let inst = make_r_type(OP_V, 1, OPMVV, 2, 3, funct7(0b100000, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VdivuVv {
            vd: VReg::V1,
            vs2: VReg::V3,
            vs1: VReg::V2,
            vm: true,
        })
    );
}

#[test]
fn test_vdivu_vx() {
    let inst = make_r_type(OP_V, 1, OPMVX, 2, 3, funct7(0b100000, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VdivuVx {
            vd: VReg::V1,
            vs2: VReg::V3,
            rs1: Reg::Sp,
            vm: true,
        })
    );
}

#[test]
fn test_vdiv_vv() {
    let inst = make_r_type(OP_V, 1, OPMVV, 2, 3, funct7(0b100001, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VdivVv {
            vd: VReg::V1,
            vs2: VReg::V3,
            vs1: VReg::V2,
            vm: true,
        })
    );
}

#[test]
fn test_vdiv_vx_masked() {
    let inst = make_r_type(OP_V, 16, OPMVX, 17, 18, funct7(0b100001, false));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VdivVx {
            vd: VReg::V16,
            vs2: VReg::V18,
            rs1: Reg::A7,
            vm: false,
        })
    );
}

#[test]
fn test_vremu_vv() {
    let inst = make_r_type(OP_V, 1, OPMVV, 2, 3, funct7(0b100010, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VremuVv {
            vd: VReg::V1,
            vs2: VReg::V3,
            vs1: VReg::V2,
            vm: true,
        })
    );
}

#[test]
fn test_vremu_vx() {
    let inst = make_r_type(OP_V, 1, OPMVX, 2, 3, funct7(0b100010, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VremuVx {
            vd: VReg::V1,
            vs2: VReg::V3,
            rs1: Reg::Sp,
            vm: true,
        })
    );
}

#[test]
fn test_vrem_vv() {
    let inst = make_r_type(OP_V, 1, OPMVV, 2, 3, funct7(0b100011, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VremVv {
            vd: VReg::V1,
            vs2: VReg::V3,
            vs1: VReg::V2,
            vm: true,
        })
    );
}

#[test]
fn test_vrem_vx() {
    let inst = make_r_type(OP_V, 1, OPMVX, 2, 3, funct7(0b100011, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VremVx {
            vd: VReg::V1,
            vs2: VReg::V3,
            rs1: Reg::Sp,
            vm: true,
        })
    );
}

// Widening integer multiply (Section 12.12)

#[test]
fn test_vwmulu_vv() {
    let inst = make_r_type(OP_V, 2, OPMVV, 4, 6, funct7(0b111000, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VwmuluVv {
            vd: VReg::V2,
            vs2: VReg::V6,
            vs1: VReg::V4,
            vm: true,
        })
    );
}

#[test]
fn test_vwmulu_vx() {
    let inst = make_r_type(OP_V, 2, OPMVX, 5, 6, funct7(0b111000, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VwmuluVx {
            vd: VReg::V2,
            vs2: VReg::V6,
            rs1: Reg::T0,
            vm: true,
        })
    );
}

#[test]
fn test_vwmulsu_vv() {
    let inst = make_r_type(OP_V, 2, OPMVV, 4, 6, funct7(0b111010, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VwmulsuVv {
            vd: VReg::V2,
            vs2: VReg::V6,
            vs1: VReg::V4,
            vm: true,
        })
    );
}

#[test]
fn test_vwmulsu_vx() {
    let inst = make_r_type(OP_V, 2, OPMVX, 5, 6, funct7(0b111010, false));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VwmulsuVx {
            vd: VReg::V2,
            vs2: VReg::V6,
            rs1: Reg::T0,
            vm: false,
        })
    );
}

#[test]
fn test_vwmul_vv() {
    let inst = make_r_type(OP_V, 2, OPMVV, 4, 6, funct7(0b111011, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VwmulVv {
            vd: VReg::V2,
            vs2: VReg::V6,
            vs1: VReg::V4,
            vm: true,
        })
    );
}

#[test]
fn test_vwmul_vx() {
    let inst = make_r_type(OP_V, 2, OPMVX, 5, 6, funct7(0b111011, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VwmulVx {
            vd: VReg::V2,
            vs2: VReg::V6,
            rs1: Reg::T0,
            vm: true,
        })
    );
}

// Single-width integer multiply-add (Section 12.13)

#[test]
fn test_vmacc_vv() {
    let inst = make_r_type(OP_V, 1, OPMVV, 2, 3, funct7(0b101101, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VmaccVv {
            vd: VReg::V1,
            vs1: VReg::V2,
            vs2: VReg::V3,
            vm: true,
        })
    );
}

#[test]
fn test_vmacc_vx() {
    let inst = make_r_type(OP_V, 1, OPMVX, 10, 3, funct7(0b101101, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VmaccVx {
            vd: VReg::V1,
            rs1: Reg::A0,
            vs2: VReg::V3,
            vm: true,
        })
    );
}

#[test]
fn test_vnmsac_vv() {
    let inst = make_r_type(OP_V, 1, OPMVV, 2, 3, funct7(0b101111, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VnmsacVv {
            vd: VReg::V1,
            vs1: VReg::V2,
            vs2: VReg::V3,
            vm: true,
        })
    );
}

#[test]
fn test_vnmsac_vx_masked() {
    let inst = make_r_type(OP_V, 1, OPMVX, 2, 3, funct7(0b101111, false));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VnmsacVx {
            vd: VReg::V1,
            rs1: Reg::Sp,
            vs2: VReg::V3,
            vm: false,
        })
    );
}

#[test]
fn test_vmadd_vv() {
    let inst = make_r_type(OP_V, 1, OPMVV, 2, 3, funct7(0b101001, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VmaddVv {
            vd: VReg::V1,
            vs1: VReg::V2,
            vs2: VReg::V3,
            vm: true,
        })
    );
}

#[test]
fn test_vmadd_vx() {
    let inst = make_r_type(OP_V, 1, OPMVX, 2, 3, funct7(0b101001, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VmaddVx {
            vd: VReg::V1,
            rs1: Reg::Sp,
            vs2: VReg::V3,
            vm: true,
        })
    );
}

#[test]
fn test_vnmsub_vv() {
    let inst = make_r_type(OP_V, 1, OPMVV, 2, 3, funct7(0b101011, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VnmsubVv {
            vd: VReg::V1,
            vs1: VReg::V2,
            vs2: VReg::V3,
            vm: true,
        })
    );
}

#[test]
fn test_vnmsub_vx() {
    let inst = make_r_type(OP_V, 1, OPMVX, 2, 3, funct7(0b101011, false));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VnmsubVx {
            vd: VReg::V1,
            rs1: Reg::Sp,
            vs2: VReg::V3,
            vm: false,
        })
    );
}

// Widening integer multiply-add (Section 12.14)

#[test]
fn test_vwmaccu_vv() {
    let inst = make_r_type(OP_V, 2, OPMVV, 4, 8, funct7(0b111100, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VwmaccuVv {
            vd: VReg::V2,
            vs1: VReg::V4,
            vs2: VReg::V8,
            vm: true,
        })
    );
}

#[test]
fn test_vwmaccu_vx() {
    let inst = make_r_type(OP_V, 2, OPMVX, 10, 8, funct7(0b111100, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VwmaccuVx {
            vd: VReg::V2,
            rs1: Reg::A0,
            vs2: VReg::V8,
            vm: true,
        })
    );
}

#[test]
fn test_vwmacc_vv() {
    let inst = make_r_type(OP_V, 2, OPMVV, 4, 8, funct7(0b111101, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VwmaccVv {
            vd: VReg::V2,
            vs1: VReg::V4,
            vs2: VReg::V8,
            vm: true,
        })
    );
}

#[test]
fn test_vwmacc_vx() {
    let inst = make_r_type(OP_V, 2, OPMVX, 10, 8, funct7(0b111101, false));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VwmaccVx {
            vd: VReg::V2,
            rs1: Reg::A0,
            vs2: VReg::V8,
            vm: false,
        })
    );
}

#[test]
fn test_vwmaccsu_vv() {
    let inst = make_r_type(OP_V, 2, OPMVV, 4, 8, funct7(0b111111, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VwmaccsuVv {
            vd: VReg::V2,
            vs1: VReg::V4,
            vs2: VReg::V8,
            vm: true,
        })
    );
}

#[test]
fn test_vwmaccsu_vx() {
    let inst = make_r_type(OP_V, 2, OPMVX, 10, 8, funct7(0b111111, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VwmaccsuVx {
            vd: VReg::V2,
            rs1: Reg::A0,
            vs2: VReg::V8,
            vm: true,
        })
    );
}

#[test]
fn test_vwmaccus_vx() {
    // Only .vx form exists for vwmaccus
    let inst = make_r_type(OP_V, 2, OPMVX, 10, 8, funct7(0b111110, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VwmaccusVx {
            vd: VReg::V2,
            rs1: Reg::A0,
            vs2: VReg::V8,
            vm: true,
        })
    );
}

#[test]
fn test_vwmaccus_vv_does_not_exist() {
    // funct6=0b111110 under OPMVV should not decode (no .vv form)
    let inst = make_r_type(OP_V, 2, OPMVV, 4, 8, funct7(0b111110, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

// High register numbers

#[test]
fn test_vmul_vv_high_regs() {
    let inst = make_r_type(OP_V, 31, OPMVV, 30, 29, funct7(0b100101, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VmulVv {
            vd: VReg::V31,
            vs2: VReg::V29,
            vs1: VReg::V30,
            vm: true,
        })
    );
}

#[test]
fn test_vdiv_vx_high_regs() {
    let inst = make_r_type(OP_V, 31, OPMVX, 31, 31, funct7(0b100001, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMulDivInstruction::VdivVx {
            vd: VReg::V31,
            vs2: VReg::V31,
            rs1: Reg::T6,
            vm: true,
        })
    );
}

// Negative tests

#[test]
fn test_wrong_opcode() {
    let inst = make_r_type(0b0110011, 1, OPMVV, 2, 3, funct7(0b100101, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_wrong_funct3() {
    // OPIVV (0b000) instead of OPMVV (0b010)
    let inst = make_r_type(OP_V, 1, 0b000, 2, 3, funct7(0b100101, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_invalid_funct6_opmvv() {
    // funct6=0b101000 is not allocated under OPMVV for this group
    let inst = make_r_type(OP_V, 1, OPMVV, 2, 3, funct7(0b101000, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_invalid_funct6_opmvx() {
    // funct6=0b111001 is not allocated under OPMVX for this group
    let inst = make_r_type(OP_V, 1, OPMVX, 2, 3, funct7(0b111001, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

// Display tests

#[test]
fn test_display_vmul_vv_unmasked() {
    let inst = make_r_type(OP_V, 1, OPMVV, 2, 3, funct7(0b100101, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vmul.vv v1, v3, v2");
}

#[test]
fn test_display_vmul_vv_masked() {
    let inst = make_r_type(OP_V, 1, OPMVV, 2, 3, funct7(0b100101, false));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vmul.vv v1, v3, v2, v0.t");
}

#[test]
fn test_display_vmul_vx() {
    let inst = make_r_type(OP_V, 1, OPMVX, 10, 3, funct7(0b100101, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vmul.vx v1, v3, a0");
}

#[test]
fn test_display_vdivu_vv() {
    let inst = make_r_type(OP_V, 8, OPMVV, 9, 10, funct7(0b100000, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vdivu.vv v8, v10, v9");
}

#[test]
fn test_display_vwmul_vv() {
    let inst = make_r_type(OP_V, 2, OPMVV, 4, 6, funct7(0b111011, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vwmul.vv v2, v6, v4");
}

#[test]
fn test_display_vmacc_vv() {
    let inst = make_r_type(OP_V, 1, OPMVV, 2, 3, funct7(0b101101, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vmacc.vv v1, v2, v3");
}

#[test]
fn test_display_vmacc_vx_masked() {
    let inst = make_r_type(OP_V, 1, OPMVX, 10, 3, funct7(0b101101, false));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vmacc.vx v1, a0, v3, v0.t");
}

#[test]
fn test_display_vwmaccus_vx() {
    let inst = make_r_type(OP_V, 2, OPMVX, 10, 8, funct7(0b111110, true));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vwmaccus.vx v2, a0, v8");
}

#[test]
fn test_display_vnmsub_vx_masked() {
    let inst = make_r_type(OP_V, 1, OPMVX, 2, 3, funct7(0b101011, false));
    let decoded = Rv64Zve64xMulDivInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{decoded}"), "vnmsub.vx v1, sp, v3, v0.t");
}
