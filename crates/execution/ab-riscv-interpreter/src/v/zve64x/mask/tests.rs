use crate::rv64::test_utils::{TestInterpreterState, initialize_state};
use crate::v::vector_registers::{VectorRegisters, VectorRegistersExt};
use crate::{ExecutableInstruction, ExecutionError};
use ab_riscv_primitives::instructions::v::zve64x::mask::Zve64xMaskInstruction;
use ab_riscv_primitives::instructions::v::{Vlmul, Vsew, Vtype};
use ab_riscv_primitives::registers::general_purpose::Reg;
use ab_riscv_primitives::registers::vector::VReg;

// With TEST_VLEN=128, VLENB=16:
//   E8/M1 -> VLMAX=16, 1 reg
//   E16/M1 -> VLMAX=8, 1 reg
//   E32/M1 -> VLMAX=4, 1 reg
//   E64/M1 -> VLMAX=2, 1 reg

// helpers

fn encode_vtype(vsew: Vsew, vlmul: Vlmul) -> u64 {
    (vlmul.to_bits() as u64) | ((vsew.to_bits() as u64) << 3)
}

fn setup(
    vl: u32,
    vsew: Vsew,
    vlmul: Vlmul,
) -> TestInterpreterState<Zve64xMaskInstruction<Reg<u64>>> {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    let vtype = Vtype::from_raw::<Reg<u64>>(encode_vtype(vsew, vlmul)).unwrap();
    state.ext_state.set_vtype(Some(vtype));
    state.ext_state.set_vl(vl);
    state.ext_state.set_vstart(0);
    state
}

fn exec(
    state: &mut TestInterpreterState<Zve64xMaskInstruction<Reg<u64>>>,
    instr: Zve64xMaskInstruction<Reg<u64>>,
) -> Result<(), ExecutionError<u64>> {
    instr.execute(state).map(|_| ())
}

fn get_vreg(state: &TestInterpreterState<Zve64xMaskInstruction<Reg<u64>>>, reg: VReg) -> [u8; 16] {
    state.ext_state.read_vreg()[usize::from(reg.bits())]
}

fn set_vreg(
    state: &mut TestInterpreterState<Zve64xMaskInstruction<Reg<u64>>>,
    reg: VReg,
    data: [u8; 16],
) {
    state.ext_state.write_vreg()[usize::from(reg.bits())] = data;
}

/// Read element `i` from a register group as a u64 (zero-extended), given SEW
fn read_elem(
    state: &TestInterpreterState<Zve64xMaskInstruction<Reg<u64>>>,
    base_reg: VReg,
    elem_i: usize,
    sew: Vsew,
) -> u64 {
    let sew_bytes = usize::from(sew.bytes());
    let elems_per_reg = 16 / sew_bytes;
    let reg_off = elem_i / elems_per_reg;
    let byte_off = (elem_i % elems_per_reg) * sew_bytes;
    let reg = &state.ext_state.read_vreg()[usize::from(base_reg.bits()) + reg_off];
    let mut buf = [0u8; 8];
    buf[..sew_bytes].copy_from_slice(&reg[byte_off..byte_off + sew_bytes]);
    u64::from_le_bytes(buf)
}

/// Read mask bit `i` from a vector register
fn mask_bit(
    state: &TestInterpreterState<Zve64xMaskInstruction<Reg<u64>>>,
    reg: VReg,
    i: u32,
) -> bool {
    let byte = state.ext_state.read_vreg()[usize::from(reg.bits())][(i / u8::BITS) as usize];
    (byte >> (i % u8::BITS)) & 1 != 0
}

// mask-logical

#[test]
fn vmand_basic() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    // vs2 = 0b10101010, vs1 = 0b11001100
    set_vreg(&mut state, VReg::V2, [0xAA; 16]);
    set_vreg(&mut state, VReg::V1, [0xCC; 16]);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vmand {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
        },
    )
    .unwrap();
    // 0xAA & 0xCC = 0x88
    assert_eq!(get_vreg(&state, VReg::V4), [0x88; 16]);
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vmor_basic() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    set_vreg(&mut state, VReg::V2, [0xAA; 16]);
    set_vreg(&mut state, VReg::V1, [0x55; 16]);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vmor {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
        },
    )
    .unwrap();
    // 0xAA | 0x55 = 0xFF
    assert_eq!(get_vreg(&state, VReg::V4), [0xFF; 16]);
}

#[test]
fn vmxor_basic() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    set_vreg(&mut state, VReg::V2, [0xF0; 16]);
    set_vreg(&mut state, VReg::V1, [0xFF; 16]);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vmxor {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
        },
    )
    .unwrap();
    // 0xF0 ^ 0xFF = 0x0F
    assert_eq!(get_vreg(&state, VReg::V4), [0x0F; 16]);
}

