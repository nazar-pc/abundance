use crate::rv64::test_utils::{TestInterpreterState, initialize_state};
use crate::v::vector_registers::{VectorRegisters, VectorRegistersExt};
use crate::{
    ExecutableInstruction, ExecutableInstructionOperands, ExecutionError, RegisterFile,
    Rs1Rs2OperandValues, Rs1Rs2Operands,
};
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

fn encode_vtype(vsew: Vsew, vlmul: Vlmul) -> u64 {
    u64::from(vlmul.to_bits()) | (u64::from(vsew.to_bits()) << 3)
}

fn setup(vl: u32, vsew: Vsew, vlmul: Vlmul) -> TestInterpreterState<ZvkbInstruction<Reg<u64>>> {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    let vtype = Vtype::from_raw::<Reg<u64>>(encode_vtype(vsew, vlmul)).unwrap();
    state.ext_state.set_vtype(Some(vtype));
    state.ext_state.set_vl(vl);
    state.ext_state.set_vstart(0);
    state
}

fn exec(
    state: &mut TestInterpreterState<ZvkbInstruction<Reg<u64>>>,
    instr: ZvkbInstruction<Reg<u64>>,
) -> Result<(), ExecutionError<u64>> {
    let Rs1Rs2Operands { rs1, rs2 } = instr.get_rs1_rs2_operands();
    let rs1rs2_values = Rs1Rs2OperandValues {
        rs1_value: state.regs.read(rs1),
        rs2_value: state.regs.read(rs2),
    };
    if let ControlFlow::Continue((rd, rd_value)) = instr.execute(
        rs1rs2_values,
        &mut state.regs,
        &mut state.ext_state,
        &mut state.memory,
        &mut state.instruction_fetcher,
        &mut state.system_instruction_handler,
    )? {
        state.regs.write(rd, rd_value);
    }
    Ok(())
}

fn write_elem(
    state: &mut TestInterpreterState<ZvkbInstruction<Reg<u64>>>,
    base_reg: VReg,
    elem_i: usize,
    sew: Vsew,
    value: u64,
) {
    let sew_bytes = usize::from(sew.bytes_width());
    let elems_per_reg = 32 / sew_bytes;
    let reg_off = elem_i / elems_per_reg;
    let byte_off = (elem_i % elems_per_reg) * sew_bytes;
    let reg = state
        .ext_state
        .write_vregs()
        .get_mut(VReg::from_bits(base_reg.to_bits() + reg_off as u8).unwrap());
    let buf = value.to_le_bytes();
    reg[byte_off..byte_off + sew_bytes].copy_from_slice(&buf[..sew_bytes]);
}

fn read_elem(
    state: &TestInterpreterState<ZvkbInstruction<Reg<u64>>>,
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

fn set_mask_bit(
    state: &mut TestInterpreterState<ZvkbInstruction<Reg<u64>>>,
    reg: VReg,
    i: u32,
    value: bool,
) {
    let byte = &mut state.ext_state.write_vregs().get_mut(reg)[(i / u8::BITS) as usize];
    if value {
        *byte |= 1 << (i % u8::BITS);
    } else {
        *byte &= !(1 << (i % u8::BITS));
    }
}

// Masking: the primary failure class
//
// These tests directly reproduce the cp_masking_edges (Test v0 = zeroes) pattern:
// vm=false, v0=all-zeros -> every element is masked off -> vd must be entirely undisturbed.

#[test]
fn vandn_vv_masked_v0_zeroes_undisturbed() {
    let mut state = setup(4, Vsew::E16, Vlmul::M1);
    // Preload vd with a sentinel
    for i in 0..4 {
        write_elem(&mut state, VReg::V4, i, Vsew::E16, 0xBEEF);
    }
    // v0 = all zeros: every mask bit is 0
    for i in 0..4 {
        set_mask_bit(&mut state, VReg::V0, i, false);
    }
    // Both sources have real values; none should appear in vd
    for i in 0..4 {
        write_elem(&mut state, VReg::V2, i, Vsew::E16, 0xFF00);
        write_elem(&mut state, VReg::V1, i, Vsew::E16, 0x00FF);
    }
    exec(
        &mut state,
        ZvkbInstruction::VandnVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4 {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E16),
            0xBEEF,
            "elem {i}: masked element should be undisturbed"
        );
    }
}

