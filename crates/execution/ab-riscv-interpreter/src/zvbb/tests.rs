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

fn setup(vl: u32, vsew: Vsew, vlmul: Vlmul) -> TestInterpreterState<ZvbbInstruction<Reg<u64>>> {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    let vtype = Vtype::from_raw::<Reg<u64>>(encode_vtype(vsew, vlmul)).unwrap();
    state.ext_state.set_vtype(Some(vtype));
    state.ext_state.set_vl(vl);
    state.ext_state.set_vstart(0);
    state
}

fn exec(
    state: &mut TestInterpreterState<ZvbbInstruction<Reg<u64>>>,
    instr: ZvbbInstruction<Reg<u64>>,
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
    state: &mut TestInterpreterState<ZvbbInstruction<Reg<u64>>>,
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
    state: &TestInterpreterState<ZvbbInstruction<Reg<u64>>>,
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
    state: &mut TestInterpreterState<ZvbbInstruction<Reg<u64>>>,
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

// Masking: v0=all-zeros with vm=false -> every element undisturbed

#[test]
fn vbrev_v_masked_v0_zeroes_undisturbed() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 0xAA);
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0xFF);
        set_mask_bit(&mut state, VReg::V0, i as u32, false);
    }
    exec(
        &mut state,
        ZvbbInstruction::VbrevV {
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
fn vclz_v_masked_v0_zeroes_undisturbed() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 0xBB);
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0xFF);
        set_mask_bit(&mut state, VReg::V0, i as u32, false);
    }
    exec(
        &mut state,
        ZvbbInstruction::VclzV {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4 {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0xBB, "elem {i}");
    }
}

#[test]
fn vctz_v_masked_v0_zeroes_undisturbed() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 0xCC);
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0xFF);
        set_mask_bit(&mut state, VReg::V0, i as u32, false);
    }
    exec(
        &mut state,
        ZvbbInstruction::VctzV {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4 {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0xCC, "elem {i}");
    }
}

#[test]
fn vcpop_v_masked_v0_zeroes_undisturbed() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 0xDD);
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0xFF);
        set_mask_bit(&mut state, VReg::V0, i as u32, false);
    }
    exec(
        &mut state,
        ZvbbInstruction::VcpopV {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4 {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0xDD, "elem {i}");
    }
}

#[test]
fn vwsll_vv_masked_v0_zeroes_undisturbed() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        // vd is at E16 width; sentinel in both halves of each 2-byte destination slot
        write_elem(&mut state, VReg::V4, i, Vsew::E16, 0xBEEF);
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0xFF);
        write_elem(&mut state, VReg::V1, i, Vsew::E8, 4);
        set_mask_bit(&mut state, VReg::V0, i as u32, false);
    }
    exec(
        &mut state,
        ZvbbInstruction::VwsllVv {
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
            "elem {i}: masked destination must be undisturbed"
        );
    }
}

// Partial masking: alternating active/inactive elements

#[test]
fn vbrev_v_partial_mask_alternating() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 0xAA);
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0xB1);
    }
    // Elements 0,2 active; elements 1,3 masked off
    set_mask_bit(&mut state, VReg::V0, 0, true);
    set_mask_bit(&mut state, VReg::V0, 1, false);
    set_mask_bit(&mut state, VReg::V0, 2, true);
    set_mask_bit(&mut state, VReg::V0, 3, false);
    exec(
        &mut state,
        ZvbbInstruction::VbrevV {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // Active: 0xB1 = 0b10110001 -> reversed = 0b10001101 = 0x8D
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 0x8D);
    // Masked off: undisturbed
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E8), 0xAA);
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E8), 0x8D);
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E8), 0xAA);
}

#[test]
fn vclz_v_partial_mask_alternating() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 0xFF);
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0x01);
    }
    set_mask_bit(&mut state, VReg::V0, 0, true);
    set_mask_bit(&mut state, VReg::V0, 1, false);
    set_mask_bit(&mut state, VReg::V0, 2, true);
    set_mask_bit(&mut state, VReg::V0, 3, false);
    exec(
        &mut state,
        ZvbbInstruction::VclzV {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // Active: clz(0x01) at E8 = 7
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 7);
    // Masked off: undisturbed
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E8), 0xFF);
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E8), 7);
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E8), 0xFF);
}

// vm=true ignores v0 entirely