#[test]
fn vmandn_basic() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    // vmandn: vd = vs2 AND NOT vs1
    set_vreg(&mut state, VReg::V2, [0xFF; 16]);
    set_vreg(&mut state, VReg::V1, [0x0F; 16]);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vmandn {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
        },
    )
    .unwrap();
    // 0xFF & !0x0F = 0xF0
    assert_eq!(get_vreg(&state, VReg::V4), [0xF0; 16]);
}

#[test]
fn vmorn_basic() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    // vmorn: vd = vs2 OR NOT vs1
    set_vreg(&mut state, VReg::V2, [0x00; 16]);
    set_vreg(&mut state, VReg::V1, [0x0F; 16]);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vmorn {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
        },
    )
    .unwrap();
    // 0x00 | !0x0F = 0xF0
    assert_eq!(get_vreg(&state, VReg::V4), [0xF0; 16]);
}

#[test]
fn vmnand_basic() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    // vmnand: vd = NOT(vs2 AND vs1)
    set_vreg(&mut state, VReg::V2, [0xFF; 16]);
    set_vreg(&mut state, VReg::V1, [0xFF; 16]);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vmnand {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
        },
    )
    .unwrap();
    // NOT(0xFF & 0xFF) = 0x00
    assert_eq!(get_vreg(&state, VReg::V4), [0x00; 16]);
}

#[test]
fn vmnor_basic() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    // vmnor: vd = NOT(vs2 OR vs1)
    set_vreg(&mut state, VReg::V2, [0x00; 16]);
    set_vreg(&mut state, VReg::V1, [0x00; 16]);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vmnor {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
        },
    )
    .unwrap();
    // NOT(0x00 | 0x00) = 0xFF
    assert_eq!(get_vreg(&state, VReg::V4), [0xFF; 16]);
}

#[test]
fn vmxnor_basic() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    // vmxnor: vd = NOT(vs2 XOR vs1)
    set_vreg(&mut state, VReg::V2, [0xAA; 16]);
    set_vreg(&mut state, VReg::V1, [0xAA; 16]);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vmxnor {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
        },
    )
    .unwrap();
    // NOT(0xAA ^ 0xAA) = NOT(0x00) = 0xFF
    assert_eq!(get_vreg(&state, VReg::V4), [0xFF; 16]);
}

/// Mask-logical ops operate on full VLENB bytes regardless of vl.
/// Even with vl=0 the full register is processed.
#[test]
fn vmand_operates_on_full_register_regardless_of_vl() {
    let mut state = setup(0, Vsew::E8, Vlmul::M1);
    set_vreg(&mut state, VReg::V2, [0xFF; 16]);
    set_vreg(&mut state, VReg::V1, [0xAA; 16]);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vmand {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
        },
    )
    .unwrap();
    // All 16 bytes processed: 0xFF & 0xAA = 0xAA
    assert_eq!(get_vreg(&state, VReg::V4), [0xAA; 16]);
}

/// vd may overlap vs2 for mask-logical ops.
#[test]
fn vmand_vd_overlaps_vs2() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    set_vreg(&mut state, VReg::V2, [0xFF; 16]);
    set_vreg(&mut state, VReg::V1, [0x0F; 16]);
    // vd = vs2
    exec(
        &mut state,
        Zve64xMaskInstruction::Vmand {
            vd: VReg::V2,
            vs2: VReg::V2,
            vs1: VReg::V1,
        },
    )
    .unwrap();
    assert_eq!(get_vreg(&state, VReg::V2), [0x0F; 16]);
}

/// vd may overlap vs1 for mask-logical ops
#[test]
fn vmand_vd_overlaps_vs1() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    set_vreg(&mut state, VReg::V2, [0xFF; 16]);
    set_vreg(&mut state, VReg::V1, [0x0F; 16]);
    // vd = vs1
    exec(
        &mut state,
        Zve64xMaskInstruction::Vmand {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V1,
        },
    )
    .unwrap();
    assert_eq!(get_vreg(&state, VReg::V1), [0x0F; 16]);
}

/// vd may be v0 for mask-logical ops (they are always unmasked)
#[test]
fn vmand_vd_is_v0() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    set_vreg(&mut state, VReg::V2, [0xFF; 16]);
    set_vreg(&mut state, VReg::V1, [0xAA; 16]);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vmand {
            vd: VReg::V0,
            vs2: VReg::V2,
            vs1: VReg::V1,
        },
    )
    .unwrap();
    assert_eq!(get_vreg(&state, VReg::V0), [0xAA; 16]);
}

/// Mask-logical ops require vector instructions to be allowed
#[test]
fn vmand_vector_not_allowed() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    state.ext_state.set_vector_allowed(false);
    let result = exec(
        &mut state,
        Zve64xMaskInstruction::Vmand {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
        },
    );
    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

// vcpop

