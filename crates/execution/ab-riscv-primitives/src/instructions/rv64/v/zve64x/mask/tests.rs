extern crate alloc;

use crate::instructions::Instruction;
use crate::instructions::rv64::v::zve64x::mask::Rv64Zve64xMaskInstruction;
use crate::instructions::test_utils::make_r_type;
use crate::registers::general_purpose::Reg;
use crate::registers::vector::VReg;
use alloc::format;

/// Helper: build a vector arithmetic OP-V instruction using make_r_type.
///
/// Vector arithmetic format:
///   `funct6[31:26] | vm[25] | vs2[24:20] | vs1[19:15] | funct3[14:12] | vd[11:7] | opcode[6:0]`
///
/// This maps to R-type with `funct7 = (funct6 << 1) | vm`.
fn make_vop(funct6: u8, vm: u8, vs2: u8, vs1: u8, funct3: u8, vd: u8) -> u32 {
    let funct7 = (funct6 << 1) | (vm & 1);
    make_r_type(0b1010111, vd, funct3, vs1, vs2, funct7)
}

// Mask-register logical instructions (Section 16.1)
// All use OPMVV (funct3=0b010), vm=1

#[test]
#[cfg_attr(miri, ignore)]
fn test_vmandn() {
    let inst = make_vop(0b011000, 1, 2, 3, 0b010, 1);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMaskInstruction::Vmandn {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vmand() {
    let inst = make_vop(0b011001, 1, 4, 5, 0b010, 6);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMaskInstruction::Vmand {
            vd: VReg::V6,
            vs2: VReg::V4,
            vs1: VReg::V5,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vmor() {
    let inst = make_vop(0b011010, 1, 8, 9, 0b010, 10);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMaskInstruction::Vmor {
            vd: VReg::V10,
            vs2: VReg::V8,
            vs1: VReg::V9,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vmxor() {
    let inst = make_vop(0b011011, 1, 12, 13, 0b010, 14);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMaskInstruction::Vmxor {
            vd: VReg::V14,
            vs2: VReg::V12,
            vs1: VReg::V13,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vmorn() {
    let inst = make_vop(0b011100, 1, 16, 17, 0b010, 18);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMaskInstruction::Vmorn {
            vd: VReg::V18,
            vs2: VReg::V16,
            vs1: VReg::V17,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vmnand() {
    let inst = make_vop(0b011101, 1, 20, 21, 0b010, 22);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMaskInstruction::Vmnand {
            vd: VReg::V22,
            vs2: VReg::V20,
            vs1: VReg::V21,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vmnor() {
    let inst = make_vop(0b011110, 1, 24, 25, 0b010, 26);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMaskInstruction::Vmnor {
            vd: VReg::V26,
            vs2: VReg::V24,
            vs1: VReg::V25,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vmxnor() {
    let inst = make_vop(0b011111, 1, 28, 29, 0b010, 30);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMaskInstruction::Vmxnor {
            vd: VReg::V30,
            vs2: VReg::V28,
            vs1: VReg::V29,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vmand_v0() {
    // Use v0 as operand
    let inst = make_vop(0b011001, 1, 0, 1, 0b010, 2);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMaskInstruction::Vmand {
            vd: VReg::V2,
            vs2: VReg::V0,
            vs1: VReg::V1,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_mask_logical_rejects_vm0() {
    // Mask-register logical instructions must have vm=1
    let inst = make_vop(0b011001, 0, 2, 3, 0b010, 1);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

// vcpop.m (Section 16.2) - VWXUNARY0, funct6=010000, vs1=10000
// Result written to scalar x register

#[test]
#[cfg_attr(miri, ignore)]
fn test_vcpop_unmasked() {
    // vcpop.m rd, vs2  (vm=1 = unmasked)
    let inst = make_vop(0b010000, 1, 4, 0b10000, 0b010, 1);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMaskInstruction::Vcpop {
            rd: Reg::Ra,
            vs2: VReg::V4,
            vm: true,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vcpop_masked() {
    // vcpop.m rd, vs2, v0.t  (vm=0 = masked)
    let inst = make_vop(0b010000, 0, 8, 0b10000, 0b010, 10);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMaskInstruction::Vcpop {
            rd: Reg::A0,
            vs2: VReg::V8,
            vm: false,
        })
    );
}

// vfirst.m (Section 16.3) - VWXUNARY0, funct6=010000, vs1=10001

#[test]
#[cfg_attr(miri, ignore)]
fn test_vfirst_unmasked() {
    let inst = make_vop(0b010000, 1, 5, 0b10001, 0b010, 2);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMaskInstruction::Vfirst {
            rd: Reg::Sp,
            vs2: VReg::V5,
            vm: true,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vfirst_masked() {
    let inst = make_vop(0b010000, 0, 12, 0b10001, 0b010, 5);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMaskInstruction::Vfirst {
            rd: Reg::T0,
            vs2: VReg::V12,
            vm: false,
        })
    );
}

// vmsbf.m (Section 16.4) - VMUNARY0, funct6=010100, vs1=00001

#[test]
#[cfg_attr(miri, ignore)]
fn test_vmsbf_unmasked() {
    let inst = make_vop(0b010100, 1, 3, 0b00001, 0b010, 1);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMaskInstruction::Vmsbf {
            vd: VReg::V1,
            vs2: VReg::V3,
            vm: true,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vmsbf_masked() {
    let inst = make_vop(0b010100, 0, 7, 0b00001, 0b010, 2);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMaskInstruction::Vmsbf {
            vd: VReg::V2,
            vs2: VReg::V7,
            vm: false,
        })
    );
}

// vmsof.m (Section 16.5) - VMUNARY0, funct6=010100, vs1=00010

#[test]
#[cfg_attr(miri, ignore)]
fn test_vmsof_unmasked() {
    let inst = make_vop(0b010100, 1, 6, 0b00010, 0b010, 4);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMaskInstruction::Vmsof {
            vd: VReg::V4,
            vs2: VReg::V6,
            vm: true,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vmsof_masked() {
    let inst = make_vop(0b010100, 0, 10, 0b00010, 0b010, 8);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMaskInstruction::Vmsof {
            vd: VReg::V8,
            vs2: VReg::V10,
            vm: false,
        })
    );
}

// vmsif.m (Section 16.6) - VMUNARY0, funct6=010100, vs1=00011

#[test]
#[cfg_attr(miri, ignore)]
fn test_vmsif_unmasked() {
    let inst = make_vop(0b010100, 1, 9, 0b00011, 0b010, 5);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMaskInstruction::Vmsif {
            vd: VReg::V5,
            vs2: VReg::V9,
            vm: true,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vmsif_masked() {
    let inst = make_vop(0b010100, 0, 15, 0b00011, 0b010, 11);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMaskInstruction::Vmsif {
            vd: VReg::V11,
            vs2: VReg::V15,
            vm: false,
        })
    );
}

// viota.m (Section 16.8) - VMUNARY0, funct6=010100, vs1=10000

#[test]
#[cfg_attr(miri, ignore)]
fn test_viota_unmasked() {
    let inst = make_vop(0b010100, 1, 2, 0b10000, 0b010, 4);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMaskInstruction::Viota {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_viota_masked() {
    let inst = make_vop(0b010100, 0, 6, 0b10000, 0b010, 8);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMaskInstruction::Viota {
            vd: VReg::V8,
            vs2: VReg::V6,
            vm: false,
        })
    );
}

// vid.v (Section 16.9) - VMUNARY0, funct6=010100, vs1=10001, vs2=00000

#[test]
#[cfg_attr(miri, ignore)]
fn test_vid_unmasked() {
    let inst = make_vop(0b010100, 1, 0, 0b10001, 0b010, 3);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMaskInstruction::Vid {
            vd: VReg::V3,
            vm: true,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vid_masked() {
    let inst = make_vop(0b010100, 0, 0, 0b10001, 0b010, 16);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMaskInstruction::Vid {
            vd: VReg::V16,
            vm: false,
        })
    );
}

// Negative tests

#[test]
#[cfg_attr(miri, ignore)]
fn test_wrong_opcode() {
    // Use OP (0b0110011) instead of OP-V
    let funct7 = (0b011001u8 << 1) | 1;
    let inst = make_r_type(0b0110011, 1, 0b010, 2, 3, funct7);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_wrong_funct3_for_mask_logical() {
    // OPIVV (funct3=0b000) instead of OPMVV (0b010) for vmand
    let inst = make_vop(0b011001, 1, 2, 3, 0b000, 1);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_wrong_funct3_opivx() {
    // OPIVX (funct3=0b100) for vcpop
    let inst = make_vop(0b010000, 1, 4, 0b10000, 0b100, 1);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vwxunary0_invalid_vs1() {
    // funct6=010000 with vs1=00001 is not vcpop or vfirst
    let inst = make_vop(0b010000, 1, 4, 0b00001, 0b010, 1);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vmunary0_invalid_vs1() {
    // funct6=010100 with vs1=00000 is reserved
    let inst = make_vop(0b010100, 1, 4, 0b00000, 0b010, 1);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vmunary0_invalid_vs1_gap() {
    // funct6=010100 with vs1=01000 falls in the gap between known encodings
    let inst = make_vop(0b010100, 1, 4, 0b01000, 0b010, 1);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_unrelated_funct6() {
    // funct6=000000 (vadd) should not decode as mask instruction
    let inst = make_vop(0b000000, 1, 2, 3, 0b010, 1);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

// Display tests

#[test]
#[cfg_attr(miri, ignore)]
fn test_display_vmand() {
    let inst = make_vop(0b011001, 1, 2, 3, 0b010, 1);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vmand.mm v1, v2, v3");
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_display_vmandn() {
    let inst = make_vop(0b011000, 1, 4, 5, 0b010, 6);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vmandn.mm v6, v4, v5");
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_display_vmxnor() {
    let inst = make_vop(0b011111, 1, 0, 0, 0b010, 0);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vmxnor.mm v0, v0, v0");
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_display_vcpop_unmasked() {
    let inst = make_vop(0b010000, 1, 4, 0b10000, 0b010, 1);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vcpop.m ra, v4");
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_display_vcpop_masked() {
    let inst = make_vop(0b010000, 0, 4, 0b10000, 0b010, 1);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vcpop.m ra, v4, v0.t");
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_display_vfirst_unmasked() {
    let inst = make_vop(0b010000, 1, 8, 0b10001, 0b010, 10);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vfirst.m a0, v8");
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_display_vmsbf_unmasked() {
    let inst = make_vop(0b010100, 1, 3, 0b00001, 0b010, 1);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vmsbf.m v1, v3");
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_display_vmsof_masked() {
    let inst = make_vop(0b010100, 0, 6, 0b00010, 0b010, 4);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vmsof.m v4, v6, v0.t");
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_display_vmsif_unmasked() {
    let inst = make_vop(0b010100, 1, 9, 0b00011, 0b010, 5);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vmsif.m v5, v9");
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_display_viota_masked() {
    let inst = make_vop(0b010100, 0, 2, 0b10000, 0b010, 4);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "viota.m v4, v2, v0.t");
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_display_vid_unmasked() {
    let inst = make_vop(0b010100, 1, 0, 0b10001, 0b010, 3);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vid.v v3");
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_display_vid_masked() {
    let inst = make_vop(0b010100, 0, 0, 0b10001, 0b010, 16);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vid.v v16, v0.t");
}

// Edge cases: high-numbered vector registers

#[test]
#[cfg_attr(miri, ignore)]
fn test_vmand_v31() {
    let inst = make_vop(0b011001, 1, 31, 31, 0b010, 31);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMaskInstruction::Vmand {
            vd: VReg::V31,
            vs2: VReg::V31,
            vs1: VReg::V31,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vcpop_rd_zero() {
    // vcpop.m x0, vs2 - result discarded
    let inst = make_vop(0b010000, 1, 4, 0b10000, 0b010, 0);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMaskInstruction::Vcpop {
            rd: Reg::Zero,
            vs2: VReg::V4,
            vm: true,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vcpop_high_rd() {
    // vcpop.m t6, v31
    let inst = make_vop(0b010000, 1, 31, 0b10000, 0b010, 31);
    let decoded = Rv64Zve64xMaskInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xMaskInstruction::Vcpop {
            rd: Reg::T6,
            vs2: VReg::V31,
            vm: true,
        })
    );
}
