extern crate alloc;

use crate::instructions::Instruction;
use crate::instructions::rv64::v::zve64x::widen_narrow::Rv64Zve64xWidenNarrowInstruction;
use crate::instructions::test_utils::make_r_type;
use crate::registers::general_purpose::Reg;
use crate::registers::vector::VReg;
use alloc::format;

/// OP-V opcode
const OP_V: u8 = 0b1010111;
/// funct3 for OPIVV
const OPIVV: u8 = 0b000;
/// funct3 for OPMVV
const OPMVV: u8 = 0b010;
/// funct3 for OPIVI
const OPIVI: u8 = 0b011;
/// funct3 for OPIVX
const OPIVX: u8 = 0b100;
/// funct3 for OPMVX
const OPMVX: u8 = 0b110;

/// Build a vector arithmetic instruction (unmasked, vm=1)
fn make_vop(funct6: u8, vs2: u8, vs1_or_rs1: u8, funct3: u8, vd: u8) -> u32 {
    // funct7 = funct6 << 1 | vm(=1 for unmasked)
    let funct7 = (funct6 << 1) | 1;
    make_r_type(OP_V, vd, funct3, vs1_or_rs1, vs2, funct7)
}

/// Build a vector arithmetic instruction (masked, vm=0)
fn make_vop_masked(funct6: u8, vs2: u8, vs1_or_rs1: u8, funct3: u8, vd: u8) -> u32 {
    // funct7 = funct6 << 1 | vm(=0 for masked)
    let funct7 = funct6 << 1;
    make_r_type(OP_V, vd, funct3, vs1_or_rs1, vs2, funct7)
}

// Widening unsigned add, 2*SEW = SEW + SEW (funct6=110000)