/// vcpop with all bits set in vs2, unmasked
#[test]
fn vcpop_all_set_unmasked() {
    let mut state = setup(16, Vsew::E8, Vlmul::M1);
    set_vreg(&mut state, VReg::V2, [0xFF; 16]);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vcpop {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(state.regs.read(Reg::A0), 16);
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

/// vcpop with all bits clear
#[test]
fn vcpop_all_clear() {
    let mut state = setup(16, Vsew::E8, Vlmul::M1);
    set_vreg(&mut state, VReg::V2, [0x00; 16]);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vcpop {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0);
}

/// vcpop counts only the bits within vl, not beyond
#[test]
fn vcpop_respects_vl() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    // All bits set, but only 4 elements active
    set_vreg(&mut state, VReg::V2, [0xFF; 16]);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vcpop {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(state.regs.read(Reg::A0), 4);
}

/// vcpop with a mask: only active elements in vs2 are counted
#[test]
fn vcpop_masked() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    // vs2: bits 0,1,2,3,4,5,6,7 all set
    set_vreg(&mut state, VReg::V2, [0xFF; 16]);
    // mask v0: only elements 0,2,4,6 active (alternating, low nibble = 0b01010101 = 0x55)
    set_vreg(
        &mut state,
        VReg::V0,
        [0x55, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );
    exec(
        &mut state,
        Zve64xMaskInstruction::Vcpop {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: false,
        },
    )
    .unwrap();
    // 4 active elements, all set in vs2 → count = 4
    assert_eq!(state.regs.read(Reg::A0), 4);
}

/// vcpop with vstart > 0 skips elements before vstart
#[test]
fn vcpop_vstart_skips_early_elements() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    set_vreg(&mut state, VReg::V2, [0xFF; 16]);
    state.ext_state.set_vstart(4);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vcpop {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: true,
        },
    )
    .unwrap();
    // Only elements 4..8 counted
    assert_eq!(state.regs.read(Reg::A0), 4);
}

/// vcpop requires valid vtype
#[test]
fn vcpop_invalid_vtype() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    state.ext_state.set_vtype(None);
    let result = exec(
        &mut state,
        Zve64xMaskInstruction::Vcpop {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: true,
        },
    );
    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

/// vcpop requires vector instructions to be allowed
#[test]
fn vcpop_vector_not_allowed() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    state.ext_state.set_vector_allowed(false);
    let result = exec(
        &mut state,
        Zve64xMaskInstruction::Vcpop {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: true,
        },
    );
    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

/// vcpop with a sparse pattern to verify exact bit-counting
#[test]
fn vcpop_sparse_bits() {
    let mut state = setup(16, Vsew::E8, Vlmul::M1);
    // Set exactly bits 0, 3, 7, 11, 15 - one per byte boundary cluster
    let mut data = [0u8; 16];
    // Byte 0: bits 0,3,7 → 0b10001001 = 0x89
    data[0] = 0x89;
    // Byte 1: bits 8+3=11 → 0b00001000 = 0x08
    data[1] = 0x08;
    // bit 15: byte 1 bit 7 → 0x80
    data[1] |= 0x80;
    set_vreg(&mut state, VReg::V2, data);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vcpop {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(state.regs.read(Reg::A0), 5);
}

// vfirst

/// vfirst finds the first set bit
#[test]
fn vfirst_basic() {
    let mut state = setup(16, Vsew::E8, Vlmul::M1);
    // First set bit at position 3
    let mut data = [0u8; 16];
    data[0] = 0b00001000;
    set_vreg(&mut state, VReg::V2, data);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vfirst {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(state.regs.read(Reg::A0), 3);
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

/// vfirst with no bits set returns -1 (all-ones for XLEN=64 → u64::MAX)
#[test]
fn vfirst_no_set_bit_returns_minus_one() {
    let mut state = setup(16, Vsew::E8, Vlmul::M1);
    set_vreg(&mut state, VReg::V2, [0x00; 16]);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vfirst {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: true,
        },
    )
    .unwrap();
    // -1 sign-extended to XLEN
    assert_eq!(state.regs.read(Reg::A0), u64::MAX);
}

/// vfirst with first bit at position 0
#[test]
fn vfirst_bit_zero() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    set_vreg(&mut state, VReg::V2, [0xFF; 16]);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vfirst {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0);
}

/// vfirst respects vl: a set bit beyond vl is not found
#[test]
fn vfirst_respects_vl() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    // Only bit 5 set - beyond vl=4
    let mut data = [0u8; 16];
    data[0] = 0b00100000;
    set_vreg(&mut state, VReg::V2, data);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vfirst {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(state.regs.read(Reg::A0), u64::MAX);
}

/// vfirst with mask: only active elements are searched
#[test]
fn vfirst_masked_skips_inactive() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    // vs2: bit 0 set, bit 4 set
    let mut vs2 = [0u8; 16];
    vs2[0] = 0b00010001;
    set_vreg(&mut state, VReg::V2, vs2);
    // Mask: elements 2,3,4,5,6,7 active (bits 2-7 = 0b11111100 = 0xFC)
    let mut mask = [0u8; 16];
    mask[0] = 0xFC;
    set_vreg(&mut state, VReg::V0, mask);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vfirst {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: false,
        },
    )
    .unwrap();
    // Bit 0 is inactive (masked out), first active set bit is at position 4
    assert_eq!(state.regs.read(Reg::A0), 4);
}