#[test]
fn vandn_vx_masked_v0_zeroes_undisturbed() {
    let mut state = setup(4, Vsew::E16, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V4, i, Vsew::E16, 0xDEAD);
        write_elem(&mut state, VReg::V2, i, Vsew::E16, 0xFF00);
    }
    state.regs.write(Reg::A0, 0x00FF);
    // v0 = all zeros
    for i in 0..4 {
        set_mask_bit(&mut state, VReg::V0, i, false);
    }
    exec(
        &mut state,
        ZvkbInstruction::VandnVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: false,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4 {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E16),
            0xDEAD,
            "elem {i}"
        );
    }
}

#[test]
fn vbrev8_masked_v0_zeroes_undisturbed() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 0xAA);
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0xFF);
    }
    for i in 0..4 {
        set_mask_bit(&mut state, VReg::V0, i, false);
    }
    exec(
        &mut state,
        ZvkbInstruction::Vbrev8V {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4 {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0xAA, "elem {i}");
    }
}

#[test]
fn vrev8_masked_v0_zeroes_undisturbed() {
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    for i in 0..2 {
        write_elem(&mut state, VReg::V4, i, Vsew::E32, 0xCAFE_BABE);
        write_elem(&mut state, VReg::V2, i, Vsew::E32, 0x0102_0304);
    }
    for i in 0..2 {
        set_mask_bit(&mut state, VReg::V0, i, false);
    }
    exec(
        &mut state,
        ZvkbInstruction::Vrev8V {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..2 {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E32),
            0xCAFE_BABE,
            "elem {i}"
        );
    }
}

#[test]
fn vrol_vv_masked_v0_zeroes_undisturbed() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 0x55);
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0xA5);
        write_elem(&mut state, VReg::V1, i, Vsew::E8, 3);
    }
    for i in 0..4 {
        set_mask_bit(&mut state, VReg::V0, i, false);
    }
    exec(
        &mut state,
        ZvkbInstruction::VrolVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4 {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0x55, "elem {i}");
    }
}

#[test]
fn vrol_vx_masked_v0_zeroes_undisturbed() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 0x99);
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0x01);
    }
    state.regs.write(Reg::A0, 4);
    for i in 0..4 {
        set_mask_bit(&mut state, VReg::V0, i, false);
    }
    exec(
        &mut state,
        ZvkbInstruction::VrolVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: false,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4 {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0x99, "elem {i}");
    }
}

#[test]
fn vror_vv_masked_v0_zeroes_undisturbed() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 0x77);
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0xB3);
        write_elem(&mut state, VReg::V1, i, Vsew::E8, 3);
    }
    for i in 0..4 {
        set_mask_bit(&mut state, VReg::V0, i, false);
    }
    exec(
        &mut state,
        ZvkbInstruction::VrorVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4 {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0x77, "elem {i}");
    }
}

#[test]
fn vror_vx_masked_v0_zeroes_undisturbed() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 0x33);
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0x80);
    }
    state.regs.write(Reg::A0, 7);
    for i in 0..4 {
        set_mask_bit(&mut state, VReg::V0, i, false);
    }
    exec(
        &mut state,
        ZvkbInstruction::VrorVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: false,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4 {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0x33, "elem {i}");
    }
}

// vror.vi with uimm < 32 (bit[25]=0): vm=false -> masked; v0=all-zeros -> undisturbed
#[test]
fn vror_vi_small_uimm_respects_masking() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 0x55);
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0x80);
    }
    for i in 0..4 {
        set_mask_bit(&mut state, VReg::V0, i, false);
    }
    exec(
        &mut state,
        ZvkbInstruction::VrorVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            uimm: 7,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // All elements masked off: vd undisturbed
    for i in 0..4 {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0x55, "elem {i}");
    }
}

// Partial masking: alternating mask bits
#[test]
fn vandn_vv_partial_mask_alternating() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 0xAA);
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0xFF);
        write_elem(&mut state, VReg::V1, i, Vsew::E8, 0x00);
    }
    // Elements 0,2 active; elements 1,3 masked off
    set_mask_bit(&mut state, VReg::V0, 0, true);
    set_mask_bit(&mut state, VReg::V0, 1, false);
    set_mask_bit(&mut state, VReg::V0, 2, true);
    set_mask_bit(&mut state, VReg::V0, 3, false);
    exec(
        &mut state,
        ZvkbInstruction::VandnVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // Active: ~0x00 & 0xFF = 0xFF
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 0xFF);
    // Masked off: undisturbed
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E8), 0xAA);
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E8), 0xFF);
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E8), 0xAA);
}