#[test]
fn vbrev_v_vm_true_ignores_v0() {
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    for i in 0..2 {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0x80);
        set_mask_bit(&mut state, VReg::V0, i as u32, false);
    }
    exec(
        &mut state,
        ZvbbInstruction::VbrevV {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 0x80 = 0b10000000 -> reversed = 0b00000001 = 0x01; v0 irrelevant
    for i in 0..2 {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0x01, "elem {i}");
    }
}

// vbrev correctness

#[test]
fn vbrev_v_e8_reverses_all_8_bits() {
    // 0xB1 = 0b10110001 -> reversed = 0b10001101 = 0x8D
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0xB1);
    }
    exec(
        &mut state,
        ZvbbInstruction::VbrevV {
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
fn vbrev_v_e16_differs_from_vbrev8() {
    // For E16 input 0xABCD:
    // vbrev8: byte 0 (0xCD=11001101->10110011=0xB3), byte 1 (0xAB=10101011->11010101=0xD5) ->
    // 0xD5B3 vbrev:  all 16 bits reversed: 1010101111001101 -> 1011001111010101 = 0xB3D5
    let mut state = setup(1, Vsew::E16, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E16, 0xABCD);
    exec(
        &mut state,
        ZvbbInstruction::VbrevV {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E16), 0xB3D5);
}

#[test]
fn vbrev_v_e32_reverses_32_bits() {
    // 0x12345678 bit-reversed = 0x1E6A2C48
    let mut state = setup(1, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 0x1234_5678);
    exec(
        &mut state,
        ZvbbInstruction::VbrevV {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0x1E6A_2C48);
}

#[test]
fn vbrev_v_e64_reverses_64_bits() {
    let input = 0x0102_0304_0506_0708_u64;
    let expected = input.reverse_bits();
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E64, input);
    exec(
        &mut state,
        ZvbbInstruction::VbrevV {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E64), expected);
}

#[test]
fn vbrev_v_idempotent_applied_twice() {
    let original = 0xDEAD_BEEF_1234_5678;
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E64, original);
    exec(
        &mut state,
        ZvbbInstruction::VbrevV {
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
        ZvbbInstruction::VbrevV {
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

#[test]
fn vbrev_v_zero_is_zero() {
    let mut state = setup(1, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 0);
    exec(
        &mut state,
        ZvbbInstruction::VbrevV {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0);
}

// vclz correctness

#[test]
fn vclz_v_e8_zero_gives_sew() {
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0x00);
    exec(
        &mut state,
        ZvbbInstruction::VclzV {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 8);
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vclz_v_e8_all_ones_gives_zero() {
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xFF);
    exec(
        &mut state,
        ZvbbInstruction::VclzV {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 0);
}

#[test]
fn vclz_v_all_sew_widths() {
    // (sew, input, expected_clz)
    for (vsew, input, expected) in [
        (Vsew::E8, 0x40, 1),
        (Vsew::E8, 0x01, 7),
        (Vsew::E16, 0x0000, 16),
        (Vsew::E16, 0x0001, 15),
        (Vsew::E16, 0x8000, 0),
        (Vsew::E32, 0x0000_0001, 31),
        (Vsew::E32, 0x8000_0000, 0),
        (Vsew::E64, 0x8000_0000_0000_0000, 0),
        (Vsew::E64, 0x0000_0000_0000_0001, 63),
    ] {
        let mut state = setup(1, vsew, Vlmul::M1);
        write_elem(&mut state, VReg::V2, 0, vsew, input);
        exec(
            &mut state,
            ZvbbInstruction::VclzV {
                vd: VReg::V4,
                vs2: VReg::V2,
                vm: true,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
        )
        .unwrap();
        assert_eq!(
            read_elem(&state, VReg::V4, 0, vsew),
            expected,
            "SEW={vsew:?} input=0x{input:X}"
        );
    }
}

// vctz correctness

#[test]
fn vctz_v_e8_zero_gives_sew() {
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0x00);
    exec(
        &mut state,
        ZvbbInstruction::VctzV {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 8);
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vctz_v_e8_all_ones_gives_zero() {
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xFF);
    exec(
        &mut state,
        ZvbbInstruction::VctzV {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 0);
}

#[test]
fn vctz_v_all_sew_widths() {
    for (vsew, input, expected) in [
        (Vsew::E8, 0x01, 0),
        (Vsew::E8, 0x80, 7),
        (Vsew::E8, 0x02, 1),
        (Vsew::E16, 0x0000, 16),
        (Vsew::E16, 0x0002, 1),
        (Vsew::E32, 0x8000_0000, 31),
        (Vsew::E64, 0x0000_0000_0000_0002, 1),
        (Vsew::E64, 0x8000_0000_0000_0000, 63),
    ] {
        let mut state = setup(1, vsew, Vlmul::M1);
        write_elem(&mut state, VReg::V2, 0, vsew, input);
        exec(
            &mut state,
            ZvbbInstruction::VctzV {
                vd: VReg::V4,
                vs2: VReg::V2,
                vm: true,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
        )
        .unwrap();
        assert_eq!(
            read_elem(&state, VReg::V4, 0, vsew),
            expected,
            "SEW={vsew:?} input=0x{input:X}"
        );
    }
}

// clz and ctz of the same value must sum to SEW only when input is a single-bit power of two
#[test]
fn vclz_vctz_single_bit_sum_equals_sew_minus_one() {
    // For 0x08 at E8 (bit 3 set): clz=4, ctz=3, clz+ctz=7=SEW-1
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0x08);
    exec(
        &mut state,
        ZvbbInstruction::VclzV {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    let clz = read_elem(&state, VReg::V4, 0, Vsew::E8);
    exec(
        &mut state,
        ZvbbInstruction::VctzV {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    let ctz = read_elem(&state, VReg::V4, 0, Vsew::E8);
    assert_eq!(clz, 4);
    assert_eq!(ctz, 3);
    assert_eq!(clz + ctz, 7);
}

// vcpop correctness

#[test]
fn vcpop_v_e8_zero_gives_zero() {
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0x00);
    exec(
        &mut state,
        ZvbbInstruction::VcpopV {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 0);
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vcpop_v_e8_all_ones_gives_sew() {
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xFF);
    exec(
        &mut state,
        ZvbbInstruction::VcpopV {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 8);
}

#[test]
fn vcpop_v_all_sew_widths() {
    for (vsew, input, expected) in [
        (Vsew::E8, 0xAA, 4),
        (Vsew::E8, 0x55, 4),
        (Vsew::E16, 0xFFFF, 16),
        (Vsew::E32, 0xDEAD_BEEF, 24),
        (Vsew::E64, u64::MAX, 64),
        (Vsew::E64, 0, 0),
    ] {
        let mut state = setup(1, vsew, Vlmul::M1);
        write_elem(&mut state, VReg::V2, 0, vsew, input);
        exec(
            &mut state,
            ZvbbInstruction::VcpopV {
                vd: VReg::V4,
                vs2: VReg::V2,
                vm: true,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
        )
        .unwrap();
        assert_eq!(
            read_elem(&state, VReg::V4, 0, vsew),
            expected,
            "SEW={vsew:?} input=0x{input:X}"
        );
    }
}

// vcpop: complement has count = SEW - original
#[test]
fn vcpop_v_complement_sums_to_sew() {
    let mut state = setup(1, Vsew::E16, Vlmul::M1);
    let val = 0x0F0F;
    write_elem(&mut state, VReg::V2, 0, Vsew::E16, val);
    exec(
        &mut state,
        ZvbbInstruction::VcpopV {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    let count = read_elem(&state, VReg::V4, 0, Vsew::E16);
    write_elem(&mut state, VReg::V2, 0, Vsew::E16, !val & 0xFFFF);
    exec(
        &mut state,
        ZvbbInstruction::VcpopV {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    let complement_count = read_elem(&state, VReg::V4, 0, Vsew::E16);
    assert_eq!(count + complement_count, 16);
}

// vwsll correctness

#[test]
fn vwsll_vv_e8_to_e16_basic() {
    // zero_extend(0x01) << 4 = 0x0010; result is E16
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0x01);
        write_elem(&mut state, VReg::V1, i, Vsew::E8, 4);
    }
    exec(
        &mut state,
        ZvbbInstruction::VwsllVv {
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
            0x0010,
            "elem {i}"
        );
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vwsll_vv_e8_to_e16_shift_zero_is_widening_identity() {
    // Shift by 0: result equals zero-extended source
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xAB);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0x00);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 0);
    write_elem(&mut state, VReg::V1, 1, Vsew::E8, 0);
    exec(
        &mut state,
        ZvbbInstruction::VwsllVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E16), 0x00AB);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E16), 0x0000);
}

#[test]
fn vwsll_vv_e16_to_e32_basic() {
    // zero_extend(0x0001u16) << 8 = 0x0000_0100
    let mut state = setup(2, Vsew::E16, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E16, 0x0001);
    write_elem(&mut state, VReg::V1, 0, Vsew::E16, 8);
    exec(
        &mut state,
        ZvbbInstruction::VwsllVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0x0000_0100);
}

#[test]
fn vwsll_vv_e32_to_e64_basic() {
    // zero_extend(0x0000_0001u32) << 32 = 0x0000_0001_0000_0000
    let mut state = setup(1, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 0x0000_0001);
    write_elem(&mut state, VReg::V1, 0, Vsew::E32, 32);
    exec(
        &mut state,
        ZvbbInstruction::VwsllVv {
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
        0x0000_0001_0000_0000
    );
}

#[test]
fn vwsll_vv_shift_amount_reduces_mod_double_sew() {
    // At E8 -> E16: shift by 17 is the same as shift by 1 (17 % 16 = 1)
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0x01);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 17);
    exec(
        &mut state,
        ZvbbInstruction::VwsllVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    let r17 = read_elem(&state, VReg::V4, 0, Vsew::E16);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 1);
    exec(
        &mut state,
        ZvbbInstruction::VwsllVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    let r1 = read_elem(&state, VReg::V4, 0, Vsew::E16);
    assert_eq!(r17, r1);
}

#[test]
fn vwsll_vv_shift_by_double_sew_is_identity() {
    // At E8 -> E16: shift by 16 (= double_sew) is 0 mod 16, same as shift by 0
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xAB);
    // Encode 16 in E8: value 16 overflows E8 as a stored element, but shift logic uses mod.
    // vs1 holds shift count as SEW-wide (E8) element; 16 % 256 = 16 in u8, stored as 16.
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 16);
    exec(
        &mut state,
        ZvbbInstruction::VwsllVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 0xAB << (16 % 16) = 0xAB << 0 = 0x00AB
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E16), 0x00AB);
}

// vwsll.vx

#[test]
fn vwsll_vx_basic() {
    // zero_extend(0x03u8) << 4 = 0x0030; rs1 provides the shift count
    let mut state = setup(3, Vsew::E8, Vlmul::M1);
    for i in 0..3 {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0x03);
    }
    state.regs.write(Reg::A0, 4);
    exec(
        &mut state,
        ZvbbInstruction::VwsllVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..3 {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E16),
            0x0030,
            "elem {i}"
        );
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vwsll_vx_scalar_zero_is_widening_identity() {
    let mut state = setup(2, Vsew::E16, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E16, 0xBEEF);
    write_elem(&mut state, VReg::V2, 1, Vsew::E16, 0x1234);
    state.regs.write(Reg::A1, 0);
    exec(
        &mut state,
        ZvbbInstruction::VwsllVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A1,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0x0000_BEEF);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E32), 0x0000_1234);
}

// vwsll.vi

#[test]
fn vwsll_vi_uimm_zero_is_widening_identity() {
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xAB);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0xCD);
    exec(
        &mut state,
        ZvbbInstruction::VwsllVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            uimm: 0,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E16), 0x00AB);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E16), 0x00CD);
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vwsll_vi_uimm_8_e8_to_e16() {
    // zero_extend(0x01) << 8 = 0x0100
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    for i in 0..2 {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0x01);
    }
    exec(
        &mut state,
        ZvbbInstruction::VwsllVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            uimm: 8,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..2 {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E16),
            0x0100,
            "elem {i}"
        );
    }
}

#[test]
fn vwsll_vi_uimm_31_e16_to_e32() {
    // zero_extend(0x0001) << 31 = 0x8000_0000
    let mut state = setup(1, Vsew::E16, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E16, 0x0001);
    exec(
        &mut state,
        ZvbbInstruction::VwsllVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            uimm: 31,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0x8000_0000);
}

// vwsll masking with v0

#[test]
fn vwsll_vi_masked_v0_all_active() {
    // vm=false, v0=all-ones: every element active, all written
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        set_mask_bit(&mut state, VReg::V0, i as u32, true);
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0x01);
    }
    exec(
        &mut state,
        ZvbbInstruction::VwsllVi {
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
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E16),
            0x0010,
            "elem {i}"
        );
    }
}