/// vfirst with vstart > 0 skips elements before vstart
#[test]
fn vfirst_vstart_skips_early() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    // Bits 1 and 5 set
    let mut data = [0u8; 16];
    data[0] = 0b00100010;
    set_vreg(&mut state, VReg::V2, data);
    state.ext_state.set_vstart(3);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vfirst {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: true,
        },
    )
    .unwrap();
    // Bit 1 is before vstart=3, so first found is bit 5
    assert_eq!(state.regs.read(Reg::A0), 5);
}

// vmsbf

/// vmsbf: all bits before the first set bit in vs2 are set
#[test]
fn vmsbf_first_at_position_3() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    // First set bit at position 3
    let mut vs2 = [0u8; 16];
    vs2[0] = 0b00001000;
    set_vreg(&mut state, VReg::V2, vs2);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vmsbf {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
        },
    )
    .unwrap();
    // Bits 0,1,2 should be set; bits 3..7 should be clear
    for i in 0..3 {
        assert!(mask_bit(&state, VReg::V4, i), "bit {i} should be set");
    }
    for i in 3..8 {
        assert!(!mask_bit(&state, VReg::V4, i), "bit {i} should be clear");
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

/// vmsbf when no bit is set in vs2: all active elements in vd are set
#[test]
fn vmsbf_no_set_bit() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    set_vreg(&mut state, VReg::V2, [0x00; 16]);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vmsbf {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
        },
    )
    .unwrap();
    for i in 0..8 {
        assert!(mask_bit(&state, VReg::V4, i), "bit {i} should be set");
    }
}

/// vmsbf when the first bit is at position 0: no bits are set in vd
#[test]
fn vmsbf_first_at_position_zero() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    let mut vs2 = [0u8; 16];
    vs2[0] = 0x01;
    set_vreg(&mut state, VReg::V2, vs2);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vmsbf {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
        },
    )
    .unwrap();
    for i in 0..8 {
        assert!(!mask_bit(&state, VReg::V4, i), "bit {i} should be clear");
    }
}

/// vmsbf respects the mask: inactive elements are left undisturbed
#[test]
fn vmsbf_masked_inactive_undisturbed() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    // First set bit in vs2 at position 4
    let mut vs2 = [0u8; 16];
    vs2[0] = 0b00010000;
    set_vreg(&mut state, VReg::V2, vs2);
    // Pre-set vd to all-ones so we can detect undisturbed bits
    set_vreg(&mut state, VReg::V4, [0xFF; 16]);
    // Mask: elements 2,3,4,5,6,7 active (bits 2-7 = 0xFC)
    let mut mask = [0u8; 16];
    mask[0] = 0xFC;
    set_vreg(&mut state, VReg::V0, mask);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vmsbf {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: false,
        },
    )
    .unwrap();
    // Elements 0,1 inactive → undisturbed (remain 1)
    assert!(
        mask_bit(&state, VReg::V4, 0),
        "inactive bit 0 must be undisturbed"
    );
    assert!(
        mask_bit(&state, VReg::V4, 1),
        "inactive bit 1 must be undisturbed"
    );
    // Elements 2,3 active and before the first set bit (4) → set
    assert!(
        mask_bit(&state, VReg::V4, 2),
        "bit 2 should be set (before first)"
    );
    assert!(
        mask_bit(&state, VReg::V4, 3),
        "bit 3 should be set (before first)"
    );
    // Element 4 active and is the first set bit → clear
    assert!(
        !mask_bit(&state, VReg::V4, 4),
        "bit 4 should be clear (is first)"
    );
    // Elements 5,6,7 active and after first set bit → clear
    for i in 5..8 {
        assert!(
            !mask_bit(&state, VReg::V4, i),
            "bit {i} should be clear (after first)"
        );
    }
}

/// vmsbf rejects vd == vs2
#[test]
fn vmsbf_vd_eq_vs2_illegal() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xMaskInstruction::Vmsbf {
            vd: VReg::V2,
            vs2: VReg::V2,
            vm: true,
        },
    );
    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

/// vmsbf rejects vd == v0 when masked
#[test]
fn vmsbf_vd_eq_v0_masked_illegal() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xMaskInstruction::Vmsbf {
            vd: VReg::V0,
            vs2: VReg::V2,
            vm: false,
        },
    );
    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

// vmsof

/// vmsof: only the first set bit position is set in vd
#[test]
fn vmsof_first_at_position_3() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    // Set bits at positions 3 and 6
    let mut vs2 = [0u8; 16];
    vs2[0] = 0b01001000;
    set_vreg(&mut state, VReg::V2, vs2);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vmsof {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
        },
    )
    .unwrap();
    // Only bit 3 should be set
    for i in 0..8 {
        assert_eq!(mask_bit(&state, VReg::V4, i), i == 3, "bit {i}");
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

/// vmsof when no bit is set: all active elements in vd are clear
#[test]
fn vmsof_no_set_bit() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    set_vreg(&mut state, VReg::V2, [0x00; 16]);
    set_vreg(&mut state, VReg::V4, [0xFF; 16]);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vmsof {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
        },
    )
    .unwrap();
    for i in 0..8 {
        assert!(!mask_bit(&state, VReg::V4, i), "bit {i} should be clear");
    }
}

