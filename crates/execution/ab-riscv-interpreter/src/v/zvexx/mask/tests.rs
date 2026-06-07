use crate::rv64::test_utils::{TestInterpreterState, initialize_state};
use crate::v::vector_registers::{VectorRegisters, VectorRegistersExt};
use crate::{
    ExecutableInstruction, ExecutableInstructionOperands, ExecutionError, RegisterFile,
    Rs1Rs2OperandValues, Rs1Rs2Operands,
};
use ab_riscv_primitives::prelude::*;

// With TEST_VLEN=256, VLENB=32:
//   E8/M1 -> VLMAX=32, 1 reg
//   E16/M1 -> VLMAX=16, 1 reg
//   E32/M1 -> VLMAX=8, 1 reg
//   E64/M1 -> VLMAX=4, 1 reg
//   E8/M8 -> VLMAX=256, 8 regs

// helpers

fn encode_vtype(vsew: Vsew, vlmul: Vlmul) -> u64 {
    u64::from(vlmul.to_bits()) | (u64::from(vsew.to_bits()) << 3)
}

fn setup(
    vl: u32,
    vsew: Vsew,
    vlmul: Vlmul,
) -> TestInterpreterState<ZveXxMaskInstruction<Reg<u64>>> {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    let vtype = Vtype::from_raw::<Reg<u64>>(encode_vtype(vsew, vlmul)).unwrap();
    state.ext_state.set_vtype(Some(vtype));
    state.ext_state.set_vl(vl);
    state.ext_state.set_vstart(0);
    state
}

fn exec(
    state: &mut TestInterpreterState<ZveXxMaskInstruction<Reg<u64>>>,
    instr: ZveXxMaskInstruction<Reg<u64>>,
) -> Result<(), ExecutionError<u64>> {
    let Rs1Rs2Operands { rs1, rs2 } = instr.get_rs1_rs2_operands();
    let rs1rs2_values = Rs1Rs2OperandValues {
        rs1_value: state.regs.read(rs1),
        rs2_value: state.regs.read(rs2),
    };

    instr
        .execute(
            rs1rs2_values,
            &mut state.regs,
            &mut state.ext_state,
            &mut state.memory,
            &mut state.instruction_fetcher,
            &mut state.system_instruction_handler,
        )
        .map(|_| ())
}

fn get_vreg(state: &TestInterpreterState<ZveXxMaskInstruction<Reg<u64>>>, reg: VReg) -> [u8; 32] {
    *state.ext_state.read_vregs().get(reg)
}

fn set_vreg(
    state: &mut TestInterpreterState<ZveXxMaskInstruction<Reg<u64>>>,
    reg: VReg,
    data: [u8; 32],
) {
    *state.ext_state.write_vregs().get_mut(reg) = data;
}

/// Read element `i` from a register group as a u64 (zero-extended), given SEW
fn read_elem(
    state: &TestInterpreterState<ZveXxMaskInstruction<Reg<u64>>>,
    base_reg: VReg,
    elem_i: usize,
    sew: Vsew,
) -> u64 {
    let sew_bytes = usize::from(sew.bytes_width());
    let elems_per_reg = 32 / sew_bytes;
    let reg_off = elem_i / elems_per_reg;
    let byte_off = (elem_i % elems_per_reg) * sew_bytes;
    let reg = state
        .ext_state
        .read_vregs()
        .get(VReg::from_bits(base_reg.to_bits() + reg_off as u8).unwrap());
    let mut buf = [0u8; 8];
    buf[..sew_bytes].copy_from_slice(&reg[byte_off..byte_off + sew_bytes]);
    u64::from_le_bytes(buf)
}

/// Read mask bit `i` from a vector register
fn mask_bit(
    state: &TestInterpreterState<ZveXxMaskInstruction<Reg<u64>>>,
    reg: VReg,
    i: u32,
) -> bool {
    let byte = state.ext_state.read_vregs().get(reg)[(i / u8::BITS) as usize];
    (byte >> (i % u8::BITS)) & 1 != 0
}

