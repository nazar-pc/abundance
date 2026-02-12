extern crate alloc;

use crate::instructions::Instruction;
use crate::instructions::rv64::v::zve64x::load::{Eew, Rv64Zve64xLoadInstruction};
use crate::registers::general_purpose::Reg;
use crate::registers::vector::VReg;
use alloc::format;

/// Build a vector load instruction word.
///
/// Layout: nf[31:29] | mew[28] | mop[27:26] | vm[25] | rs2_vs2_lumop[24:20]
///         | rs1[19:15] | width[14:12] | vd[11:7] | opcode[6:0]
#[expect(clippy::too_many_arguments, reason = "Fine for tests")]
fn make_vl(nf: u8, mew: u8, mop: u8, vm: u8, rs2_field: u8, rs1: u8, width: u8, vd: u8) -> u32 {
    let opcode: u32 = 0b0000111;
    (opcode)
        | ((vd as u32) << 7)
        | ((width as u32) << 12)
        | ((rs1 as u32) << 15)
        | ((rs2_field as u32) << 20)
        | ((vm as u32) << 25)
        | ((mop as u32) << 26)
        | ((mew as u32) << 28)
        | ((nf as u32) << 29)
}

// Unit-stride loads

#[test]
fn test_vle8() {
    // vle8.v v1, (x2), vm=1 (unmasked)
    let inst = make_vl(0, 0, 0b00, 1, 0b00000, 2, 0b000, 1);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xLoadInstruction::Vle {
            vd: VReg::V1,
            rs1: Reg::Sp,
            vm: true,
            eew: Eew::E8,
        })
    );
}

#[test]
fn test_vle16_masked() {
    // vle16.v v8, (x10), v0.t
    let inst = make_vl(0, 0, 0b00, 0, 0b00000, 10, 0b101, 8);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xLoadInstruction::Vle {
            vd: VReg::V8,
            rs1: Reg::A0,
            vm: false,
            eew: Eew::E16,
        })
    );
}

#[test]
fn test_vle32() {
    let inst = make_vl(0, 0, 0b00, 1, 0b00000, 5, 0b110, 16);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xLoadInstruction::Vle {
            vd: VReg::V16,
            rs1: Reg::T0,
            vm: true,
            eew: Eew::E32,
        })
    );
}

#[test]
fn test_vle64() {
    let inst = make_vl(0, 0, 0b00, 1, 0b00000, 3, 0b111, 24);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xLoadInstruction::Vle {
            vd: VReg::V24,
            rs1: Reg::Gp,
            vm: true,
            eew: Eew::E64,
        })
    );
}

// Fault-only-first loads

#[test]
fn test_vle8ff() {
    // vle8ff.v v4, (x11)
    let inst = make_vl(0, 0, 0b00, 1, 0b10000, 11, 0b000, 4);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xLoadInstruction::Vleff {
            vd: VReg::V4,
            rs1: Reg::A1,
            vm: true,
            eew: Eew::E8,
        })
    );
}

#[test]
fn test_vle32ff_masked() {
    let inst = make_vl(0, 0, 0b00, 0, 0b10000, 10, 0b110, 8);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xLoadInstruction::Vleff {
            vd: VReg::V8,
            rs1: Reg::A0,
            vm: false,
            eew: Eew::E32,
        })
    );
}

// Mask load

#[test]
fn test_vlm() {
    // vlm.v v0, (x10) - width=e8, vm=1, nf=0, lumop=01011
    let inst = make_vl(0, 0, 0b00, 1, 0b01011, 10, 0b000, 0);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xLoadInstruction::Vlm {
            vd: VReg::V0,
            rs1: Reg::A0,
        })
    );
}

#[test]
fn test_vlm_invalid_width() {
    // vlm with width != e8 should fail
    let inst = make_vl(0, 0, 0b00, 1, 0b01011, 10, 0b110, 0);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_vlm_invalid_masked() {
    // vlm with vm=0 should fail
    let inst = make_vl(0, 0, 0b00, 0, 0b01011, 10, 0b000, 0);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

// Strided loads

#[test]
fn test_vlse8() {
    // vlse8.v v2, (x10), x11
    let inst = make_vl(0, 0, 0b10, 1, 11, 10, 0b000, 2);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xLoadInstruction::Vlse {
            vd: VReg::V2,
            rs1: Reg::A0,
            rs2: Reg::A1,
            vm: true,
            eew: Eew::E8,
        })
    );
}