#[test]
#[cfg_attr(miri, ignore)]
fn test_vwaddu_vv() {
    let inst = make_vop(0b110000, 2, 3, OPMVV, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VwadduVv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vwaddu_vv_masked() {
    let inst = make_vop_masked(0b110000, 4, 5, OPMVV, 8);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VwadduVv {
            vd: VReg::V8,
            vs2: VReg::V4,
            vs1: VReg::V5,
            vm: false,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vwaddu_vx() {
    let inst = make_vop(0b110000, 2, 10, OPMVX, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VwadduVx {
            vd: VReg::V1,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
        })
    );
}

// Widening signed add, 2*SEW = SEW + SEW (funct6=110001)

#[test]
#[cfg_attr(miri, ignore)]
fn test_vwadd_vv() {
    let inst = make_vop(0b110001, 2, 3, OPMVV, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VwaddVv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vwadd_vx() {
    let inst = make_vop(0b110001, 2, 5, OPMVX, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VwaddVx {
            vd: VReg::V1,
            vs2: VReg::V2,
            rs1: Reg::T0,
            vm: true,
        })
    );
}

// Widening unsigned sub, 2*SEW = SEW - SEW (funct6=110010)

#[test]
#[cfg_attr(miri, ignore)]
fn test_vwsubu_vv() {
    let inst = make_vop(0b110010, 2, 3, OPMVV, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VwsubuVv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vwsubu_vx() {
    let inst = make_vop(0b110010, 2, 10, OPMVX, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VwsubuVx {
            vd: VReg::V1,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
        })
    );
}

// Widening signed sub, 2*SEW = SEW - SEW (funct6=110011)

#[test]
#[cfg_attr(miri, ignore)]
fn test_vwsub_vv() {
    let inst = make_vop(0b110011, 2, 3, OPMVV, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VwsubVv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vwsub_vx() {
    let inst = make_vop(0b110011, 2, 5, OPMVX, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VwsubVx {
            vd: VReg::V1,
            vs2: VReg::V2,
            rs1: Reg::T0,
            vm: true,
        })
    );
}

// Widening unsigned add, 2*SEW = 2*SEW + SEW (funct6=110100)

#[test]
#[cfg_attr(miri, ignore)]
fn test_vwaddu_wv() {
    let inst = make_vop(0b110100, 2, 3, OPMVV, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VwadduWv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vwaddu_wx() {
    let inst = make_vop(0b110100, 2, 10, OPMVX, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VwadduWx {
            vd: VReg::V1,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
        })
    );
}

// Widening signed add, 2*SEW = 2*SEW + SEW (funct6=110101)

#[test]
#[cfg_attr(miri, ignore)]
fn test_vwadd_wv() {
    let inst = make_vop(0b110101, 2, 3, OPMVV, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VwaddWv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vwadd_wx() {
    let inst = make_vop(0b110101, 2, 5, OPMVX, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VwaddWx {
            vd: VReg::V1,
            vs2: VReg::V2,
            rs1: Reg::T0,
            vm: true,
        })
    );
}

// Widening unsigned sub, 2*SEW = 2*SEW - SEW (funct6=110110)

#[test]
#[cfg_attr(miri, ignore)]
fn test_vwsubu_wv() {
    let inst = make_vop(0b110110, 2, 3, OPMVV, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VwsubuWv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vwsubu_wx() {
    let inst = make_vop(0b110110, 2, 10, OPMVX, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VwsubuWx {
            vd: VReg::V1,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
        })
    );
}

// Widening signed sub, 2*SEW = 2*SEW - SEW (funct6=110111)

#[test]
#[cfg_attr(miri, ignore)]
fn test_vwsub_wv() {
    let inst = make_vop(0b110111, 2, 3, OPMVV, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VwsubWv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vwsub_wx() {
    let inst = make_vop(0b110111, 2, 5, OPMVX, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VwsubWx {
            vd: VReg::V1,
            vs2: VReg::V2,
            rs1: Reg::T0,
            vm: true,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vwsub_wx_masked() {
    let inst = make_vop_masked(0b110111, 4, 11, OPMVX, 8);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VwsubWx {
            vd: VReg::V8,
            vs2: VReg::V4,
            rs1: Reg::A1,
            vm: false,
        })
    );
}

// Narrowing shift right logical (funct6=101100)

#[test]
#[cfg_attr(miri, ignore)]
fn test_vnsrl_wv() {
    let inst = make_vop(0b101100, 2, 3, OPIVV, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VnsrlWv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vnsrl_wx() {
    let inst = make_vop(0b101100, 2, 10, OPIVX, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VnsrlWx {
            vd: VReg::V1,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vnsrl_wi() {
    let inst = make_vop(0b101100, 2, 3, OPIVI, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VnsrlWi {
            vd: VReg::V1,
            vs2: VReg::V2,
            uimm: 3,
            vm: true,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vnsrl_wi_max_uimm() {
    let inst = make_vop(0b101100, 4, 31, OPIVI, 8);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VnsrlWi {
            vd: VReg::V8,
            vs2: VReg::V4,
            uimm: 31,
            vm: true,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vnsrl_wv_masked() {
    let inst = make_vop_masked(0b101100, 2, 3, OPIVV, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VnsrlWv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: false,
        })
    );
}

// Narrowing shift right arithmetic (funct6=101101)

#[test]
#[cfg_attr(miri, ignore)]
fn test_vnsra_wv() {
    let inst = make_vop(0b101101, 2, 3, OPIVV, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VnsraWv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V3,
            vm: true,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vnsra_wx() {
    let inst = make_vop(0b101101, 2, 10, OPIVX, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VnsraWx {
            vd: VReg::V1,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vnsra_wi() {
    let inst = make_vop(0b101101, 2, 5, OPIVI, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VnsraWi {
            vd: VReg::V1,
            vs2: VReg::V2,
            uimm: 5,
            vm: true,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vnsra_wi_masked() {
    let inst = make_vop_masked(0b101101, 4, 7, OPIVI, 16);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VnsraWi {
            vd: VReg::V16,
            vs2: VReg::V4,
            uimm: 7,
            vm: false,
        })
    );
}

// Integer zero-extension (funct6=010010, OPMVV, vs1 field selects op)

#[test]
#[cfg_attr(miri, ignore)]
fn test_vzext_vf2() {
    // vs1=0b00110
    let inst = make_vop(0b010010, 2, 0b00110, OPMVV, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VzextVf2 {
            vd: VReg::V1,
            vs2: VReg::V2,
            vm: true,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vzext_vf4() {
    // vs1=0b00100
    let inst = make_vop(0b010010, 4, 0b00100, OPMVV, 8);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VzextVf4 {
            vd: VReg::V8,
            vs2: VReg::V4,
            vm: true,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vzext_vf8() {
    // vs1=0b00010
    let inst = make_vop(0b010010, 2, 0b00010, OPMVV, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VzextVf8 {
            vd: VReg::V1,
            vs2: VReg::V2,
            vm: true,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vzext_vf2_masked() {
    let inst = make_vop_masked(0b010010, 2, 0b00110, OPMVV, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VzextVf2 {
            vd: VReg::V1,
            vs2: VReg::V2,
            vm: false,
        })
    );
}

// Integer sign-extension

#[test]
#[cfg_attr(miri, ignore)]
fn test_vsext_vf2() {
    // vs1=0b00111
    let inst = make_vop(0b010010, 2, 0b00111, OPMVV, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VsextVf2 {
            vd: VReg::V1,
            vs2: VReg::V2,
            vm: true,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vsext_vf4() {
    // vs1=0b00101
    let inst = make_vop(0b010010, 4, 0b00101, OPMVV, 8);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VsextVf4 {
            vd: VReg::V8,
            vs2: VReg::V4,
            vm: true,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vsext_vf8() {
    // vs1=0b00011
    let inst = make_vop(0b010010, 2, 0b00011, OPMVV, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VsextVf8 {
            vd: VReg::V1,
            vs2: VReg::V2,
            vm: true,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vsext_vf8_masked() {
    let inst = make_vop_masked(0b010010, 16, 0b00011, OPMVV, 24);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VsextVf8 {
            vd: VReg::V24,
            vs2: VReg::V16,
            vm: false,
        })
    );
}

// Negative tests

#[test]
#[cfg_attr(miri, ignore)]
fn test_wrong_opcode() {
    let inst = make_vop(0b110000, 2, 3, OPMVV, 1) & !0x7f | 0b0110011;
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_widening_add_wrong_funct3() {
    // funct6=110000 with OPIVV (funct3=000) instead of OPMVV
    let inst = make_vop(0b110000, 2, 3, OPIVV, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_narrowing_shift_wrong_funct3() {
    // funct6=101100 with OPMVV (funct3=010) instead of OPIVV/OPIVX/OPIVI
    let inst = make_vop(0b101100, 2, 3, OPMVV, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_extension_wrong_funct3() {
    // funct6=010010 with OPIVV instead of OPMVV
    let inst = make_vop(0b010010, 2, 0b00110, OPIVV, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_extension_invalid_vs1() {
    // funct6=010010, OPMVV, but vs1=0b00000 (not a valid extension encoding)
    let inst = make_vop(0b010010, 2, 0b00000, OPMVV, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_extension_reserved_vs1() {
    // funct6=010010, OPMVV, vs1=0b00001 (reserved, not assigned)
    let inst = make_vop(0b010010, 2, 0b00001, OPMVV, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_unknown_funct6() {
    // funct6=111111 is not a widening/narrowing/extension instruction
    let inst = make_vop(0b111111, 2, 3, OPMVV, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

// High register numbers

#[test]
#[cfg_attr(miri, ignore)]
fn test_vwaddu_vv_high_regs() {
    let inst = make_vop(0b110000, 30, 31, OPMVV, 28);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VwadduVv {
            vd: VReg::V28,
            vs2: VReg::V30,
            vs1: VReg::V31,
            vm: true,
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_vnsrl_wx_high_regs() {
    let inst = make_vop(0b101100, 24, 31, OPIVX, 16);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xWidenNarrowInstruction::VnsrlWx {
            vd: VReg::V16,
            vs2: VReg::V24,
            rs1: Reg::T6,
            vm: true,
        })
    );
}

// Display tests

#[test]
#[cfg_attr(miri, ignore)]
fn test_display_vwaddu_vv_unmasked() {
    let inst = make_vop(0b110000, 2, 3, OPMVV, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vwaddu.vv v1, v2, v3");
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_display_vwaddu_vv_masked() {
    let inst = make_vop_masked(0b110000, 2, 3, OPMVV, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vwaddu.vv v1, v2, v3, v0.t");
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_display_vwadd_vx() {
    let inst = make_vop(0b110001, 4, 10, OPMVX, 8);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vwadd.vx v8, v4, a0");
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_display_vwsub_wv() {
    let inst = make_vop(0b110111, 2, 3, OPMVV, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vwsub.wv v1, v2, v3");
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_display_vnsrl_wi() {
    let inst = make_vop(0b101100, 4, 3, OPIVI, 2);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vnsrl.wi v2, v4, 3");
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_display_vnsra_wx_masked() {
    let inst = make_vop_masked(0b101101, 2, 10, OPIVX, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vnsra.wx v1, v2, a0, v0.t");
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_display_vzext_vf2() {
    let inst = make_vop(0b010010, 2, 0b00110, OPMVV, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vzext.vf2 v1, v2");
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_display_vsext_vf4_masked() {
    let inst = make_vop_masked(0b010010, 4, 0b00101, OPMVV, 8);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vsext.vf4 v8, v4, v0.t");
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_display_vzext_vf8() {
    let inst = make_vop(0b010010, 2, 0b00010, OPMVV, 1);
    let decoded = Rv64Zve64xWidenNarrowInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vzext.vf8 v1, v2");
}