// vl=0

#[test]
fn vbrev_v_vl_zero_no_writes() {
    let mut state = setup(0, Vsew::E32, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V4, i, Vsew::E32, 0xDEAD_BEEF);
    }
    exec(
        &mut state,
        ZvbbInstruction::VbrevV {
            vd: VReg::V4,
            vs2: VReg::V2,
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
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
}

#[test]
fn vwsll_vv_vl_zero_no_writes() {
    let mut state = setup(0, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V4, i, Vsew::E16, 0xCAFE);
    }
    exec(
        &mut state,
        ZvbbInstruction::VwsllVv {
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
            0xCAFE,
            "elem {i}"
        );
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
}

// vstart partial execution

#[test]
fn vclz_v_vstart_skips_earlier_elements() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0x01);
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 0xAA);
    }
    state.ext_state.set_vstart(2);
    exec(
        &mut state,
        ZvbbInstruction::VclzV {
            vd: VReg::V4,
            vs2: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // Elements 0,1 undisturbed (vstart=2)
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 0xAA);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E8), 0xAA);
    // Elements 2,3 processed: clz(0x01) at E8 = 7
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E8), 7);
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E8), 7);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vwsll_vv_vstart_skips_earlier_elements() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4 {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0x01);
        write_elem(&mut state, VReg::V1, i, Vsew::E8, 4);
        write_elem(&mut state, VReg::V4, i, Vsew::E16, 0xBEEF);
    }
    state.ext_state.set_vstart(2);
    exec(
        &mut state,
        ZvbbInstruction::VwsllVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // Elements 0,1 undisturbed (vstart=2)
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E16), 0xBEEF);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E16), 0xBEEF);
    // Elements 2,3 processed: 0x01 << 4 = 0x0010
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E16), 0x0010);
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E16), 0x0010);
    assert_eq!(state.ext_state.vstart(), 0);
}

