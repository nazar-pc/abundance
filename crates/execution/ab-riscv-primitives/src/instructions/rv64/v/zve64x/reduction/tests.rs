extern crate alloc;

use crate::instructions::Instruction;
use crate::instructions::rv64::v::zve64x::reduction::Rv64Zve64xReductionInstruction;
use crate::instructions::test_utils::make_r_type;
use crate::registers::general_purpose::Reg;
use crate::registers::vector::VReg;
use alloc::format;

/// Build a vector arithmetic instruction (OPMVV/OPIVV format).
///
/// Layout: [funct6(6)|vm(1)|vs2(5)|vs1(5)|funct3(3)|vd(5)|opcode(7)]
///
/// Reuses `make_r_type` by packing funct6 and vm into the funct7 field:
/// funct7 = (funct6 << 1) | vm
fn make_v_arith(funct6: u8, vm: bool, vs2: u8, vs1: u8, funct3: u8, vd: u8) -> u32 {
    let funct7 = (funct6 << 1) | (vm as u8);
    make_r_type(0b1010111, vd, funct3, vs1, vs2, funct7)
}

// Single-width integer reductions (OPMVV, funct3=0b010)

#[test]
fn test_vredsum() {
    let inst = make_v_arith(0b000000, true, 2, 1, 0b010, 3);
    let decoded = Rv64Zve64xReductionInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xReductionInstruction::Vredsum {
            vd: VReg::V3,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        })
    );
}

#[test]
fn test_vredand() {
    let inst = make_v_arith(0b000001, true, 4, 5, 0b010, 6);
    let decoded = Rv64Zve64xReductionInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xReductionInstruction::Vredand {
            vd: VReg::V6,
            vs2: VReg::V4,
            vs1: VReg::V5,
            vm: true,
        })
    );
}

#[test]
fn test_vredor() {
    let inst = make_v_arith(0b000010, true, 8, 9, 0b010, 10);
    let decoded = Rv64Zve64xReductionInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xReductionInstruction::Vredor {
            vd: VReg::V10,
            vs2: VReg::V8,
            vs1: VReg::V9,
            vm: true,
        })
    );
}

#[test]
fn test_vredxor() {
    let inst = make_v_arith(0b000011, true, 12, 13, 0b010, 14);
    let decoded = Rv64Zve64xReductionInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xReductionInstruction::Vredxor {
            vd: VReg::V14,
            vs2: VReg::V12,
            vs1: VReg::V13,
            vm: true,
        })
    );
}

#[test]
fn test_vredminu() {
    let inst = make_v_arith(0b000100, true, 16, 17, 0b010, 18);
    let decoded = Rv64Zve64xReductionInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xReductionInstruction::Vredminu {
            vd: VReg::V18,
            vs2: VReg::V16,
            vs1: VReg::V17,
            vm: true,
        })
    );
}

#[test]
fn test_vredmin() {
    let inst = make_v_arith(0b000101, true, 20, 21, 0b010, 22);
    let decoded = Rv64Zve64xReductionInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xReductionInstruction::Vredmin {
            vd: VReg::V22,
            vs2: VReg::V20,
            vs1: VReg::V21,
            vm: true,
        })
    );
}

#[test]
fn test_vredmaxu() {
    let inst = make_v_arith(0b000110, true, 24, 25, 0b010, 26);
    let decoded = Rv64Zve64xReductionInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xReductionInstruction::Vredmaxu {
            vd: VReg::V26,
            vs2: VReg::V24,
            vs1: VReg::V25,
            vm: true,
        })
    );
}

#[test]
fn test_vredmax() {
    let inst = make_v_arith(0b000111, true, 28, 29, 0b010, 30);
    let decoded = Rv64Zve64xReductionInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xReductionInstruction::Vredmax {
            vd: VReg::V30,
            vs2: VReg::V28,
            vs1: VReg::V29,
            vm: true,
        })
    );
}

// Widening integer reductions (OPIVV, funct3=0b000)

#[test]
fn test_vwredsumu() {
    let inst = make_v_arith(0b110000, true, 2, 1, 0b000, 4);
    let decoded = Rv64Zve64xReductionInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xReductionInstruction::Vwredsumu {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        })
    );
}

#[test]
fn test_vwredsum() {
    let inst = make_v_arith(0b110001, true, 8, 4, 0b000, 12);
    let decoded = Rv64Zve64xReductionInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xReductionInstruction::Vwredsum {
            vd: VReg::V12,
            vs2: VReg::V8,
            vs1: VReg::V4,
            vm: true,
        })
    );
}

// Masking (vm=0 means masked, vm=1 means unmasked)

#[test]
fn test_vredsum_masked() {
    let inst = make_v_arith(0b000000, false, 2, 1, 0b010, 3);
    let decoded = Rv64Zve64xReductionInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xReductionInstruction::Vredsum {
            vd: VReg::V3,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: false,
        })
    );
}

#[test]
fn test_vredand_masked() {
    let inst = make_v_arith(0b000001, false, 4, 5, 0b010, 6);
    let decoded = Rv64Zve64xReductionInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xReductionInstruction::Vredand {
            vd: VReg::V6,
            vs2: VReg::V4,
            vs1: VReg::V5,
            vm: false,
        })
    );
}