#[test]
fn vror_vv_partial_mask_alternating() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 0xCC);
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0x80);
        write_elem(&mut state, VReg::V1, i, Vsew::E8, 1);
    }
    // Elements 1,3 active; elements 0,2 masked off
    set_mask_bit(&mut state, VReg::V0, 0, false);
    set_mask_bit(&mut state, VReg::V0, 1, true);
    set_mask_bit(&mut state, VReg::V0, 2, false);
    set_mask_bit(&mut state, VReg::V0, 3, true);
    exec(
        &mut state,
        ZvkbInstruction::VrorVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // Masked off: undisturbed
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 0xCC);
    // Active: rotate_right(0x80, 1) = 0x40
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E8), 0x40);
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E8), 0xCC);
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E8), 0x40);
}

// vm=true ignores v0 entirely
#[test]
fn vandn_vv_vm_true_ignores_v0() {
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    for i in 0..2 {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0xFF);
        write_elem(&mut state, VReg::V1, i, Vsew::E8, 0x00);
    }
    // v0 = all zeros: irrelevant for unmasked execution
    for i in 0..2 {
        set_mask_bit(&mut state, VReg::V0, i, false);
    }
    exec(
        &mut state,
        ZvkbInstruction::VandnVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // All elements written: ~0x00 & 0xFF = 0xFF
    for i in 0..2 {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0xFF, "elem {i}");
    }
}

// vandn.vv

#[test]
fn vandn_vv_basic_e8() {
    // ~0xAB = 0x54; 0x54 & 0x3F = 0x14
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0x3F);
        write_elem(&mut state, VReg::V1, i, Vsew::E8, 0xAB);
    }
    exec(
        &mut state,
        ZvkbInstruction::VandnVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4 {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0x14, "elem {i}");
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vandn_vv_all_ones_source_gives_zero() {
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xFF);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0x00);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 0xFF);
    write_elem(&mut state, VReg::V1, 1, Vsew::E8, 0xFF);
    exec(
        &mut state,
        ZvkbInstruction::VandnVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 0x00);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E8), 0x00);
}

#[test]
fn vandn_vv_all_zeros_source_gives_vs2() {
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xA5);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0x5A);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 0x00);
    write_elem(&mut state, VReg::V1, 1, Vsew::E8, 0x00);
    exec(
        &mut state,
        ZvkbInstruction::VandnVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 0xA5);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E8), 0x5A);
}

#[test]
fn vandn_vv_e32_masks_to_sew() {
    let mut state = setup(1, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 0xFFFF_FFFF);
    write_elem(&mut state, VReg::V1, 0, Vsew::E32, 0x0000_0000);
    exec(
        &mut state,
        ZvkbInstruction::VandnVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0xFFFF_FFFF);
}

// vandn.vx

#[test]
fn vandn_vx_basic() {
    // vs2=0xF0, rs1=0x3C -> ~0x3C = 0xC3 (E8); 0xC3 & 0xF0 = 0xC0
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0xF0);
    }
    state.regs.write(Reg::A0, 0x3C);
    exec(
        &mut state,
        ZvkbInstruction::VandnVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4 {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0xC0, "elem {i}");
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vandn_vx_scalar_zero_gives_vs2() {
    let mut state = setup(2, Vsew::E16, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E16, 0xBEEF);
    write_elem(&mut state, VReg::V2, 1, Vsew::E16, 0x1234);
    state.regs.write(Reg::A1, 0);
    exec(
        &mut state,
        ZvkbInstruction::VandnVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A1,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E16), 0xBEEF);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E16), 0x1234);
}

// vbrev8.v

