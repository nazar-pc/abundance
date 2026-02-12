extern crate alloc;

use crate::instructions::Instruction;
use crate::instructions::rv64::v::zve64x::store::Rv64Zve64xStoreInstruction;
use crate::registers::general_purpose::Reg;
use crate::registers::vector::{Eew, VReg};
use alloc::format;

/// Build a vector store instruction word.
///
/// Layout: nf[31:29] | mew[28] | mop[27:26] | vm[25] | rs2_vs2_sumop[24:20]
///         | rs1[19:15] | width[14:12] | vs3[11:7] | opcode[6:0]
#[expect(clippy::too_many_arguments, reason = "Fine for tests")]
fn make_vs(nf: u8, mew: u8, mop: u8, vm: u8, rs2_field: u8, rs1: u8, width: u8, vs3: u8) -> u32 {
    let opcode: u32 = 0b0100111;
    (opcode)
        | ((vs3 as u32) << 7)
        | ((width as u32) << 12)
        | ((rs1 as u32) << 15)
        | ((rs2_field as u32) << 20)
        | ((vm as u32) << 25)
        | ((mop as u32) << 26)
        | ((mew as u32) << 28)
        | ((nf as u32) << 29)
}

// Unit-stride stores

#[test]
fn test_vse8() {
    let inst = make_vs(0, 0, 0b00, 1, 0b00000, 2, 0b000, 1);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xStoreInstruction::Vse {
            vs3: VReg::V1,
            rs1: Reg::Sp,
            vm: true,
            eew: Eew::E8,
        })
    );
}

#[test]
fn test_vse16_masked() {
    let inst = make_vs(0, 0, 0b00, 0, 0b00000, 10, 0b101, 8);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xStoreInstruction::Vse {
            vs3: VReg::V8,
            rs1: Reg::A0,
            vm: false,
            eew: Eew::E16,
        })
    );
}

#[test]
fn test_vse32() {
    let inst = make_vs(0, 0, 0b00, 1, 0b00000, 5, 0b110, 16);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xStoreInstruction::Vse {
            vs3: VReg::V16,
            rs1: Reg::T0,
            vm: true,
            eew: Eew::E32,
        })
    );
}

#[test]
fn test_vse64() {
    let inst = make_vs(0, 0, 0b00, 1, 0b00000, 3, 0b111, 24);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xStoreInstruction::Vse {
            vs3: VReg::V24,
            rs1: Reg::Gp,
            vm: true,
            eew: Eew::E64,
        })
    );
}

// Mask store

#[test]
fn test_vsm() {
    // vsm.v v0, (x10) - width=e8, vm=1, nf=0, sumop=01011
    let inst = make_vs(0, 0, 0b00, 1, 0b01011, 10, 0b000, 0);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xStoreInstruction::Vsm {
            vs3: VReg::V0,
            rs1: Reg::A0,
        })
    );
}

#[test]
fn test_vsm_invalid_width() {
    let inst = make_vs(0, 0, 0b00, 1, 0b01011, 10, 0b110, 0);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_vsm_invalid_masked() {
    let inst = make_vs(0, 0, 0b00, 0, 0b01011, 10, 0b000, 0);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

// Strided stores

#[test]
fn test_vsse8() {
    let inst = make_vs(0, 0, 0b10, 1, 11, 10, 0b000, 2);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xStoreInstruction::Vsse {
            vs3: VReg::V2,
            rs1: Reg::A0,
            rs2: Reg::A1,
            vm: true,
            eew: Eew::E8,
        })
    );
}

#[test]
fn test_vsse64_masked() {
    let inst = make_vs(0, 0, 0b10, 0, 12, 10, 0b111, 8);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xStoreInstruction::Vsse {
            vs3: VReg::V8,
            rs1: Reg::A0,
            rs2: Reg::A2,
            vm: false,
            eew: Eew::E64,
        })
    );
}

// Indexed-unordered stores

#[test]
fn test_vsuxei8() {
    let inst = make_vs(0, 0, 0b01, 1, 2, 10, 0b000, 4);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xStoreInstruction::Vsuxei {
            vs3: VReg::V4,
            rs1: Reg::A0,
            vs2: VReg::V2,
            vm: true,
            eew: Eew::E8,
        })
    );
}

#[test]
fn test_vsuxei32_masked() {
    let inst = make_vs(0, 0, 0b01, 0, 16, 5, 0b110, 8);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xStoreInstruction::Vsuxei {
            vs3: VReg::V8,
            rs1: Reg::T0,
            vs2: VReg::V16,
            vm: false,
            eew: Eew::E32,
        })
    );
}

// Indexed-ordered stores

#[test]
fn test_vsoxei16() {
    let inst = make_vs(0, 0, 0b11, 1, 8, 10, 0b101, 4);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xStoreInstruction::Vsoxei {
            vs3: VReg::V4,
            rs1: Reg::A0,
            vs2: VReg::V8,
            vm: true,
            eew: Eew::E16,
        })
    );
}

#[test]
fn test_vsoxei64_masked() {
    let inst = make_vs(0, 0, 0b11, 0, 24, 11, 0b111, 16);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xStoreInstruction::Vsoxei {
            vs3: VReg::V16,
            rs1: Reg::A1,
            vs2: VReg::V24,
            vm: false,
            eew: Eew::E64,
        })
    );
}

// Whole-register stores

#[test]
fn test_vs1r() {
    // vs1r.v v8, (x10) - nf=0 (nreg=1), sumop=01000, vm=1, width=e8
    let inst = make_vs(0, 0, 0b00, 1, 0b01000, 10, 0b000, 8);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xStoreInstruction::Vsr {
            vs3: VReg::V8,
            rs1: Reg::A0,
            nreg: 1,
        })
    );
}

