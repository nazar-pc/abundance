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

fn setup(vl: u32, vsew: Vsew, vlmul: Vlmul) -> TestInterpreterState<ZvbcInstruction<Reg<u64>>> {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    let vtype = Vtype::from_raw::<Reg<u64>>(encode_vtype(vsew, vlmul)).unwrap();
    state.ext_state.set_vtype(Some(vtype));
    state.ext_state.set_vl(vl);
    state.ext_state.set_vstart(0);
    state
}

fn exec(
    state: &mut TestInterpreterState<ZvbcInstruction<Reg<u64>>>,
    instr: ZvbcInstruction<Reg<u64>>,
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
    state: &mut TestInterpreterState<ZvbcInstruction<Reg<u64>>>,
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
    state: &TestInterpreterState<ZvbcInstruction<Reg<u64>>>,
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
    state: &mut TestInterpreterState<ZvbcInstruction<Reg<u64>>>,
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

// Masking: vm=false with v0=all-zeros -> every element is undisturbed

#[test]
fn vclmul_vv_masked_v0_zeroes_undisturbed() {
    let mut state = setup(4, Vsew::E16, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V4, i, Vsew::E16, 0xBEEF);
        write_elem(&mut state, VReg::V2, i, Vsew::E16, 0xFF00);
        write_elem(&mut state, VReg::V1, i, Vsew::E16, 0x00FF);
    }
    for i in 0..4 {
        set_mask_bit(&mut state, VReg::V0, i, false);
    }
    exec(
        &mut state,
        ZvbcInstruction::VclmulVv {
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
fn vclmul_vx_masked_v0_zeroes_undisturbed() {
    let mut state = setup(4, Vsew::E16, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V4, i, Vsew::E16, 0xDEAD);
        write_elem(&mut state, VReg::V2, i, Vsew::E16, 0xFF00);
    }
    state.regs.write(Reg::A0, 0x00FF);
    for i in 0..4 {
        set_mask_bit(&mut state, VReg::V0, i, false);
    }
    exec(
        &mut state,
        ZvbcInstruction::VclmulVx {
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
fn vclmulh_vv_masked_v0_zeroes_undisturbed() {
    let mut state = setup(4, Vsew::E16, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V4, i, Vsew::E16, 0xCAFE);
        write_elem(&mut state, VReg::V2, i, Vsew::E16, 0xFFFF);
        write_elem(&mut state, VReg::V1, i, Vsew::E16, 0x8000);
    }
    for i in 0..4 {
        set_mask_bit(&mut state, VReg::V0, i, false);
    }
    exec(
        &mut state,
        ZvbcInstruction::VclmulhVv {
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
            0xCAFE,
            "elem {i}"
        );
    }
}

#[test]
fn vclmulh_vx_masked_v0_zeroes_undisturbed() {
    let mut state = setup(4, Vsew::E16, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V4, i, Vsew::E16, 0x1234);
        write_elem(&mut state, VReg::V2, i, Vsew::E16, 0xFFFF);
    }
    state.regs.write(Reg::A1, 0x8000);
    for i in 0..4 {
        set_mask_bit(&mut state, VReg::V0, i, false);
    }
    exec(
        &mut state,
        ZvbcInstruction::VclmulhVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A1,
            vm: false,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4 {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E16),
            0x1234,
            "elem {i}"
        );
    }
}

// Partial masking: alternating active/inactive elements

#[test]
fn vclmul_vv_partial_mask_alternating() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 0xAA);
        // clmul(0x09, 0x09) = 0x41 at E8 (x^3+1 squared = x^6+1)
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0x09);
        write_elem(&mut state, VReg::V1, i, Vsew::E8, 0x09);
    }
    // Elements 0,2 active; elements 1,3 masked off
    set_mask_bit(&mut state, VReg::V0, 0, true);
    set_mask_bit(&mut state, VReg::V0, 1, false);
    set_mask_bit(&mut state, VReg::V0, 2, true);
    set_mask_bit(&mut state, VReg::V0, 3, false);
    exec(
        &mut state,
        ZvbcInstruction::VclmulVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 0x41);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E8), 0xAA);
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E8), 0x41);
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E8), 0xAA);
}

// vm=true ignores v0 entirely