/// vmsof rejects vd == vs2
#[test]
fn vmsof_vd_eq_vs2_illegal() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xMaskInstruction::Vmsof {
            vd: VReg::V2,
            vs2: VReg::V2,
            vm: true,
        },
    );
    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

/// vmsof rejects vd == v0 when masked
#[test]
fn vmsof_vd_eq_v0_masked_illegal() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xMaskInstruction::Vmsof {
            vd: VReg::V0,
            vs2: VReg::V2,
            vm: false,
        },
    );
    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

/// vmsof with mask: inactive elements are undisturbed
#[test]
fn vmsof_masked_inactive_undisturbed() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    // First set bit in vs2 at position 2
    let mut vs2 = [0u8; 16];
    vs2[0] = 0b00000100;
    set_vreg(&mut state, VReg::V2, vs2);
    // vd pre-set to all-ones
    set_vreg(&mut state, VReg::V4, [0xFF; 16]);
    // Mask: elements 2..8 active (bits 2-7 = 0xFC)
    let mut mask = [0u8; 16];
    mask[0] = 0xFC;
    set_vreg(&mut state, VReg::V0, mask);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vmsof {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: false,
        },
    )
    .unwrap();
    // Elements 0,1 inactive: undisturbed (remain 1)
    assert!(mask_bit(&state, VReg::V4, 0));
    assert!(mask_bit(&state, VReg::V4, 1));
    // Element 2 active and is the first set bit: set
    assert!(mask_bit(&state, VReg::V4, 2));
    // Elements 3..8 active but after first: clear
    for i in 3..8 {
        assert!(!mask_bit(&state, VReg::V4, i), "bit {i}");
    }
}

// vmsif

/// vmsif: bits up to and including the first set position are set
#[test]
fn vmsif_first_at_position_3() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    // First set bit at position 3
    let mut vs2 = [0u8; 16];
    vs2[0] = 0b00001000;
    set_vreg(&mut state, VReg::V2, vs2);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vmsif {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
        },
    )
    .unwrap();
    // Bits 0,1,2,3 should be set; bits 4..7 clear
    for i in 0..=3 {
        assert!(mask_bit(&state, VReg::V4, i), "bit {i} should be set");
    }
    for i in 4..8 {
        assert!(!mask_bit(&state, VReg::V4, i), "bit {i} should be clear");
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

/// vmsif when no bit is set: all active elements are set
#[test]
fn vmsif_no_set_bit() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    set_vreg(&mut state, VReg::V2, [0x00; 16]);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vmsif {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
        },
    )
    .unwrap();
    for i in 0..8 {
        assert!(mask_bit(&state, VReg::V4, i), "bit {i} should be set");
    }
}

/// vmsif when the first bit is at position 0: only bit 0 is set
#[test]
fn vmsif_first_at_position_zero() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    let mut vs2 = [0u8; 16];
    vs2[0] = 0x01;
    set_vreg(&mut state, VReg::V2, vs2);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vmsif {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
        },
    )
    .unwrap();
    assert!(mask_bit(&state, VReg::V4, 0), "bit 0 should be set");
    for i in 1..8 {
        assert!(!mask_bit(&state, VReg::V4, i), "bit {i} should be clear");
    }
}

/// vmsif rejects vd == vs2
#[test]
fn vmsif_vd_eq_vs2_illegal() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xMaskInstruction::Vmsif {
            vd: VReg::V2,
            vs2: VReg::V2,
            vm: true,
        },
    );
    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

/// vmsif rejects vd == v0 when masked
#[test]
fn vmsif_vd_eq_v0_masked_illegal() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xMaskInstruction::Vmsif {
            vd: VReg::V0,
            vs2: VReg::V2,
            vm: false,
        },
    );
    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