// mask-logical
// E8/M8 gives VLMAX=256, so the whole 32-byte mask register is body and the per-byte
// assertions below exercise every bit of the logical operation.
#[test]
fn vmand_basic() {
    let mut state = setup(256, Vsew::E8, Vlmul::M8);
    // vs2 = 0b10101010, vs1 = 0b11001100
    set_vreg(&mut state, VReg::V2, [0xAA; 32]);
    set_vreg(&mut state, VReg::V1, [0xCC; 32]);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vmand {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 0xAA & 0xCC = 0x88
    assert_eq!(get_vreg(&state, VReg::V4), [0x88; 32]);
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vmor_basic() {
    let mut state = setup(256, Vsew::E8, Vlmul::M8);
    set_vreg(&mut state, VReg::V2, [0xAA; 32]);
    set_vreg(&mut state, VReg::V1, [0x55; 32]);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vmor {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 0xAA | 0x55 = 0xFF
    assert_eq!(get_vreg(&state, VReg::V4), [0xFF; 32]);
}

#[test]
fn vmxor_basic() {
    let mut state = setup(256, Vsew::E8, Vlmul::M8);
    set_vreg(&mut state, VReg::V2, [0xF0; 32]);
    set_vreg(&mut state, VReg::V1, [0xFF; 32]);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vmxor {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 0xF0 ^ 0xFF = 0x0F
    assert_eq!(get_vreg(&state, VReg::V4), [0x0F; 32]);
}

#[test]
fn vmandn_basic() {
    let mut state = setup(256, Vsew::E8, Vlmul::M8);
    // vmandn: vd = vs2 AND NOT vs1
    set_vreg(&mut state, VReg::V2, [0xFF; 32]);
    set_vreg(&mut state, VReg::V1, [0x0F; 32]);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vmandn {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 0xFF & !0x0F = 0xF0
    assert_eq!(get_vreg(&state, VReg::V4), [0xF0; 32]);
}

#[test]
fn vmorn_basic() {
    let mut state = setup(256, Vsew::E8, Vlmul::M8);
    // vmorn: vd = vs2 OR NOT vs1
    set_vreg(&mut state, VReg::V2, [0x00; 32]);
    set_vreg(&mut state, VReg::V1, [0x0F; 32]);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vmorn {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 0x00 | !0x0F = 0xF0
    assert_eq!(get_vreg(&state, VReg::V4), [0xF0; 32]);
}

#[test]
fn vmnand_basic() {
    let mut state = setup(256, Vsew::E8, Vlmul::M8);
    // vmnand: vd = NOT(vs2 AND vs1)
    set_vreg(&mut state, VReg::V2, [0xFF; 32]);
    set_vreg(&mut state, VReg::V1, [0xFF; 32]);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vmnand {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // NOT(0xFF & 0xFF) = 0x00
    assert_eq!(get_vreg(&state, VReg::V4), [0x00; 32]);
}

#[test]
fn vmnor_basic() {
    let mut state = setup(256, Vsew::E8, Vlmul::M8);
    // vmnor: vd = NOT(vs2 OR vs1)
    set_vreg(&mut state, VReg::V2, [0x00; 32]);
    set_vreg(&mut state, VReg::V1, [0x00; 32]);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vmnor {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // NOT(0x00 | 0x00) = 0xFF
    assert_eq!(get_vreg(&state, VReg::V4), [0xFF; 32]);
}

#[test]
fn vmxnor_basic() {
    let mut state = setup(256, Vsew::E8, Vlmul::M8);
    // vmxnor: vd = NOT(vs2 XOR vs1)
    set_vreg(&mut state, VReg::V2, [0xAA; 32]);
    set_vreg(&mut state, VReg::V1, [0xAA; 32]);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vmxnor {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // NOT(0xAA ^ 0xAA) = NOT(0x00) = 0xFF
    assert_eq!(get_vreg(&state, VReg::V4), [0xFF; 32]);
}

/// Mask-logical ops compute only the body [0, vl); bits at or past vl are tail-agnostic and
/// left undisturbed here. This is the core property the certification suite checks.
#[test]
fn vmand_respects_vl_tail_undisturbed() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    set_vreg(&mut state, VReg::V2, [0xFF; 32]);
    set_vreg(&mut state, VReg::V1, [0xAA; 32]);
    // Pre-set vd so undisturbed tail bytes are detectable
    set_vreg(&mut state, VReg::V4, [0x33; 32]);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vmand {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // Body is the first 8 mask bits = byte 0: 0xFF & 0xAA = 0xAA
    assert_eq!(get_vreg(&state, VReg::V4)[0], 0xAA);
    // Bytes 1..32 are past vl: undisturbed
    for b in 1..32usize {
        assert_eq!(get_vreg(&state, VReg::V4)[b], 0x33, "tail byte {b}");
    }
}

/// Mask-logical ops honor vstart: prestart bits [0, vstart) are undisturbed.
#[test]
fn vmand_respects_vstart_prestart_undisturbed() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    set_vreg(&mut state, VReg::V2, [0xFF; 32]);
    set_vreg(&mut state, VReg::V1, [0xFF; 32]);
    set_vreg(&mut state, VReg::V4, [0x00; 32]);
    state.ext_state.set_vstart(4);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vmand {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // Bits 0..4 prestart: undisturbed (0)
    for i in 0..4 {
        assert!(!mask_bit(&state, VReg::V4, i), "prestart bit {i}");
    }
    // Bits 4..8 body: 0xFF & 0xFF = 1
    for i in 4..8 {
        assert!(mask_bit(&state, VReg::V4, i), "body bit {i}");
    }
    assert_eq!(state.ext_state.vstart(), 0);
}

/// With vl=0, mask-logical ops write nothing; vd is undisturbed but VS is still marked dirty.
#[test]
fn vmand_vl_zero_undisturbed() {
    let mut state = setup(0, Vsew::E8, Vlmul::M1);
    set_vreg(&mut state, VReg::V2, [0xFF; 32]);
    set_vreg(&mut state, VReg::V1, [0xAA; 32]);
    set_vreg(&mut state, VReg::V4, [0x5C; 32]);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vmand {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(get_vreg(&state, VReg::V4), [0x5C; 32]);
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
}

/// Regression for the Vx16-vmand.mm certification failure. The cert uses SEW=16, LMUL=1/4,
/// which with VLEN=256 yields VLMAX=4, so vl=4. The same effective body [0, 4) is reproduced
/// here with E16/M1, vl=4 (no dependence on the fractional-LMUL enum). The old implementation
/// applied the logical op across the whole physical register, corrupting the tail-agnostic
/// bytes that the certification signature compares as undisturbed; the body bits stayed
/// correct, so only the tail check tripped.
#[test]
fn vmand_cert_regression_tail_undisturbed() {
    let mut state = setup(4, Vsew::E16, Vlmul::M1);
    // In the cert flow vd == vs2 (v13); mirror that overlap here.
    set_vreg(&mut state, VReg::V13, [0xFF; 32]);
    set_vreg(&mut state, VReg::V11, [0x0F; 32]);
    let before = get_vreg(&state, VReg::V13);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vmand {
            vd: VReg::V13,
            vs2: VReg::V13,
            vs1: VReg::V11,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // Body bits 0..4: 0xFF & 0x0F = 1
    for i in 0..4u32 {
        assert!(mask_bit(&state, VReg::V13, i), "body bit {i}");
    }
    let after = get_vreg(&state, VReg::V13);
    // Every bit at or past vl=4 is undisturbed: high nibble of byte 0 plus all higher bytes.
    assert_eq!(after[0] & 0xF0, before[0] & 0xF0, "tail of byte 0");
    for b in 1..32usize {
        assert_eq!(after[b], before[b], "tail byte {b}");
    }
}

/// vd may overlap vs2 for mask-logical ops
#[test]
fn vmand_vd_overlaps_vs2() {
    let mut state = setup(256, Vsew::E8, Vlmul::M8);
    set_vreg(&mut state, VReg::V2, [0xFF; 32]);
    set_vreg(&mut state, VReg::V1, [0x0F; 32]);
    // vd = vs2
    exec(
        &mut state,
        ZveXxMaskInstruction::Vmand {
            vd: VReg::V2,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(get_vreg(&state, VReg::V2), [0x0F; 32]);
}

/// vd may overlap vs1 for mask-logical ops
#[test]
fn vmand_vd_overlaps_vs1() {
    let mut state = setup(256, Vsew::E8, Vlmul::M8);
    set_vreg(&mut state, VReg::V2, [0xFF; 32]);
    set_vreg(&mut state, VReg::V1, [0x0F; 32]);
    // vd = vs1
    exec(
        &mut state,
        ZveXxMaskInstruction::Vmand {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(get_vreg(&state, VReg::V1), [0x0F; 32]);
}

/// vd may be v0 for mask-logical ops (they are always unmasked)
#[test]
fn vmand_vd_is_v0() {
    let mut state = setup(256, Vsew::E8, Vlmul::M8);
    set_vreg(&mut state, VReg::V2, [0xFF; 32]);
    set_vreg(&mut state, VReg::V1, [0xAA; 32]);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vmand {
            vd: VReg::V0,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(get_vreg(&state, VReg::V0), [0xAA; 32]);
}

/// Mask-logical ops require vector instructions to be allowed
#[test]
fn vmand_vector_not_allowed() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    state.ext_state.set_vector_allowed(false);
    let result = exec(
        &mut state,
        ZveXxMaskInstruction::Vmand {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
    set_vreg(&mut state, VReg::V2, [0xFF; 32]);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vcpop {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
    set_vreg(&mut state, VReg::V2, [0x00; 32]);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vcpop {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
    set_vreg(&mut state, VReg::V2, [0xFF; 32]);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vcpop {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
    set_vreg(&mut state, VReg::V2, [0xFF; 32]);
    // mask v0: only elements 0,2,4,6 active (alternating, low nibble = 0b01010101 = 0x55)
    set_vreg(
        &mut state,
        VReg::V0,
        [
            0x55, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
        ],
    );
    exec(
        &mut state,
        ZveXxMaskInstruction::Vcpop {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 4 active elements, all set in vs2 -> count = 4
    assert_eq!(state.regs.read(Reg::A0), 4);
}

/// vcpop with vstart > 0 skips elements before vstart
#[test]
fn vcpop_vstart_skips_early_elements() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    set_vreg(&mut state, VReg::V2, [0xFF; 32]);
    state.ext_state.set_vstart(4);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vcpop {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
        ZveXxMaskInstruction::Vcpop {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
        ZveXxMaskInstruction::Vcpop {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
    let mut data = [0u8; 32];
    // Byte 0: bits 0,3,7 -> 0b1000_1001 = 0x89
    data[0] = 0x89;
    // Byte 1: bits 8+3=11 -> 0b0000_1000 = 0x08
    data[1] = 0x08;
    // bit 15: byte 1 bit 7 -> 0x80
    data[1] |= 0x80;
    set_vreg(&mut state, VReg::V2, data);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vcpop {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
    let mut data = [0u8; 32];
    data[0] = 0b0000_1000;
    set_vreg(&mut state, VReg::V2, data);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vfirst {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(state.regs.read(Reg::A0), 3);
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

/// vfirst with no bits set returns -1 (all-ones for XLEN=64 -> u64::MAX)
#[test]
fn vfirst_no_set_bit_returns_minus_one() {
    let mut state = setup(16, Vsew::E8, Vlmul::M1);
    set_vreg(&mut state, VReg::V2, [0x00; 32]);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vfirst {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
    set_vreg(&mut state, VReg::V2, [0xFF; 32]);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vfirst {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
    let mut data = [0u8; 32];
    data[0] = 0b0010_0000;
    set_vreg(&mut state, VReg::V2, data);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vfirst {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
    let mut vs2 = [0u8; 32];
    vs2[0] = 0b0001_0001;
    set_vreg(&mut state, VReg::V2, vs2);
    // Mask: elements 2,3,4,5,6,7 active (bits 2-7 = 0b11111100 = 0xFC)
    let mut mask = [0u8; 32];
    mask[0] = 0xFC;
    set_vreg(&mut state, VReg::V0, mask);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vfirst {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
    let mut data = [0u8; 32];
    data[0] = 0b0010_0010;
    set_vreg(&mut state, VReg::V2, data);
    state.ext_state.set_vstart(3);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vfirst {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
    let mut vs2 = [0u8; 32];
    vs2[0] = 0b0000_1000;
    set_vreg(&mut state, VReg::V2, vs2);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vmsbf {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
    set_vreg(&mut state, VReg::V2, [0x00; 32]);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vmsbf {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
    let mut vs2 = [0u8; 32];
    vs2[0] = 0x01;
    set_vreg(&mut state, VReg::V2, vs2);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vmsbf {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
    let mut vs2 = [0u8; 32];
    vs2[0] = 0b0001_0000;
    set_vreg(&mut state, VReg::V2, vs2);
    // Pre-set vd to all-ones so we can detect undisturbed bits
    set_vreg(&mut state, VReg::V4, [0xFF; 32]);
    // Mask: elements 2,3,4,5,6,7 active (bits 2-7 = 0xFC)
    let mut mask = [0u8; 32];
    mask[0] = 0xFC;
    set_vreg(&mut state, VReg::V0, mask);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vmsbf {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // Elements 0,1 inactive -> undisturbed (remain 1)
    assert!(
        mask_bit(&state, VReg::V4, 0),
        "inactive bit 0 must be undisturbed"
    );
    assert!(
        mask_bit(&state, VReg::V4, 1),
        "inactive bit 1 must be undisturbed"
    );
    // Elements 2,3 active and before the first set bit (4) -> set
    assert!(
        mask_bit(&state, VReg::V4, 2),
        "bit 2 should be set (before first)"
    );
    assert!(
        mask_bit(&state, VReg::V4, 3),
        "bit 3 should be set (before first)"
    );
    // Element 4 active and is the first set bit -> clear
    assert!(
        !mask_bit(&state, VReg::V4, 4),
        "bit 4 should be clear (is first)"
    );
    // Elements 5,6,7 active and after first set bit -> clear
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
        ZveXxMaskInstruction::Vmsbf {
            vd: VReg::V2,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
        ZveXxMaskInstruction::Vmsbf {
            vd: VReg::V0,
            vs2: VReg::V2,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    );
    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

/// Spec §16.4: vmsbf.m with vstart != 0 is a mandatory illegal instruction exception.
#[test]
fn vmsbf_nonzero_vstart_illegal() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    state.ext_state.set_vstart(1);
    let result = exec(
        &mut state,
        ZveXxMaskInstruction::Vmsbf {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
    let mut vs2 = [0u8; 32];
    vs2[0] = 0b0100_1000;
    set_vreg(&mut state, VReg::V2, vs2);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vmsof {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
    set_vreg(&mut state, VReg::V2, [0x00; 32]);
    set_vreg(&mut state, VReg::V4, [0xFF; 32]);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vmsof {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
        ZveXxMaskInstruction::Vmsof {
            vd: VReg::V2,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
        ZveXxMaskInstruction::Vmsof {
            vd: VReg::V0,
            vs2: VReg::V2,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
    let mut vs2 = [0u8; 32];
    vs2[0] = 0b0000_0100;
    set_vreg(&mut state, VReg::V2, vs2);
    // vd pre-set to all-ones
    set_vreg(&mut state, VReg::V4, [0xFF; 32]);
    // Mask: elements 2..8 active (bits 2-7 = 0xFC)
    let mut mask = [0u8; 32];
    mask[0] = 0xFC;
    set_vreg(&mut state, VReg::V0, mask);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vmsof {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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

/// Spec §16.4: vmsof.m with vstart != 0 is a mandatory illegal instruction exception.
#[test]
fn vmsof_nonzero_vstart_illegal() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    state.ext_state.set_vstart(1);
    let result = exec(
        &mut state,
        ZveXxMaskInstruction::Vmsof {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    );
    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

// vmsif

/// vmsif: bits up to and including the first set position are set
#[test]
fn vmsif_first_at_position_3() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    // First set bit at position 3
    let mut vs2 = [0u8; 32];
    vs2[0] = 0b0000_1000;
    set_vreg(&mut state, VReg::V2, vs2);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vmsif {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
    set_vreg(&mut state, VReg::V2, [0x00; 32]);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vmsif {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
    let mut vs2 = [0u8; 32];
    vs2[0] = 0x01;
    set_vreg(&mut state, VReg::V2, vs2);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vmsif {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
        ZveXxMaskInstruction::Vmsif {
            vd: VReg::V2,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
        ZveXxMaskInstruction::Vmsif {
            vd: VReg::V0,
            vs2: VReg::V2,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    );
    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

/// Spec §16.4: vmsif.m with vstart != 0 is a mandatory illegal instruction exception.
#[test]
fn vmsif_nonzero_vstart_illegal() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    state.ext_state.set_vstart(1);
    let result = exec(
        &mut state,
        ZveXxMaskInstruction::Vmsif {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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

    let mut vs2 = [0u8; 32];
    vs2[(k / u8::BITS) as usize] |= 1 << (k % u8::BITS);

    for i in 0..vl {
        let mut sbf_state = setup(vl, Vsew::E8, Vlmul::M1);
        set_vreg(&mut sbf_state, VReg::V2, vs2);
        exec(
            &mut sbf_state,
            ZveXxMaskInstruction::Vmsbf {
                vd: VReg::V4,
                vs2: VReg::V2,
                vm: true,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
        )
        .unwrap();

        let mut sof_state = setup(vl, Vsew::E8, Vlmul::M1);
        set_vreg(&mut sof_state, VReg::V2, vs2);
        exec(
            &mut sof_state,
            ZveXxMaskInstruction::Vmsof {
                vd: VReg::V4,
                vs2: VReg::V2,
                vm: true,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
        )
        .unwrap();

        let mut sif_state = setup(vl, Vsew::E8, Vlmul::M1);
        set_vreg(&mut sif_state, VReg::V2, vs2);
        exec(
            &mut sif_state,
            ZveXxMaskInstruction::Vmsif {
                vd: VReg::V4,
                vs2: VReg::V2,
                vm: true,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
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
    // vs2 bits: 0=1, 1=0, 2=1, 3=0, 4=1, 5=0, 6=0, 7=0 -> byte = 0b00010101 = 0x15
    let mut vs2 = [0u8; 32];
    vs2[0] = 0x15;
    set_vreg(&mut state, VReg::V2, vs2);
    exec(
        &mut state,
        ZveXxMaskInstruction::Viota {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
    // vs2: bits 1 and 2 set -> byte = 0b00000110 = 0x06
    let mut vs2 = [0u8; 32];
    vs2[0] = 0x06;
    set_vreg(&mut state, VReg::V2, vs2);
    exec(
        &mut state,
        ZveXxMaskInstruction::Viota {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // i=0: 0, i=1: 0, i=2: 1, i=3: 2
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E32), 0);
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E32), 1);
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E32), 2);
}

/// Per spec §16.8, viota honors the source mask: inactive vs2 elements are treated as
/// zero for the prefix sum. Inactive destination elements are mask-agnostic (here:
/// undisturbed).
#[test]
fn viota_inactive_vs2_bits_treated_as_zero() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    // vs2: all bits set
    set_vreg(&mut state, VReg::V2, [0xFF; 32]);
    // Execution mask: only elements 4..8 active (bits 4-7 = 0xF0)
    let mut mask = [0u8; 32];
    mask[0] = 0xF0;
    set_vreg(&mut state, VReg::V0, mask);
    // Pre-set vd to a sentinel so we can confirm inactive elements are undisturbed
    set_vreg(&mut state, VReg::V4, [0xAB; 32]);
    exec(
        &mut state,
        ZveXxMaskInstruction::Viota {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // Elements 0..4 inactive: undisturbed (0xAB), and their vs2 bits count as zero
    // for the prefix sum.
    for i in 0..4usize {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E8),
            0xAB,
            "inactive elem {i}"
        );
    }
    // Elements 4..8 active. Because elements 0..4 are inactive, their vs2 bits contribute zero, so
    // prefix count at i=4 is 0; i=5 is 1 (bit 4 set & active); etc. i=4: 0, i=5: 1, i=6: 2, i=7: 3
    let expected = [0, 1, 2, 3];
    for (k, &exp) in expected.iter().enumerate() {
        let i = 4 + k;
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E8),
            exp,
            "active elem {i}"
        );
    }
}

/// Per spec §16.8: viota.m with vstart != 0 is a mandatory illegal instruction exception.
#[test]
fn viota_nonzero_vstart_illegal() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    state.ext_state.set_vstart(1);
    let result = exec(
        &mut state,
        ZveXxMaskInstruction::Viota {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    );
    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

/// viota rejects vd == vs2
#[test]
fn viota_vd_eq_vs2_illegal() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    let result = exec(
        &mut state,
        ZveXxMaskInstruction::Viota {
            vd: VReg::V2,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
        ZveXxMaskInstruction::Viota {
            vd: VReg::V0,
            vs2: VReg::V2,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
        ZveXxMaskInstruction::Viota {
            vd: VReg::V3,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    );
    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

/// viota.m never raises an illegal-instruction exception because of a narrow SEW. Per spec §16.8
/// the result simply wraps (truncates to SEW) if it does not fit, matching the general "integer
/// operations wrap around on overflow" rule. This exercises the widest element width to confirm
/// the prefix count is written correctly and the instruction is accepted.
///
/// The exact `VLMAX == 2^SEW` boundary that the removed check got wrong is covered by
/// [`viota_e8_m8_vlmax_256_boundary()`].
#[test]
fn viota_e64_m1_no_width_trap() {
    let mut state = setup(2, Vsew::E64, Vlmul::M1);
    let mut vs2 = [0u8; 32];
    vs2[0] = 0b11;
    set_vreg(&mut state, VReg::V2, vs2);
    exec(
        &mut state,
        ZveXxMaskInstruction::Viota {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E64), 0);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E64), 1);
}

/// Regression test for the removed SEW-width trap. At `E8/M8` with `TEST_VLEN=256` the maximum
/// vector length is `VLMAX = 8 * 256 / 8 = 256`, i.e. `VLMAX == 2^SEW` for `SEW=8`. The largest
/// prefix count `viota.m` writes is `VLMAX - 1 = 255`, which fits exactly in an 8-bit element, so
/// the instruction is legal. The removed check rejected this (`256 >> 8 == 1`) and wrongly trapped
/// it - exactly the failure seen for `viota.m` on the ACT4 runner. With every mask bit set and
/// active, element `i` must equal `i` across the full 8-register destination group.
#[test]
fn viota_e8_m8_vlmax_256_boundary() {
    let mut state = setup(256, Vsew::E8, Vlmul::M8);
    // A single mask register holds VLEN bits; with VLENB=32 that is all 256 elements.
    set_vreg(&mut state, VReg::V8, [0xFF; 32]);
    exec(
        &mut state,
        ZveXxMaskInstruction::Viota {
            vd: VReg::V16,
            vs2: VReg::V8,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // Element i counts the set bits strictly before i; with every bit set that is just i. The
    // largest, 255, exercises the boundary where the old width check would have trapped.
    for i in 0..256usize {
        assert_eq!(
            read_elem(&state, VReg::V16, i, Vsew::E8),
            i as u64,
            "elem {i}"
        );
    }
}

// vid

/// vid.v: each element receives its own index
#[test]
fn vid_basic_e8_m1() {
    let mut state = setup(16, Vsew::E8, Vlmul::M1);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vid {
            vd: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
        ZveXxMaskInstruction::Vid {
            vd: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
        ZveXxMaskInstruction::Vid {
            vd: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
        ZveXxMaskInstruction::Vid {
            vd: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
    set_vreg(&mut state, VReg::V4, [0xEE; 32]);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vid {
            vd: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
    set_vreg(&mut state, VReg::V4, [0xBE; 32]);
    // Mask: only even elements active (bits 0,2,4,6 = 0b01010101 = 0x55)
    let mut mask = [0u8; 32];
    mask[0] = 0x55;
    set_vreg(&mut state, VReg::V0, mask);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vid {
            vd: VReg::V4,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
        ZveXxMaskInstruction::Vid {
            vd: VReg::V0,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
        ZveXxMaskInstruction::Vid {
            vd: VReg::V3,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
    set_vreg(&mut state, VReg::V4, [0xFF; 32]);
    state.ext_state.set_vstart(4);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vid {
            vd: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
        ZveXxMaskInstruction::Vid {
            vd: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
        ZveXxMaskInstruction::Vid {
            vd: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
    set_vreg(&mut state, VReg::V2, [0xFF; 32]);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vcpop {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0);
}

/// With vl=0, vfirst returns -1
#[test]
fn vfirst_vl_zero() {
    let mut state = setup(0, Vsew::E8, Vlmul::M1);
    set_vreg(&mut state, VReg::V2, [0xFF; 32]);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vfirst {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(state.regs.read(Reg::A0), u64::MAX);
}

/// With vl=0, vmsbf writes nothing and vd is untouched
#[test]
fn vmsbf_vl_zero() {
    let mut state = setup(0, Vsew::E8, Vlmul::M1);
    set_vreg(&mut state, VReg::V2, [0xFF; 32]);
    set_vreg(&mut state, VReg::V4, [0xAB; 32]);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vmsbf {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(get_vreg(&state, VReg::V4), [0xAB; 32]);
}

/// With vl=0, vid writes nothing and vd is untouched
#[test]
fn vid_vl_zero() {
    let mut state = setup(0, Vsew::E8, Vlmul::M1);
    set_vreg(&mut state, VReg::V4, [0xCD; 32]);
    exec(
        &mut state,
        ZveXxMaskInstruction::Vid {
            vd: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(get_vreg(&state, VReg::V4), [0xCD; 32]);
}

// vs_dirty and vstart invariants

/// Every instruction marks VS dirty and resets vstart (for instructions that accept non-zero
/// vstart)
#[test]
fn all_instructions_mark_vs_dirty_and_reset_vstart() {
    // Instructions that accept vstart != 0
    let vstart_ok: &[ZveXxMaskInstruction<Reg<u64>>] = &[
        ZveXxMaskInstruction::Vmand {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
        ZveXxMaskInstruction::Vmor {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
        ZveXxMaskInstruction::Vmxor {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
        ZveXxMaskInstruction::Vmandn {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
        ZveXxMaskInstruction::Vmorn {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
        ZveXxMaskInstruction::Vmnand {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
        ZveXxMaskInstruction::Vmnor {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
        ZveXxMaskInstruction::Vmxnor {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
        ZveXxMaskInstruction::Vcpop {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
        ZveXxMaskInstruction::Vfirst {
            rd: Reg::A0,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
        ZveXxMaskInstruction::Vid {
            vd: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    ];
    for (idx, &instr) in vstart_ok.iter().enumerate() {
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
    // Instructions that trap on vstart != 0 per spec (§16.4, §16.8) - checked with vstart=0
    let vstart_must_be_zero: &[ZveXxMaskInstruction<Reg<u64>>] = &[
        ZveXxMaskInstruction::Vmsbf {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
        ZveXxMaskInstruction::Vmsof {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
        ZveXxMaskInstruction::Vmsif {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
        ZveXxMaskInstruction::Viota {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    ];
    for (idx, &instr) in vstart_must_be_zero.iter().enumerate() {
        let mut state = setup(4, Vsew::E8, Vlmul::M1);
        exec(&mut state, instr).unwrap();
        assert_eq!(
            state.ext_state.vs_dirty_count(),
            1,
            "vstart=0 instruction {idx}: vs_dirty"
        );
        assert_eq!(state.ext_state.vstart(), 0, "vstart=0 instruction {idx}");
    }
}

/// Mask-logical ops reject execution when vtype is invalid (vill=1).
/// All eight ops are verified; vmand is representative, the others are spot-checked.
#[test]
fn mask_logical_invalid_vtype() {
    let ops: &[ZveXxMaskInstruction<Reg<u64>>] = &[
        ZveXxMaskInstruction::Vmand {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
        ZveXxMaskInstruction::Vmor {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
        ZveXxMaskInstruction::Vmxor {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
        ZveXxMaskInstruction::Vmandn {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
        ZveXxMaskInstruction::Vmorn {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
        ZveXxMaskInstruction::Vmnand {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
        ZveXxMaskInstruction::Vmnor {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
        ZveXxMaskInstruction::Vmxnor {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
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