#[test]
fn vclmul_vv_vm_true_ignores_v0() {
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    for i in 0..2 {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0x09);
        write_elem(&mut state, VReg::V1, i, Vsew::E8, 0x09);
        // v0 = all zeros: irrelevant for unmasked execution
        set_mask_bit(&mut state, VReg::V0, i as u32, false);
    }
    exec(
        &mut state,
        ZvbcInstruction::VclmulVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..2 {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0x41, "elem {i}");
    }
}

// vclmul.vv: mathematical correctness

// clmul(x^3+1, x^3+1) = x^6+1 = 0x41 in GF(2)[x] with 8-bit elements.
// Verifies the squaring identity and the basic product at E8.
#[test]
fn vclmul_vv_e8_squaring_polynomial() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0x09);
        write_elem(&mut state, VReg::V1, i, Vsew::E8, 0x09);
    }
    exec(
        &mut state,
        ZvbcInstruction::VclmulVv {
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
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0x41, "elem {i}");
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

// Multiplying by the identity polynomial (1 = 0x01) must return vs2 unchanged.
#[test]
fn vclmul_vv_e8_multiply_by_one_is_identity() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0xB7);
        write_elem(&mut state, VReg::V1, i, Vsew::E8, 0x01);
    }
    exec(
        &mut state,
        ZvbcInstruction::VclmulVv {
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
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0xB7, "elem {i}");
    }
}

// Multiplying any element by zero gives zero.
#[test]
fn vclmul_vv_e16_multiply_by_zero_gives_zero() {
    let mut state = setup(4, Vsew::E16, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V2, i, Vsew::E16, 0xABCD);
        write_elem(&mut state, VReg::V1, i, Vsew::E16, 0x0000);
    }
    exec(
        &mut state,
        ZvbcInstruction::VclmulVv {
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
            read_elem(&state, VReg::V4, i, Vsew::E16),
            0x0000,
            "elem {i}"
        );
    }
}