#[test]
fn test_vlse64_masked() {
    let inst = make_vl(0, 0, 0b10, 0, 12, 10, 0b111, 8);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xLoadInstruction::Vlse {
            vd: VReg::V8,
            rs1: Reg::A0,
            rs2: Reg::A2,
            vm: false,
            eew: Eew::E64,
        })
    );
}

// Indexed-unordered loads

#[test]
fn test_vluxei8() {
    // vluxei8.v v4, (x10), v2
    let inst = make_vl(0, 0, 0b01, 1, 2, 10, 0b000, 4);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xLoadInstruction::Vluxei {
            vd: VReg::V4,
            rs1: Reg::A0,
            vs2: VReg::V2,
            vm: true,
            eew: Eew::E8,
        })
    );
}

#[test]
fn test_vluxei32_masked() {
    let inst = make_vl(0, 0, 0b01, 0, 16, 5, 0b110, 8);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xLoadInstruction::Vluxei {
            vd: VReg::V8,
            rs1: Reg::T0,
            vs2: VReg::V16,
            vm: false,
            eew: Eew::E32,
        })
    );
}

// Indexed-ordered loads

#[test]
fn test_vloxei16() {
    // vloxei16.v v4, (x10), v8
    let inst = make_vl(0, 0, 0b11, 1, 8, 10, 0b101, 4);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xLoadInstruction::Vloxei {
            vd: VReg::V4,
            rs1: Reg::A0,
            vs2: VReg::V8,
            vm: true,
            eew: Eew::E16,
        })
    );
}

#[test]
fn test_vloxei64_masked() {
    let inst = make_vl(0, 0, 0b11, 0, 24, 11, 0b111, 16);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xLoadInstruction::Vloxei {
            vd: VReg::V16,
            rs1: Reg::A1,
            vs2: VReg::V24,
            vm: false,
            eew: Eew::E64,
        })
    );
}

// Whole-register loads

#[test]
fn test_vl1re8() {
    // vl1re8.v v8, (x10) - nf=0 (nreg=1), lumop=01000, vm=1, width=e8
    let inst = make_vl(0, 0, 0b00, 1, 0b01000, 10, 0b000, 8);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xLoadInstruction::Vlr {
            vd: VReg::V8,
            rs1: Reg::A0,
            nreg: 1,
            eew: Eew::E8,
        })
    );
}

#[test]
fn test_vl2re32() {
    // vl2re32.v v8, (x10) - nf=1 (nreg=2)
    let inst = make_vl(1, 0, 0b00, 1, 0b01000, 10, 0b110, 8);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xLoadInstruction::Vlr {
            vd: VReg::V8,
            rs1: Reg::A0,
            nreg: 2,
            eew: Eew::E32,
        })
    );
}

#[test]
fn test_vl4re64() {
    // vl4re64.v v8, (x10) - nf=3 (nreg=4)
    let inst = make_vl(3, 0, 0b00, 1, 0b01000, 10, 0b111, 8);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xLoadInstruction::Vlr {
            vd: VReg::V8,
            rs1: Reg::A0,
            nreg: 4,
            eew: Eew::E64,
        })
    );
}

#[test]
fn test_vl8re16() {
    // vl8re16.v v0, (x10) - nf=7 (nreg=8)
    let inst = make_vl(7, 0, 0b00, 1, 0b01000, 10, 0b101, 0);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xLoadInstruction::Vlr {
            vd: VReg::V0,
            rs1: Reg::A0,
            nreg: 8,
            eew: Eew::E16,
        })
    );
}

#[test]
fn test_vlr_invalid_nreg_3() {
    // nf=2 => nreg=3, which is not a power of 2
    let inst = make_vl(2, 0, 0b00, 1, 0b01000, 10, 0b000, 8);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_vlr_invalid_nreg_5() {
    // nf=4 => nreg=5
    let inst = make_vl(4, 0, 0b00, 1, 0b01000, 10, 0b000, 8);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_vlr_invalid_masked() {
    // Whole-register load with vm=0 is invalid
    let inst = make_vl(0, 0, 0b00, 0, 0b01000, 10, 0b000, 8);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

// Segment loads

#[test]
fn test_vlseg2e8() {
    // vlseg2e8.v v4, (x10) - nf=1 means 2 fields
    let inst = make_vl(1, 0, 0b00, 1, 0b00000, 10, 0b000, 4);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xLoadInstruction::Vlseg {
            vd: VReg::V4,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E8,
            nf: 2,
        })
    );
}

#[test]
fn test_vlseg8e32_masked() {
    // vlseg8e32.v v0, (x5), v0.t - nf=7 means 8 fields
    let inst = make_vl(7, 0, 0b00, 0, 0b00000, 5, 0b110, 0);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xLoadInstruction::Vlseg {
            vd: VReg::V0,
            rs1: Reg::T0,
            vm: false,
            eew: Eew::E32,
            nf: 8,
        })
    );
}