/// Cross-check: vmsbf, vmsof, and vmsif on the same input give the expected relationship.
///
/// For vs2 with first set bit at position k:
///   vmsbf[i] = i < k
///   vmsof[i] = i == k
///   vmsif[i] = i <= k
#[test]
fn vmsbf_vmsof_vmsif_relationship() {
    let k = 5u32;
    let vl = 8u32;

    let mut vs2 = [0u8; 16];
    vs2[(k / u8::BITS) as usize] |= 1 << (k % u8::BITS);

    for i in 0..vl {
        let mut sbf_state = setup(vl, Vsew::E8, Vlmul::M1);
        set_vreg(&mut sbf_state, VReg::V2, vs2);
        exec(
            &mut sbf_state,
            Zve64xMaskInstruction::Vmsbf {
                vd: VReg::V4,
                vs2: VReg::V2,
                vm: true,
            },
        )
        .unwrap();

        let mut sof_state = setup(vl, Vsew::E8, Vlmul::M1);
        set_vreg(&mut sof_state, VReg::V2, vs2);
        exec(
            &mut sof_state,
            Zve64xMaskInstruction::Vmsof {
                vd: VReg::V4,
                vs2: VReg::V2,
                vm: true,
            },
        )
        .unwrap();

        let mut sif_state = setup(vl, Vsew::E8, Vlmul::M1);
        set_vreg(&mut sif_state, VReg::V2, vs2);
        exec(
            &mut sif_state,
            Zve64xMaskInstruction::Vmsif {
                vd: VReg::V4,
                vs2: VReg::V2,
                vm: true,
            },
        )
        .unwrap();

        assert_eq!(
            mask_bit(&sbf_state, VReg::V4, i),
            i < k,
            "vmsbf bit {i}: expected {}",
            i < k
        );
        assert_eq!(
            mask_bit(&sof_state, VReg::V4, i),
            i == k,
            "vmsof bit {i}: expected {}",
            i == k
        );
        assert_eq!(
            mask_bit(&sif_state, VReg::V4, i),
            i <= k,
            "vmsif bit {i}: expected {}",
            i <= k
        );
    }
}

// viota

/// viota: each element receives the count of set bits before it in vs2
#[test]
fn viota_basic_e8_m1() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    // vs2 bits: 0=1, 1=0, 2=1, 3=0, 4=1, 5=0, 6=0, 7=0 → byte = 0b00010101 = 0x15
    let mut vs2 = [0u8; 16];
    vs2[0] = 0x15;
    set_vreg(&mut state, VReg::V2, vs2);
    exec(
        &mut state,
        Zve64xMaskInstruction::Viota {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
        },
    )
    .unwrap();
    // Expected prefix counts (counting vs2 bits strictly before i):
    // i=0: 0  (no bits before 0)
    // i=1: 1  (bit 0 set)
    // i=2: 1  (bit 0 set, bit 1 clear)
    // i=3: 2  (bits 0,2 set)
    // i=4: 2  (bits 0,2 set, bit 3 clear)
    // i=5: 3  (bits 0,2,4 set)
    // i=6: 3
    // i=7: 3
    let expected: [u64; 8] = [0, 1, 1, 2, 2, 3, 3, 3];
    for (i, &exp) in expected.iter().enumerate() {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), exp, "elem {i}");
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

/// viota with SEW=32
#[test]
fn viota_e32_m1() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    // vs2: bits 1 and 2 set → byte = 0b00000110 = 0x06
    let mut vs2 = [0u8; 16];
    vs2[0] = 0x06;
    set_vreg(&mut state, VReg::V2, vs2);
    exec(
        &mut state,
        Zve64xMaskInstruction::Viota {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
        },
    )
    .unwrap();
    // i=0: 0, i=1: 0, i=2: 1, i=3: 2
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E32), 0);
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E32), 1);
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E32), 2);
}

/// viota prefix count advances for *all* vs2 bits, not just active (masked) ones
#[test]
fn viota_prefix_count_ignores_execution_mask() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    // vs2: all bits set
    set_vreg(&mut state, VReg::V2, [0xFF; 16]);
    // Execution mask: only elements 4..8 active (bits 4-7 = 0xF0)
    let mut mask = [0u8; 16];
    mask[0] = 0xF0;
    set_vreg(&mut state, VReg::V0, mask);
    // Pre-set vd to a sentinel so we can confirm inactive elements are undisturbed
    set_vreg(&mut state, VReg::V4, [0xAB; 16]);
    exec(
        &mut state,
        Zve64xMaskInstruction::Viota {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: false,
        },
    )
    .unwrap();
    // Elements 0..4 inactive: undisturbed (0xAB)
    for i in 0..4usize {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E8),
            0xAB,
            "inactive elem {i}"
        );
    }
    // Elements 4..8 active.
    // Prefix count at i=4: counts set bits in vs2[0..4] = 4 (bits 0,1,2,3 all set)
    // i=5: 5, i=6: 6, i=7: 7
    for i in 4..8usize {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E8),
            i as u64,
            "elem {i}"
        );
    }
}

/// viota rejects vd == vs2
#[test]
fn viota_vd_eq_vs2_illegal() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xMaskInstruction::Viota {
            vd: VReg::V2,
            vs2: VReg::V2,
            vm: true,
        },
    );
    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

/// viota rejects vd == v0 when masked
#[test]
fn viota_vd_eq_v0_masked_illegal() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xMaskInstruction::Viota {
            vd: VReg::V0,
            vs2: VReg::V2,
            vm: false,
        },
    );
    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