#[test]
fn vbrev8_v_e8_reverses_all_bits() {
    // 0xB1 = 0b10110001 -> reversed = 0b10001101 = 0x8D
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0xB1);
    }
    exec(
        &mut state,
        ZvkbInstruction::Vbrev8V {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4 {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0x8D, "elem {i}");
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vbrev8_v_e16_reverses_bits_in_each_byte_independently() {
    // 0xABCD: byte 0 (low) = 0xCD -> 0xB3; byte 1 (high) = 0xAB -> 0xD5; result = 0xD5B3
    let mut state = setup(1, Vsew::E16, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E16, 0xABCD);
    exec(
        &mut state,
        ZvkbInstruction::Vbrev8V {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E16), 0xD5B3);
}

#[test]
fn vbrev8_v_e32_four_bytes_each_reversed() {
    // 0x01020304 -> 0x8040C020
    let mut state = setup(1, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 0x0102_0304);
    exec(
        &mut state,
        ZvkbInstruction::Vbrev8V {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0x8040_C020);
}

#[test]
fn vbrev8_v_idempotent_applied_twice() {
    let original = 0xDEAD_BEEF_1234_5678;
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E64, original);
    exec(
        &mut state,
        ZvkbInstruction::Vbrev8V {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    let intermediate = read_elem(&state, VReg::V4, 0, Vsew::E64);
    write_elem(&mut state, VReg::V2, 0, Vsew::E64, intermediate);
    exec(
        &mut state,
        ZvkbInstruction::Vbrev8V {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E64), original);
}

// vrev8.v

#[test]
fn vrev8_v_e8_is_noop() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0xAB);
    }
    exec(
        &mut state,
        ZvkbInstruction::Vrev8V {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4 {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0xAB, "elem {i}");
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vrev8_v_e16_swaps_two_bytes() {
    let mut state = setup(2, Vsew::E16, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E16, 0xABCD);
    write_elem(&mut state, VReg::V2, 1, Vsew::E16, 0x0100);
    exec(
        &mut state,
        ZvkbInstruction::Vrev8V {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E16), 0xCDAB);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E16), 0x0001);
}

#[test]
fn vrev8_v_e64_reverses_eight_bytes() {
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E64, 0x0102_0304_0506_0708);
    exec(
        &mut state,
        ZvkbInstruction::Vrev8V {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(
        read_elem(&state, VReg::V4, 0, Vsew::E64),
        0x0807_0605_0403_0201
    );
}

#[test]
fn vrev8_v_idempotent_applied_twice() {
    let original = 0xCAFE_BABE_DEAD_BEEF;
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E64, original);
    exec(
        &mut state,
        ZvkbInstruction::Vrev8V {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    let intermediate = read_elem(&state, VReg::V4, 0, Vsew::E64);
    write_elem(&mut state, VReg::V2, 0, Vsew::E64, intermediate);
    exec(
        &mut state,
        ZvkbInstruction::Vrev8V {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E64), original);
}

// vrol

#[test]
fn vrol_vv_e8_basic() {
    // rotate_left(0xB3, 3): hi = (0xB3 << 3) & 0xFF = 0x98; lo = 0xB3 >> 5 = 0x05; result = 0x9D
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0xB3);
        write_elem(&mut state, VReg::V1, i, Vsew::E8, 3);
    }
    exec(
        &mut state,
        ZvkbInstruction::VrolVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4 {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0x9D, "elem {i}");
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vrol_vv_shift_zero_is_identity() {
    let mut state = setup(1, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 0xDEAD_BEEF);
    write_elem(&mut state, VReg::V1, 0, Vsew::E32, 0);
    exec(
        &mut state,
        ZvkbInstruction::VrolVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0xDEAD_BEEF);
}

#[test]
fn vrol_vv_shift_equals_sew_is_identity() {
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xA5);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 8);
    exec(
        &mut state,
        ZvkbInstruction::VrolVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 0xA5);
}

#[test]
fn vrol_vv_e64_shift_1() {
    // rotate_left(0x8000000000000001, 1) = 0x0000000000000003
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E64, 0x8000_0000_0000_0001);
    write_elem(&mut state, VReg::V1, 0, Vsew::E64, 1);
    exec(
        &mut state,
        ZvkbInstruction::VrolVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(
        read_elem(&state, VReg::V4, 0, Vsew::E64),
        0x0000_0000_0000_0003
    );
}

#[test]
fn vrol_vx_basic() {
    // rotate_left(0x01, 4) at E8 = 0x10
    let mut state = setup(3, Vsew::E8, Vlmul::M1);
    for i in 0..3 {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0x01);
    }
    state.regs.write(Reg::A0, 4);
    exec(
        &mut state,
        ZvkbInstruction::VrolVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..3 {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0x10, "elem {i}");
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

// vror

#[test]
fn vror_vv_e8_basic() {
    // rotate_right(0xB3, 3): bottom 3 bits (011) go to top -> 0x76
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0xB3);
        write_elem(&mut state, VReg::V1, i, Vsew::E8, 3);
    }
    exec(
        &mut state,
        ZvkbInstruction::VrorVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4 {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0x76, "elem {i}");
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vror_vv_shift_zero_is_identity() {
    let mut state = setup(1, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 0xDEAD_BEEF);
    write_elem(&mut state, VReg::V1, 0, Vsew::E32, 0);
    exec(
        &mut state,
        ZvkbInstruction::VrorVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0xDEAD_BEEF);
}

#[test]
fn vror_vv_e64_shift_1() {
    // rotate_right(0x0000000000000003, 1) = 0x8000000000000001
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E64, 0x0000_0000_0000_0003);
    write_elem(&mut state, VReg::V1, 0, Vsew::E64, 1);
    exec(
        &mut state,
        ZvkbInstruction::VrorVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(
        read_elem(&state, VReg::V4, 0, Vsew::E64),
        0x8000_0000_0000_0001
    );
}

#[test]
fn vrol_and_vror_are_inverses() {
    let original = 0xA5A5_A5A5;
    let shift = 11;
    let mut state = setup(1, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, original);
    write_elem(&mut state, VReg::V1, 0, Vsew::E32, shift);
    exec(
        &mut state,
        ZvkbInstruction::VrolVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    let rotated_left = read_elem(&state, VReg::V4, 0, Vsew::E32);
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, rotated_left);
    exec(
        &mut state,
        ZvkbInstruction::VrorVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), original);
}

#[test]
fn vror_vx_basic() {
    // rotate_right(0x80, 7) at E8 = 0x01
    let mut state = setup(3, Vsew::E8, Vlmul::M1);
    for i in 0..3 {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0x80);
    }
    state.regs.write(Reg::A0, 7);
    exec(
        &mut state,
        ZvkbInstruction::VrorVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..3 {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0x01, "elem {i}");
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

// vror.vi

#[test]
fn vror_vi_uimm_0_is_identity() {
    // uimm=0 encodes bit[25]=0 -> vm=false (masked); set v0 all-active so elements are written
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    for i in 0..2 {
        set_mask_bit(&mut state, VReg::V0, i, true);
    }
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 0x1234_5678);
    write_elem(&mut state, VReg::V2, 1, Vsew::E32, 0xDEAD_BEEF);
    exec(
        &mut state,
        ZvkbInstruction::VrorVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            uimm: 0,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0x1234_5678);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E32), 0xDEAD_BEEF);
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vror_vi_uimm_4_e8() {
    // rotate_right(0xAB, 4): 0xAB >> 4 = 0x0A; (0xAB << 4) & 0xFF = 0xB0; result = 0xBA
    // uimm=4 -> vm=false (masked); set v0 all-active
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        set_mask_bit(&mut state, VReg::V0, i, true);
    }
    for i in 0..4 {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0xAB);
    }
    exec(
        &mut state,
        ZvkbInstruction::VrorVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            uimm: 4,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4 {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0xBA, "elem {i}");
    }
}