#[test]
fn test_vwredsumu_masked() {
    let inst = make_v_arith(0b110000, false, 8, 4, 0b000, 12);
    let decoded = Rv64Zve64xReductionInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xReductionInstruction::Vwredsumu {
            vd: VReg::V12,
            vs2: VReg::V8,
            vs1: VReg::V4,
            vm: false,
        })
    );
}

#[test]
fn test_vwredsum_masked() {
    let inst = make_v_arith(0b110001, false, 2, 1, 0b000, 3);
    let decoded = Rv64Zve64xReductionInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xReductionInstruction::Vwredsum {
            vd: VReg::V3,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: false,
        })
    );
}

// Edge cases: v0 as operands

#[test]
fn test_vredsum_v0() {
    let inst = make_v_arith(0b000000, true, 0, 0, 0b010, 0);
    let decoded = Rv64Zve64xReductionInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xReductionInstruction::Vredsum {
            vd: VReg::V0,
            vs2: VReg::V0,
            vs1: VReg::V0,
            vm: true,
        })
    );
}

#[test]
fn test_vredmax_v31() {
    let inst = make_v_arith(0b000111, true, 31, 31, 0b010, 31);
    let decoded = Rv64Zve64xReductionInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xReductionInstruction::Vredmax {
            vd: VReg::V31,
            vs2: VReg::V31,
            vs1: VReg::V31,
            vm: true,
        })
    );
}

// Negative tests

#[test]
fn test_wrong_opcode() {
    // Use OP (0b0110011) instead of OP-V
    let funct7 = 1;
    let inst = make_r_type(0b0110011, 3, 0b010, 1, 2, funct7);
    let decoded = Rv64Zve64xReductionInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_wrong_funct3_for_single_width() {
    // funct3=0b001 (OPFVV) instead of 0b010 (OPMVV) for single-width reduction
    let inst = make_v_arith(0b000000, true, 2, 1, 0b001, 3);
    let decoded = Rv64Zve64xReductionInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_wrong_funct3_for_widening() {
    // funct3=0b010 (OPMVV) instead of 0b000 (OPIVV) for widening reduction
    let inst = make_v_arith(0b110000, true, 2, 1, 0b010, 3);
    let decoded = Rv64Zve64xReductionInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_invalid_funct6_opmvv() {
    // funct6=0b001000 is not a reduction under OPMVV
    let inst = make_v_arith(0b001000, true, 2, 1, 0b010, 3);
    let decoded = Rv64Zve64xReductionInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_invalid_funct6_opivv() {
    // funct6=0b110010 is not a widening reduction under OPIVV
    let inst = make_v_arith(0b110010, true, 2, 1, 0b000, 3);
    let decoded = Rv64Zve64xReductionInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_funct6_boundary_above_single_width() {
    // funct6=0b001000 (just above the range 000000-000111)
    let inst = make_v_arith(0b001000, true, 2, 1, 0b010, 3);
    let decoded = Rv64Zve64xReductionInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

// Display formatting

#[test]
fn test_display_vredsum_unmasked() {
    let inst = make_v_arith(0b000000, true, 2, 1, 0b010, 3);
    let decoded = Rv64Zve64xReductionInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vredsum.vs v3, v2, v1");
}

#[test]
fn test_display_vredsum_masked() {
    let inst = make_v_arith(0b000000, false, 2, 1, 0b010, 3);
    let decoded = Rv64Zve64xReductionInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vredsum.vs v3, v2, v1, v0.t");
}

#[test]
fn test_display_vredand() {
    let inst = make_v_arith(0b000001, true, 8, 4, 0b010, 12);
    let decoded = Rv64Zve64xReductionInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vredand.vs v12, v8, v4");
}

#[test]
fn test_display_vredor() {
    let inst = make_v_arith(0b000010, true, 8, 4, 0b010, 12);
    let decoded = Rv64Zve64xReductionInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vredor.vs v12, v8, v4");
}

#[test]
fn test_display_vredxor() {
    let inst = make_v_arith(0b000011, true, 8, 4, 0b010, 12);
    let decoded = Rv64Zve64xReductionInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vredxor.vs v12, v8, v4");
}

#[test]
fn test_display_vredminu() {
    let inst = make_v_arith(0b000100, true, 8, 4, 0b010, 12);
    let decoded = Rv64Zve64xReductionInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vredminu.vs v12, v8, v4");
}

#[test]
fn test_display_vredmin() {
    let inst = make_v_arith(0b000101, true, 8, 4, 0b010, 12);
    let decoded = Rv64Zve64xReductionInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vredmin.vs v12, v8, v4");
}

#[test]
fn test_display_vredmaxu() {
    let inst = make_v_arith(0b000110, true, 8, 4, 0b010, 12);
    let decoded = Rv64Zve64xReductionInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vredmaxu.vs v12, v8, v4");
}

#[test]
fn test_display_vredmax() {
    let inst = make_v_arith(0b000111, true, 8, 4, 0b010, 12);
    let decoded = Rv64Zve64xReductionInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vredmax.vs v12, v8, v4");
}

#[test]
fn test_display_vwredsumu_unmasked() {
    let inst = make_v_arith(0b110000, true, 8, 4, 0b000, 12);
    let decoded = Rv64Zve64xReductionInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vwredsumu.vs v12, v8, v4");
}

#[test]
fn test_display_vwredsum_masked() {
    let inst = make_v_arith(0b110001, false, 2, 1, 0b000, 3);
    let decoded = Rv64Zve64xReductionInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vwredsum.vs v3, v2, v1, v0.t");
}