/// viota rejects misaligned vd for the current LMUL
#[test]
fn viota_misaligned_vd_illegal() {
    let mut state = setup(16, Vsew::E8, Vlmul::M2);
    // With M2, vd must be even-numbered; V3 is misaligned
    let result = exec(
        &mut state,
        Zve64xMaskInstruction::Viota {
            vd: VReg::V3,
            vs2: VReg::V2,
            vm: true,
        },
    );
    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

/// viota with vstart > 0: elements before vstart are undisturbed
#[test]
fn viota_vstart_undisturbed_below() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    // vs2: all bits clear → prefix counts are all zero
    set_vreg(&mut state, VReg::V2, [0x00; 16]);
    // Pre-set vd to sentinel
    set_vreg(&mut state, VReg::V4, [0xCD; 16]);
    state.ext_state.set_vstart(4);
    exec(
        &mut state,
        Zve64xMaskInstruction::Viota {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
        },
    )
    .unwrap();
    // Elements 0..4: undisturbed
    for i in 0..4usize {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0xCD, "elem {i}");
    }
    // Elements 4..8: written (prefix count = 0 for all, since vs2 is all clear)
    for i in 4..8usize {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0, "elem {i}");
    }
}

// vid

/// vid.v: each element receives its own index
#[test]
fn vid_basic_e8_m1() {
    let mut state = setup(16, Vsew::E8, Vlmul::M1);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vid {
            vd: VReg::V4,
            vm: true,
        },
    )
    .unwrap();
    for i in 0..16usize {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E8),
            i as u64,
            "elem {i}"
        );
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

/// vid.v with SEW=16
#[test]
fn vid_e16_m1() {
    let mut state = setup(8, Vsew::E16, Vlmul::M1);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vid {
            vd: VReg::V4,
            vm: true,
        },
    )
    .unwrap();
    for i in 0..8usize {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E16),
            i as u64,
            "elem {i}"
        );
    }
}

/// vid.v with SEW=32
#[test]
fn vid_e32_m1() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vid {
            vd: VReg::V4,
            vm: true,
        },
    )
    .unwrap();
    for i in 0..4usize {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E32),
            i as u64,
            "elem {i}"
        );
    }
}

/// vid.v with SEW=64
#[test]
fn vid_e64_m1() {
    let mut state = setup(2, Vsew::E64, Vlmul::M1);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vid {
            vd: VReg::V4,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E64), 0);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E64), 1);
}

/// vid.v respects vl: elements at or beyond vl are not written
#[test]
fn vid_respects_vl() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    // Pre-set entire vd register to sentinel value
    set_vreg(&mut state, VReg::V4, [0xEE; 16]);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vid {
            vd: VReg::V4,
            vm: true,
        },
    )
    .unwrap();
    // First 4 elements written with their index
    for i in 0..4usize {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E8),
            i as u64,
            "elem {i}"
        );
    }
    // Remaining elements undisturbed
    for i in 4..16usize {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0xEE, "elem {i}");
    }
}

/// vid.v with mask: inactive elements are undisturbed
#[test]
fn vid_masked_inactive_undisturbed() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    // Pre-set vd to sentinel
    set_vreg(&mut state, VReg::V4, [0xBE; 16]);
    // Mask: only even elements active (bits 0,2,4,6 = 0b01010101 = 0x55)
    let mut mask = [0u8; 16];
    mask[0] = 0x55;
    set_vreg(&mut state, VReg::V0, mask);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vid {
            vd: VReg::V4,
            vm: false,
        },
    )
    .unwrap();
    for i in 0..8usize {
        if i % 2 == 0 {
            assert_eq!(
                read_elem(&state, VReg::V4, i, Vsew::E8),
                i as u64,
                "active elem {i}"
            );
        } else {
            assert_eq!(
                read_elem(&state, VReg::V4, i, Vsew::E8),
                0xBE,
                "inactive elem {i}"
            );
        }
    }
}

/// vid.v rejects vd == v0 when masked
#[test]
fn vid_vd_eq_v0_masked_illegal() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xMaskInstruction::Vid {
            vd: VReg::V0,
            vm: false,
        },
    );
    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

/// vid.v rejects misaligned vd for the current LMUL
#[test]
fn vid_misaligned_vd_illegal() {
    let mut state = setup(16, Vsew::E8, Vlmul::M2);
    // With M2, vd must be even-numbered; V3 is misaligned
    let result = exec(
        &mut state,
        Zve64xMaskInstruction::Vid {
            vd: VReg::V3,
            vm: true,
        },
    );
    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

/// vid.v with vstart > 0: elements before vstart are undisturbed
#[test]
fn vid_vstart_undisturbed_below() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    set_vreg(&mut state, VReg::V4, [0xFF; 16]);
    state.ext_state.set_vstart(4);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vid {
            vd: VReg::V4,
            vm: true,
        },
    )
    .unwrap();
    // Elements 0..4: undisturbed
    for i in 0..4usize {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0xFF, "elem {i}");
    }
    // Elements 4..8: written with index
    for i in 4..8usize {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E8),
            i as u64,
            "elem {i}"
        );
    }
}