// Error paths

#[test]
fn error_vector_not_allowed() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    state.ext_state.set_vector_allowed(false);
    let result = exec(
        &mut state,
        ZvbbInstruction::VbrevV {
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

#[test]
fn error_vill_vtype() {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    state.ext_state.set_vtype(None);
    state.ext_state.set_vl(0);
    let result = exec(
        &mut state,
        ZvbbInstruction::VclzV {
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

#[test]
fn error_misaligned_vd_lmul_m2() {
    // M2: group_regs=2; vd must be even; V3 is odd
    let mut state = setup(4, Vsew::E32, Vlmul::M2);
    let result = exec(
        &mut state,
        ZvbbInstruction::VbrevV {
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

#[test]
fn error_misaligned_vs2_lmul_m4() {
    // M4: group_regs=4; vs2 must be multiple of 4; V2 is not
    let mut state = setup(4, Vsew::E32, Vlmul::M4);
    let result = exec(
        &mut state,
        ZvbbInstruction::VcpopV {
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

// vwsll-specific error paths

#[test]
fn error_vwsll_sew_e64_illegal() {
    // E64 has no double width; vwsll is illegal at SEW=E64
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    let result = exec(
        &mut state,
        ZvbbInstruction::VwsllVv {
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
fn error_vwsll_lmul_m8_illegal() {
    // LMUL=M8 with any SEW gives EMUL(vd)=16, out of [1/8, 8] range
    let mut state = setup(1, Vsew::E8, Vlmul::M8);
    let result = exec(
        &mut state,
        ZvbbInstruction::VwsllVv {
            vd: VReg::V0,
            vs2: VReg::V0,
            vs1: VReg::V0,
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
fn error_vwsll_misaligned_dest_lmul_m1() {
    // LMUL=M1, SEW=E8: dest uses EMUL=M2, so vd must be even; V1 is misaligned
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    let result = exec(
        &mut state,
        ZvbbInstruction::VwsllVv {
            vd: VReg::V1,
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
fn error_vwsll_misaligned_vs1_lmul_m2() {
    // LMUL=M2: source group_regs=2; vs1=V3 is not a multiple of 2
    let mut state = setup(2, Vsew::E8, Vlmul::M2);
    let result = exec(
        &mut state,
        ZvbbInstruction::VwsllVv {
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