#[test]
fn vror_vi_e64_rotation_14() {
    // vror.vi v0, v16, 14 is the exact instruction from the certification test (0x53073057)
    // rotate_right(0xce30175474eadef7, 14):
    // lo = 0xce30175474eadef7 >> 14 = 0x000030b8c05d51bb (approx)
    // hi = (0xce30175474eadef7 << 50) & u64::MAX
    // Verify by checking that rotate_left(result, 14) == input
    let input = 0xce30_1754_74ea_def7_u64;
    let expected = input.rotate_right(14);
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V16, 0, Vsew::E64, input);
    exec(
        &mut state,
        ZvkbInstruction::VrorVi {
            vd: VReg::V0,
            vs2: VReg::V16,
            uimm: 14,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V0, 0, Vsew::E64), expected);
}

#[test]
fn vror_vi_uimm_31_at_e64() {
    // rotate_right(0x8000000000000001, 31):
    // = rotate_left(0x8000000000000001, 33)
    let input = 0x8000_0000_0000_0001_u64;
    let expected = input.rotate_right(31);
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E64, input);
    exec(
        &mut state,
        ZvkbInstruction::VrorVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            uimm: 31,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E64), expected);
}

#[test]
fn vror_vi_uimm_reduces_mod_sew() {
    // At E8, uimm=9 should give same result as uimm=1
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xF0);
    exec(
        &mut state,
        ZvkbInstruction::VrorVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            uimm: 9,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    let r9 = read_elem(&state, VReg::V4, 0, Vsew::E8);

    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xF0);
    exec(
        &mut state,
        ZvkbInstruction::VrorVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            uimm: 1,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    let r1 = read_elem(&state, VReg::V4, 0, Vsew::E8);

    assert_eq!(r9, r1);
}