#[test]
fn test_vs2r() {
    // vs2r.v v8, (x10) - nf=1 (nreg=2)
    let inst = make_vs(1, 0, 0b00, 1, 0b01000, 10, 0b000, 8);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xStoreInstruction::Vsr {
            vs3: VReg::V8,
            rs1: Reg::A0,
            nreg: 2,
        })
    );
}

#[test]
fn test_vs4r() {
    let inst = make_vs(3, 0, 0b00, 1, 0b01000, 10, 0b000, 8);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xStoreInstruction::Vsr {
            vs3: VReg::V8,
            rs1: Reg::A0,
            nreg: 4,
        })
    );
}

#[test]
fn test_vs8r() {
    let inst = make_vs(7, 0, 0b00, 1, 0b01000, 10, 0b000, 0);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xStoreInstruction::Vsr {
            vs3: VReg::V0,
            rs1: Reg::A0,
            nreg: 8,
        })
    );
}

#[test]
fn test_vsr_invalid_nreg_3() {
    let inst = make_vs(2, 0, 0b00, 1, 0b01000, 10, 0b000, 8);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_vsr_invalid_masked() {
    let inst = make_vs(0, 0, 0b00, 0, 0b01000, 10, 0b000, 8);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_vsr_invalid_width() {
    // Whole-register store must have width=e8 (0b000)
    let inst = make_vs(0, 0, 0b00, 1, 0b01000, 10, 0b110, 8);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

// Segment stores

#[test]
fn test_vsseg2e8() {
    let inst = make_vs(1, 0, 0b00, 1, 0b00000, 10, 0b000, 4);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xStoreInstruction::Vsseg {
            vs3: VReg::V4,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E8,
            nf: 2,
        })
    );
}

#[test]
fn test_vsseg8e32_masked() {
    let inst = make_vs(7, 0, 0b00, 0, 0b00000, 5, 0b110, 0);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xStoreInstruction::Vsseg {
            vs3: VReg::V0,
            rs1: Reg::T0,
            vm: false,
            eew: Eew::E32,
            nf: 8,
        })
    );
}

#[test]
fn test_vssseg4e64() {
    let inst = make_vs(3, 0, 0b10, 1, 11, 10, 0b111, 8);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xStoreInstruction::Vssseg {
            vs3: VReg::V8,
            rs1: Reg::A0,
            rs2: Reg::A1,
            vm: true,
            eew: Eew::E64,
            nf: 4,
        })
    );
}

#[test]
fn test_vsuxseg2ei32() {
    let inst = make_vs(1, 0, 0b01, 1, 8, 10, 0b110, 4);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xStoreInstruction::Vsuxseg {
            vs3: VReg::V4,
            rs1: Reg::A0,
            vs2: VReg::V8,
            vm: true,
            eew: Eew::E32,
            nf: 2,
        })
    );
}

#[test]
fn test_vsoxseg3ei8_masked() {
    let inst = make_vs(2, 0, 0b11, 0, 12, 10, 0b000, 4);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xStoreInstruction::Vsoxseg {
            vs3: VReg::V4,
            rs1: Reg::A0,
            vs2: VReg::V12,
            vm: false,
            eew: Eew::E8,
            nf: 3,
        })
    );
}

// Negative tests

#[test]
fn test_wrong_opcode() {
    // Use LOAD-FP opcode
    let mut inst = make_vs(0, 0, 0b00, 1, 0b00000, 10, 0b000, 8);
    inst = (inst & !0x7f) | 0b0000111;
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_mew_reserved() {
    let inst = make_vs(0, 1, 0b00, 1, 0b00000, 10, 0b000, 8);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_invalid_width() {
    let inst = make_vs(0, 0, 0b00, 1, 0b00000, 10, 0b010, 8);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_invalid_sumop() {
    let inst = make_vs(0, 0, 0b00, 1, 0b00010, 10, 0b000, 8);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

// Display tests

#[test]
fn test_display_vse32() {
    let inst = make_vs(0, 0, 0b00, 1, 0b00000, 10, 0b110, 8);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vse32.v v8, (a0)");
}

#[test]
fn test_display_vse8_masked() {
    let inst = make_vs(0, 0, 0b00, 0, 0b00000, 10, 0b000, 8);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vse8.v v8, (a0), v0.t");
}

#[test]
fn test_display_vsm() {
    let inst = make_vs(0, 0, 0b00, 1, 0b01011, 10, 0b000, 0);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vsm.v v0, (a0)");
}

#[test]
fn test_display_vsse64() {
    let inst = make_vs(0, 0, 0b10, 1, 11, 10, 0b111, 8);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vsse64.v v8, (a0), a1");
}

#[test]
fn test_display_vsuxei32() {
    let inst = make_vs(0, 0, 0b01, 1, 16, 10, 0b110, 8);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vsuxei32.v v8, (a0), v16");
}

#[test]
fn test_display_vs4r() {
    let inst = make_vs(3, 0, 0b00, 1, 0b01000, 10, 0b000, 8);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vs4r.v v8, (a0)");
}

#[test]
fn test_display_vsseg3e16() {
    let inst = make_vs(2, 0, 0b00, 1, 0b00000, 10, 0b101, 8);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vsseg3e16.v v8, (a0)");
}

#[test]
fn test_display_vsoxseg2ei64_masked() {
    let inst = make_vs(1, 0, 0b11, 0, 12, 10, 0b111, 4);
    let decoded = Rv64Zve64xStoreInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vsoxseg2ei64.v v4, (a0), v12, v0.t");
}