#[test]
fn test_vlseg3e16ff() {
    // vlseg3e16ff.v v8, (x10) - nf=2 means 3 fields, lumop=10000
    let inst = make_vl(2, 0, 0b00, 1, 0b10000, 10, 0b101, 8);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xLoadInstruction::Vlsegff {
            vd: VReg::V8,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E16,
            nf: 3,
        })
    );
}

#[test]
fn test_vlsseg4e64() {
    // vlsseg4e64.v v8, (x10), x11 - strided segment, nf=3
    let inst = make_vl(3, 0, 0b10, 1, 11, 10, 0b111, 8);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xLoadInstruction::Vlsseg {
            vd: VReg::V8,
            rs1: Reg::A0,
            rs2: Reg::A1,
            vm: true,
            eew: Eew::E64,
            nf: 4,
        })
    );
}

#[test]
fn test_vluxseg2ei32() {
    // vluxseg2ei32.v v4, (x10), v8 - indexed-unordered segment, nf=1
    let inst = make_vl(1, 0, 0b01, 1, 8, 10, 0b110, 4);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xLoadInstruction::Vluxseg {
            vd: VReg::V4,
            rs1: Reg::A0,
            vs2: VReg::V8,
            vm: true,
            eew: Eew::E32,
            nf: 2,
        })
    );
}

#[test]
fn test_vloxseg3ei8() {
    // vloxseg3ei8.v v4, (x10), v12, v0.t - indexed-ordered segment, nf=2
    let inst = make_vl(2, 0, 0b11, 0, 12, 10, 0b000, 4);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xLoadInstruction::Vloxseg {
            vd: VReg::V4,
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
    // Use STORE-FP opcode instead of LOAD-FP
    let inst = make_vl(0, 0, 0b00, 1, 0b00000, 10, 0b000, 8) | 0b0100000;
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_mew_reserved() {
    // mew=1 is reserved
    let inst = make_vl(0, 1, 0b00, 1, 0b00000, 10, 0b000, 8);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_invalid_width() {
    // width=0b001 is not a valid EEW
    let inst = make_vl(0, 0, 0b00, 1, 0b00000, 10, 0b001, 8);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_invalid_lumop() {
    // lumop=0b00001 is reserved
    let inst = make_vl(0, 0, 0b00, 1, 0b00001, 10, 0b000, 8);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

// Display tests

#[test]
fn test_display_vle32() {
    let inst = make_vl(0, 0, 0b00, 1, 0b00000, 10, 0b110, 8);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vle32.v v8, (a0)");
}

#[test]
fn test_display_vle8_masked() {
    let inst = make_vl(0, 0, 0b00, 0, 0b00000, 10, 0b000, 8);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vle8.v v8, (a0), v0.t");
}

#[test]
fn test_display_vlm() {
    let inst = make_vl(0, 0, 0b00, 1, 0b01011, 10, 0b000, 0);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vlm.v v0, (a0)");
}

#[test]
fn test_display_vlse64() {
    let inst = make_vl(0, 0, 0b10, 1, 11, 10, 0b111, 8);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vlse64.v v8, (a0), a1");
}

#[test]
fn test_display_vluxei32() {
    let inst = make_vl(0, 0, 0b01, 1, 16, 10, 0b110, 8);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vluxei32.v v8, (a0), v16");
}

#[test]
fn test_display_vl4re64() {
    let inst = make_vl(3, 0, 0b00, 1, 0b01000, 10, 0b111, 8);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vl4re64.v v8, (a0)");
}

#[test]
fn test_display_vlseg3e16() {
    let inst = make_vl(2, 0, 0b00, 1, 0b00000, 10, 0b101, 8);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vlseg3e16.v v8, (a0)");
}

#[test]
fn test_display_vloxseg2ei64_masked() {
    let inst = make_vl(1, 0, 0b11, 0, 12, 10, 0b111, 4);
    let decoded = Rv64Zve64xLoadInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vloxseg2ei64.v v4, (a0), v12, v0.t");
}