// Carry-less multiplication is commutative: clmul(a, b) = clmul(b, a).
#[test]
fn vclmul_vv_e32_commutative() {
    let a = 0x1234_5678;
    let b = 0xABCD_EF01;
    let mut state_ab = setup(1, Vsew::E32, Vlmul::M1);
    write_elem(&mut state_ab, VReg::V2, 0, Vsew::E32, a);
    write_elem(&mut state_ab, VReg::V1, 0, Vsew::E32, b);
    exec(
        &mut state_ab,
        ZvbcInstruction::VclmulVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    let result_ab = read_elem(&state_ab, VReg::V4, 0, Vsew::E32);
    let mut state_ba = setup(1, Vsew::E32, Vlmul::M1);
    write_elem(&mut state_ba, VReg::V2, 0, Vsew::E32, b);
    write_elem(&mut state_ba, VReg::V1, 0, Vsew::E32, a);
    exec(
        &mut state_ba,
        ZvbcInstruction::VclmulVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    let result_ba = read_elem(&state_ba, VReg::V4, 0, Vsew::E32);
    assert_eq!(result_ab, result_ba);
}

// At E64, x^63 * x^63 = x^126; this lies entirely in the high half so the low half is 0.
#[test]
fn vclmul_vv_e64_high_bit_product_is_zero_in_low_half() {
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E64, 0x8000_0000_0000_0000);
    write_elem(&mut state, VReg::V1, 0, Vsew::E64, 0x8000_0000_0000_0000);
    exec(
        &mut state,
        ZvbcInstruction::VclmulVv {
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
        0x0000_0000_0000_0000
    );
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

// vclmul.vx

// clmul(0x09, 0x09) = 0x41 via scalar source gives the same result as VV form.
#[test]
fn vclmul_vx_basic_e8() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0x09);
    }
    state.regs.write(Reg::A0, 0x09);
    exec(
        &mut state,
        ZvbcInstruction::VclmulVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4 {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0x41, "elem {i}");
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

// VV and VX produce identical results when the scalar matches the vector element value.
#[test]
fn vclmul_vv_and_vx_agree_on_same_operand() {
    let a = 0x1234;
    let b = 0x5678;
    let mut state_vv = setup(1, Vsew::E16, Vlmul::M1);
    write_elem(&mut state_vv, VReg::V2, 0, Vsew::E16, a);
    write_elem(&mut state_vv, VReg::V1, 0, Vsew::E16, b);
    exec(
        &mut state_vv,
        ZvbcInstruction::VclmulVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    let vv_result = read_elem(&state_vv, VReg::V4, 0, Vsew::E16);
    let mut state_vx = setup(1, Vsew::E16, Vlmul::M1);
    write_elem(&mut state_vx, VReg::V2, 0, Vsew::E16, a);
    state_vx.regs.write(Reg::A0, b);
    exec(
        &mut state_vx,
        ZvbcInstruction::VclmulVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    let vx_result = read_elem(&state_vx, VReg::V4, 0, Vsew::E16);
    assert_eq!(vv_result, vx_result);
}

// Bits above SEW in the scalar register are discarded; only the lower SEW bits participate.
#[test]
fn vclmul_vx_upper_scalar_bits_ignored() {
    let mut state_clean = setup(1, Vsew::E8, Vlmul::M1);
    write_elem(&mut state_clean, VReg::V2, 0, Vsew::E8, 0x09);
    state_clean.regs.write(Reg::A0, 0x09);
    exec(
        &mut state_clean,
        ZvbcInstruction::VclmulVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    let result_clean = read_elem(&state_clean, VReg::V4, 0, Vsew::E8);
    // Same operation but scalar has garbage in bits 63:8; lower 8 bits are identical
    let mut state_dirty = setup(1, Vsew::E8, Vlmul::M1);
    write_elem(&mut state_dirty, VReg::V2, 0, Vsew::E8, 0x09);
    state_dirty.regs.write(Reg::A0, 0xFFFF_FFFF_FFFF_0009);
    exec(
        &mut state_dirty,
        ZvbcInstruction::VclmulVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    let result_dirty = read_elem(&state_dirty, VReg::V4, 0, Vsew::E8);
    assert_eq!(result_clean, result_dirty);
}

// vclmulh.vv: mathematical correctness

// clmulh(0xFF, 0x80) = upper 8 bits of (0xFF * x^7) = 0x7F.
// Full product: 0xFF << 7 = 0x7F80; lower byte = 0x80, upper byte = 0x7F.
#[test]
fn vclmulh_vv_e8_nontrivial_upper_bits() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0xFF);
        write_elem(&mut state, VReg::V1, i, Vsew::E8, 0x80);
    }
    exec(
        &mut state,
        ZvbcInstruction::VclmulhVv {
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
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0x7F, "elem {i}");
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

// When the product fits in SEW bits, the upper half is zero.
// clmul(0x09, 0x09) = 0x41 (7 bits, fits in E8); upper 8 bits = 0.
#[test]
fn vclmulh_vv_e8_product_fits_in_sew_gives_zero() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0x09);
        write_elem(&mut state, VReg::V1, i, Vsew::E8, 0x09);
    }
    exec(
        &mut state,
        ZvbcInstruction::VclmulhVv {
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
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0x00, "elem {i}");
    }
}

// clmulh(0xFFFF, 0x8000) = upper 16 bits of (0xFFFF << 15) = 0x7FFF.
// Full product: 0x7FFF8000; lower 16 bits = 0x8000, upper 16 bits = 0x7FFF.
#[test]
fn vclmulh_vv_e16_nontrivial_upper_bits() {
    let mut state = setup(2, Vsew::E16, Vlmul::M1);
    for i in 0..2 {
        write_elem(&mut state, VReg::V2, i, Vsew::E16, 0xFFFF);
        write_elem(&mut state, VReg::V1, i, Vsew::E16, 0x8000);
    }
    exec(
        &mut state,
        ZvbcInstruction::VclmulhVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..2 {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E16),
            0x7FFF,
            "elem {i}"
        );
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

// At E64, x^63 * x^63 = x^126; this lies entirely in the upper half at bit 126 = bit 62 of
// the upper 64-bit word, so clmulh = 0x4000_0000_0000_0000.
#[test]
fn vclmulh_vv_e64_high_bit_product_in_upper_half() {
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E64, 0x8000_0000_0000_0000);
    write_elem(&mut state, VReg::V1, 0, Vsew::E64, 0x8000_0000_0000_0000);
    exec(
        &mut state,
        ZvbcInstruction::VclmulhVv {
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
        0x4000_0000_0000_0000
    );
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

// vclmul and vclmulh together reconstruct the full 2*SEW product.
// At E8: clmul(0xFF, 0x80) = 0x80, clmulh(0xFF, 0x80) = 0x7F -> product = 0x7F80.
#[test]
fn vclmul_and_vclmulh_reconstruct_full_product_e8() {
    let mut state_lo = setup(1, Vsew::E8, Vlmul::M1);
    write_elem(&mut state_lo, VReg::V2, 0, Vsew::E8, 0xFF);
    write_elem(&mut state_lo, VReg::V1, 0, Vsew::E8, 0x80);
    exec(
        &mut state_lo,
        ZvbcInstruction::VclmulVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    let lo = read_elem(&state_lo, VReg::V4, 0, Vsew::E8);
    let mut state_hi = setup(1, Vsew::E8, Vlmul::M1);
    write_elem(&mut state_hi, VReg::V2, 0, Vsew::E8, 0xFF);
    write_elem(&mut state_hi, VReg::V1, 0, Vsew::E8, 0x80);
    exec(
        &mut state_hi,
        ZvbcInstruction::VclmulhVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    let hi = read_elem(&state_hi, VReg::V4, 0, Vsew::E8);
    assert_eq!(lo, 0x80);
    assert_eq!(hi, 0x7F);
    // The 16-bit product (hi:lo) must equal 0x7F80
    assert_eq!((hi << 8) | lo, 0x7F80);
}

// vclmulh.vx

// clmulh(0xFF, 0x80) = 0x7F via scalar source; same as VV form.
#[test]
fn vclmulh_vx_basic_e8() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0xFF);
    }
    state.regs.write(Reg::A0, 0x80);
    exec(
        &mut state,
        ZvbcInstruction::VclmulhVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4 {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0x7F, "elem {i}");
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

// VV and VX produce identical results for vclmulh when the scalar matches.
#[test]
fn vclmulh_vv_and_vx_agree_on_same_operand() {
    let a = 0xFFFF;
    let b = 0x8000;
    let mut state_vv = setup(1, Vsew::E16, Vlmul::M1);
    write_elem(&mut state_vv, VReg::V2, 0, Vsew::E16, a);
    write_elem(&mut state_vv, VReg::V1, 0, Vsew::E16, b);
    exec(
        &mut state_vv,
        ZvbcInstruction::VclmulhVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    let vv_result = read_elem(&state_vv, VReg::V4, 0, Vsew::E16);
    let mut state_vx = setup(1, Vsew::E16, Vlmul::M1);
    write_elem(&mut state_vx, VReg::V2, 0, Vsew::E16, a);
    state_vx.regs.write(Reg::A0, b);
    exec(
        &mut state_vx,
        ZvbcInstruction::VclmulhVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    let vx_result = read_elem(&state_vx, VReg::V4, 0, Vsew::E16);
    assert_eq!(vv_result, vx_result);
}

// vstart partial execution

#[test]
fn vclmul_vv_vstart_skips_earlier_elements() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0x09);
        write_elem(&mut state, VReg::V1, i, Vsew::E8, 0x09);
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 0xAA);
    }
    state.ext_state.set_vstart(2);
    exec(
        &mut state,
        ZvbcInstruction::VclmulVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // Elements 0,1: undisturbed (below vstart=2)
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 0xAA);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E8), 0xAA);
    // Elements 2,3: clmul(0x09, 0x09) = 0x41
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E8), 0x41);
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E8), 0x41);
    assert_eq!(state.ext_state.vstart(), 0);
}