/// vid.v requires a valid vtype
#[test]
fn vid_invalid_vtype() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    state.ext_state.set_vtype(None);
    let result = exec(
        &mut state,
        Zve64xMaskInstruction::Vid {
            vd: VReg::V4,
            vm: true,
        },
    );
    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

/// vid.v requires vector instructions to be allowed
#[test]
fn vid_vector_not_allowed() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    state.ext_state.set_vector_allowed(false);
    let result = exec(
        &mut state,
        Zve64xMaskInstruction::Vid {
            vd: VReg::V4,
            vm: true,
        },
    );
    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

// vl=0 edge cases

/// With vl=0, vcpop returns 0
#[test]
fn vcpop_vl_zero() {
    let mut state = setup(0, Vsew::E8, Vlmul::M1);
    set_vreg(&mut state, VReg::V2, [0xFF; 16]);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vcpop {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0);
}

/// With vl=0, vfirst returns -1
#[test]
fn vfirst_vl_zero() {
    let mut state = setup(0, Vsew::E8, Vlmul::M1);
    set_vreg(&mut state, VReg::V2, [0xFF; 16]);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vfirst {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(state.regs.read(Reg::A0), u64::MAX);
}

/// With vl=0, vmsbf writes nothing and vd is untouched
#[test]
fn vmsbf_vl_zero() {
    let mut state = setup(0, Vsew::E8, Vlmul::M1);
    set_vreg(&mut state, VReg::V2, [0xFF; 16]);
    set_vreg(&mut state, VReg::V4, [0xAB; 16]);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vmsbf {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(get_vreg(&state, VReg::V4), [0xAB; 16]);
}

/// With vl=0, vid writes nothing and vd is untouched
#[test]
fn vid_vl_zero() {
    let mut state = setup(0, Vsew::E8, Vlmul::M1);
    set_vreg(&mut state, VReg::V4, [0xCD; 16]);
    exec(
        &mut state,
        Zve64xMaskInstruction::Vid {
            vd: VReg::V4,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(get_vreg(&state, VReg::V4), [0xCD; 16]);
}

// vs_dirty and vstart invariants

/// Every instruction marks VS dirty and resets vstart, even when vl=0
#[test]
fn all_instructions_mark_vs_dirty_and_reset_vstart() {
    let instructions: &[Zve64xMaskInstruction<Reg<u64>>] = &[
        Zve64xMaskInstruction::Vmand {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
        },
        Zve64xMaskInstruction::Vmor {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
        },
        Zve64xMaskInstruction::Vmxor {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
        },
        Zve64xMaskInstruction::Vmandn {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
        },
        Zve64xMaskInstruction::Vmorn {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
        },
        Zve64xMaskInstruction::Vmnand {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
        },
        Zve64xMaskInstruction::Vmnor {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
        },
        Zve64xMaskInstruction::Vmxnor {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
        },
        Zve64xMaskInstruction::Vcpop {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: true,
        },
        Zve64xMaskInstruction::Vfirst {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: true,
        },
        Zve64xMaskInstruction::Vmsbf {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
        },
        Zve64xMaskInstruction::Vmsof {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
        },
        Zve64xMaskInstruction::Vmsif {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
        },
        Zve64xMaskInstruction::Viota {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
        },
        Zve64xMaskInstruction::Vid {
            vd: VReg::V4,
            vm: true,
        },
    ];

    for (idx, &instr) in instructions.iter().enumerate() {
        let mut state = setup(4, Vsew::E8, Vlmul::M1);
        state.ext_state.set_vstart(2);
        exec(&mut state, instr).unwrap();
        assert_eq!(
            state.ext_state.vs_dirty_count(),
            1,
            "instruction {idx}: vs_dirty"
        );
        assert_eq!(
            state.ext_state.vstart(),
            0,
            "instruction {idx}: vstart reset"
        );
    }
}

/// Mask-logical ops reject execution when vtype is invalid (vill=1).
/// All eight ops are verified; vmand is representative, the others are spot-checked.
#[test]
fn mask_logical_invalid_vtype() {
    let ops: &[Zve64xMaskInstruction<Reg<u64>>] = &[
        Zve64xMaskInstruction::Vmand {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
        },
        Zve64xMaskInstruction::Vmor {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
        },
        Zve64xMaskInstruction::Vmxor {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
        },
        Zve64xMaskInstruction::Vmandn {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
        },
        Zve64xMaskInstruction::Vmorn {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
        },
        Zve64xMaskInstruction::Vmnand {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
        },
        Zve64xMaskInstruction::Vmnor {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
        },
        Zve64xMaskInstruction::Vmxnor {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
        },
    ];
    for (idx, &op) in ops.iter().enumerate() {
        let mut state = setup(4, Vsew::E8, Vlmul::M1);
        state.ext_state.set_vtype(None);
        let result = exec(&mut state, op);
        assert!(
            matches!(result, Err(ExecutionError::IllegalInstruction { .. })),
            "op {idx} should reject vill=1"
        );
    }
}