// Cross-SEW correctness

#[test]
fn vror_vv_all_sew_widths() {
    for (vsew, input, shift, expected) in [
        (Vsew::E8, 0xFF, 4, 0xFF),
        (Vsew::E16, 0xFFFF, 8, 0xFFFF),
        (Vsew::E32, 0x1234_5678, 16, 0x5678_1234),
        (Vsew::E64, 0x0102_0304_0506_0708, 8, 0x0801_0203_0405_0607),
    ] {
        let mut state = setup(1, vsew, Vlmul::M1);
        write_elem(&mut state, VReg::V2, 0, vsew, input);
        write_elem(&mut state, VReg::V1, 0, vsew, shift);
        exec(
            &mut state,
            ZvkbInstruction::VrorVv {
                vm: true,
                vd: VReg::V4,
                vs2: VReg::V2,
                vs1: VReg::V1,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
        )
        .unwrap();
        assert_eq!(
            read_elem(&state, VReg::V4, 0, vsew),
            expected,
            "SEW={vsew:?}"
        );
    }
}

// Error paths

#[test]
fn error_vector_not_allowed() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    state.ext_state.set_vector_allowed(false);
    let result = exec(
        &mut state,
        ZvkbInstruction::VandnVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
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

#[test]
fn error_vill_vtype() {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    state.ext_state.set_vtype(None);
    state.ext_state.set_vl(0);
    let result = exec(
        &mut state,
        ZvkbInstruction::VrorVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
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

#[test]
fn error_misaligned_vd_lmul_m2() {
    let mut state = setup(4, Vsew::E32, Vlmul::M2);
    let result = exec(
        &mut state,
        ZvkbInstruction::VandnVv {
            vd: VReg::V3,
            vs2: VReg::V2,
            vs1: VReg::V4,
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

#[test]
fn error_misaligned_vs2_lmul_m4() {
    let mut state = setup(4, Vsew::E32, Vlmul::M4);
    let result = exec(
        &mut state,
        ZvkbInstruction::VrorVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V8,
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

#[test]
fn error_misaligned_vs1_lmul_m2() {
    let mut state = setup(4, Vsew::E32, Vlmul::M2);
    let result = exec(
        &mut state,
        ZvkbInstruction::VrolVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V3,
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

// vstart partial execution

#[test]
fn vror_vi_vstart_skips_earlier_elements() {
    // uimm=1 -> vm=false (masked); set v0 mask bits for all elements so active elements
    // (vstart..vl = 2..4) are written; elements 0,1 are skipped by vstart, not by masking
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        set_mask_bit(&mut state, VReg::V0, i, true);
    }
    for i in 0..4 {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0x80);
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 0xAA);
    }
    state.ext_state.set_vstart(2);
    exec(
        &mut state,
        ZvkbInstruction::VrorVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            uimm: 1,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // Elements 0,1 undisturbed (vstart=2)
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 0xAA);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E8), 0xAA);
    // Elements 2,3 processed: rotate_right(0x80, 1) = 0x40
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E8), 0x40);
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E8), 0x40);
    assert_eq!(state.ext_state.vstart(), 0);
}

// vl=0

#[test]
fn vandn_vv_vl_zero_no_writes() {
    let mut state = setup(0, Vsew::E32, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V4, i, Vsew::E32, 0xDEAD);
    }
    exec(
        &mut state,
        ZvkbInstruction::VandnVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4 {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E32),
            0xDEAD,
            "elem {i}"
        );
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
}