// vl=0

#[test]
fn vclmul_vv_vl_zero_no_writes() {
    let mut state = setup(0, Vsew::E32, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V4, i, Vsew::E32, 0xDEAD_BEEF);
    }
    exec(
        &mut state,
        ZvbcInstruction::VclmulVv {
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
            0xDEAD_BEEF,
            "elem {i}"
        );
    }
    // mark_vs_dirty is called unconditionally, even for vl=0
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
}

#[test]
fn vclmulh_vv_vl_zero_no_writes() {
    let mut state = setup(0, Vsew::E32, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V4, i, Vsew::E32, 0xCAFE_BABE);
    }
    exec(
        &mut state,
        ZvbcInstruction::VclmulhVv {
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
            0xCAFE_BABE,
            "elem {i}"
        );
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
}

// Error paths

#[test]
fn error_vector_not_allowed() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    state.ext_state.set_vector_allowed(false);
    let result = exec(
        &mut state,
        ZvbcInstruction::VclmulVv {
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
        ZvbcInstruction::VclmulhVv {
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
        ZvbcInstruction::VclmulVv {
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
        ZvbcInstruction::VclmulhVv {
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
        ZvbcInstruction::VclmulVv {
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

#[test]
fn error_vclmulh_misaligned_vd_lmul_m2() {
    let mut state = setup(4, Vsew::E32, Vlmul::M2);
    let result = exec(
        &mut state,
        ZvbcInstruction::VclmulhVv {
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
