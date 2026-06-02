use crate::rv64::test_utils::{TestInterpreterState, initialize_state};
use crate::v::vector_registers::{VectorRegisters, VectorRegistersExt};
use crate::v::zve64x::arith::zve64x_arith_helpers::sign_extend;
use crate::{
    ExecutableInstruction, ExecutableInstructionOperands, ExecutionError, RegisterFile,
    Rs1Rs2OperandValues, Rs1Rs2Operands,
};
use ab_riscv_primitives::prelude::*;

fn encode_vtype(vsew: Vsew, vlmul: Vlmul) -> u64 {
    u64::from(vlmul.to_bits()) | (u64::from(vsew.to_bits()) << 3u8)
}

fn setup(
    vl: u32,
    vsew: Vsew,
    vlmul: Vlmul,
) -> TestInterpreterState<Zve64xFixedPointInstruction<Reg<u64>>> {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    let vtype = Vtype::from_raw::<Reg<u64>>(encode_vtype(vsew, vlmul)).unwrap();
    state.ext_state.set_vtype(Some(vtype));
    state.ext_state.set_vl(vl);
    state.ext_state.set_vstart(0);
    state
}

fn setup_with_vxrm(
    vl: u32,
    vsew: Vsew,
    vlmul: Vlmul,
    vxrm: Vxrm,
) -> TestInterpreterState<Zve64xFixedPointInstruction<Reg<u64>>> {
    let mut state = setup(vl, vsew, vlmul);
    state.ext_state.set_vxrm(vxrm);
    state
}

fn exec(
    state: &mut TestInterpreterState<Zve64xFixedPointInstruction<Reg<u64>>>,
    instr: Zve64xFixedPointInstruction<Reg<u64>>,
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

fn write_elem(
    state: &mut TestInterpreterState<Zve64xFixedPointInstruction<Reg<u64>>>,
    base_reg: VReg,
    elem_i: usize,
    sew: Vsew,
    value: u64,
) {
    let sew_bytes = usize::from(sew.bytes());
    let elems_per_reg = 16 / sew_bytes;
    let reg_off = elem_i / elems_per_reg;
    let byte_off = (elem_i % elems_per_reg) * sew_bytes;
    let reg = &mut state.ext_state.write_vreg()[usize::from(base_reg.bits()) + reg_off];
    let buf = value.to_le_bytes();
    reg[byte_off..byte_off + sew_bytes].copy_from_slice(&buf[..sew_bytes]);
}

fn read_elem(
    state: &TestInterpreterState<Zve64xFixedPointInstruction<Reg<u64>>>,
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

// Write a 2*SEW-wide element into a double-width register group (for narrowing source)
fn write_wide_elem(
    state: &mut TestInterpreterState<Zve64xFixedPointInstruction<Reg<u64>>>,
    base_reg: VReg,
    elem_i: usize,
    sew: Vsew,
    value: u64,
) {
    let wide_bytes = usize::from(sew.bytes()) * 2;
    let elems_per_reg = 16 / wide_bytes;
    let reg_off = elem_i / elems_per_reg;
    let byte_off = (elem_i % elems_per_reg) * wide_bytes;
    let reg = &mut state.ext_state.write_vreg()[usize::from(base_reg.bits()) + reg_off];
    let buf = value.to_le_bytes();
    reg[byte_off..byte_off + wide_bytes].copy_from_slice(&buf[..wide_bytes]);
}

fn set_mask_bit(
    state: &mut TestInterpreterState<Zve64xFixedPointInstruction<Reg<u64>>>,
    reg: VReg,
    i: u32,
    val: bool,
) {
    let byte = &mut state.ext_state.write_vreg()[usize::from(reg.bits())][(i / u8::BITS) as usize];
    if val {
        *byte |= 1 << (i % u8::BITS);
    } else {
        *byte &= !(1 << (i % u8::BITS));
    }
}

fn vxsat(state: &TestInterpreterState<Zve64xFixedPointInstruction<Reg<u64>>>) -> bool {
    state.ext_state.vxsat()
}

// vsaddu

#[test]
fn vsaddu_vv_e8_no_overflow() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 10);
        write_elem(&mut state, VReg::V1, i, Vsew::E8, 20);
    }
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VsadduVv {
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
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 30, "elem {i}");
    }
    assert!(!vxsat(&state), "vxsat must not be set on no-overflow");
    assert_eq!(state.ext_state.vstart(), 0);
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
}

#[test]
fn vsaddu_vv_e8_saturates_at_max() {
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 200);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 100);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0);
    write_elem(&mut state, VReg::V1, 1, Vsew::E8, 0);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VsadduVv {
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
        read_elem(&state, VReg::V4, 0, Vsew::E8),
        255,
        "saturated elem"
    );
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E8), 0, "no-sat elem");
    assert!(vxsat(&state), "vxsat must be set");
}

#[test]
fn vsaddu_vv_e32_saturates() {
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 0xFFFF_FFFE);
    write_elem(&mut state, VReg::V1, 0, Vsew::E32, 3);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VsadduVv {
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
    assert!(vxsat(&state));
}

#[test]
fn vsaddu_vv_e64_saturates() {
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E64, u64::MAX);
    write_elem(&mut state, VReg::V1, 0, Vsew::E64, 1);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VsadduVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E64), u64::MAX);
    assert!(vxsat(&state));
}

#[test]
fn vsaddu_vx_e16() {
    let mut state = setup(4, Vsew::E16, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E16, 0xFFF0);
    }
    state.regs.write(Reg::A0, 0x20);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VsadduVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4 {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E16), 0xFFFF);
    }
    assert!(vxsat(&state));
}

#[test]
fn vsaddu_vi_e8() {
    // Immediate is zero-extended for vsaddu
    let mut state = setup(3, Vsew::E8, Vlmul::M1);
    // 250 + 5 = 255 exactly - no overflow, vxsat must NOT be set
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 250);
    // 251 + 5 = 256 - saturates to 255
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 251);
    write_elem(&mut state, VReg::V2, 2, Vsew::E8, 1);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VsadduVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            imm: 5,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(
        read_elem(&state, VReg::V4, 0, Vsew::E8),
        255,
        "exact max, no saturation"
    );
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E8), 255, "saturated");
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E8), 6);
    assert!(vxsat(&state), "elem 1 saturated so vxsat must be set");
}

#[test]
fn vsaddu_vi_e8_high_bit_immediate_sign_extends() {
    // Per v-spec §11.1/§12.1: OPIVI immediate is SIGN-extended for vsaddu.vi.
    // imm=-1 (encoding 0b11111) sign-extends to 0xFF in an 8-bit element,
    // which as an unsigned operand is 255. Then:
    //   elem0: 10 + 255 = 265 -> saturates to 255
    //   elem1: 0  + 255 = 255 -> exact max, no saturation
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 10);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VsadduVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            imm: -1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(
        read_elem(&state, VReg::V4, 0, Vsew::E8),
        255,
        "10 + 255 saturates"
    );
    assert_eq!(
        read_elem(&state, VReg::V4, 1, Vsew::E8),
        255,
        "0 + 255 = 255 exact"
    );
    assert!(vxsat(&state), "elem 0 saturated");
}

#[test]
fn vsaddu_vi_e16_sign_extends_to_sew() {
    // imm=-1 sign-extends to 0xFFFF at SEW=16 (= 65535 unsigned).
    let mut state = setup(1, Vsew::E16, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E16, 1);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VsadduVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            imm: -1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 1 + 65535 = 65536 -> saturates to 0xFFFF
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E16), 0xFFFF);
    assert!(vxsat(&state));
}

#[test]
fn vsaddu_vxsat_is_sticky() {
    // First instruction saturates, second does not; vxsat should remain set
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 200);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 100);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VsadduVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert!(vxsat(&state));
    // Second instruction: no overflow
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 1);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 1);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VsadduVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert!(vxsat(&state), "vxsat must remain set (sticky)");
}

// vsadd

#[test]
fn vsadd_vv_e8_positive_overflow() {
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    // i8::MAX + 1 overflows
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 127);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 1);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VsaddVv {
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
        read_elem(&state, VReg::V4, 0, Vsew::E8),
        127,
        "clamped at i8::MAX"
    );
    assert!(vxsat(&state));
}

#[test]
fn vsadd_vv_e8_negative_overflow() {
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    // i8::MIN + (-1) underflows
    // -128
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0x80u64);
    // -1
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 0xFFu64);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VsaddVv {
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
        read_elem(&state, VReg::V4, 0, Vsew::E8),
        0x80,
        "clamped at i8::MIN"
    );
    assert!(vxsat(&state));
}

#[test]
fn vsadd_vv_e8_no_overflow() {
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 50);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 50);
    // -16
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0xF0u64);
    write_elem(&mut state, VReg::V1, 1, Vsew::E8, 10);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VsaddVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 100);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E8) as i8, -6i8);
    assert!(!vxsat(&state));
}

#[test]
fn vsadd_vv_e32_max_plus_one() {
    let mut state = setup(1, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 0x7FFF_FFFFu64);
    write_elem(&mut state, VReg::V1, 0, Vsew::E32, 1);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VsaddVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0x7FFF_FFFF);
    assert!(vxsat(&state));
}

#[test]
fn vsadd_vi_sign_extends_immediate() {
    // imm = -1 (0x1F as 5-bit signed) should sign-extend to -1 in i64
    let mut state = setup(1, Vsew::E16, Vlmul::M1);
    // 0 + (-1) = -1
    write_elem(&mut state, VReg::V2, 0, Vsew::E16, 0);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VsaddVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            imm: -1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // -1 as u16
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E16), 0xFFFF);
    assert!(!vxsat(&state));
}

// vssubu

#[test]
fn vssubu_vv_e8_no_underflow() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 50);
        write_elem(&mut state, VReg::V1, i, Vsew::E8, 30);
    }
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VssubuVv {
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
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 20);
    }
    assert!(!vxsat(&state));
}

#[test]
fn vssubu_vv_e8_clamps_at_zero() {
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 10);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 20);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0);
    write_elem(&mut state, VReg::V1, 1, Vsew::E8, 1);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VssubuVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 0);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E8), 0);
    assert!(vxsat(&state));
}

#[test]
fn vssubu_vx_e64_clamps() {
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E64, 5);
    state.regs.write(Reg::A0, 10u64);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VssubuVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E64), 0);
    assert!(vxsat(&state));
}

// vssub

#[test]
fn vssub_vv_e8_positive_underflow() {
    // i8::MIN - 1 underflows
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    // -128
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0x80u64);
    // 1
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 1);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VssubVv {
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
        read_elem(&state, VReg::V4, 0, Vsew::E8),
        0x80,
        "clamped at i8::MIN"
    );
    assert!(vxsat(&state));
}

#[test]
fn vssub_vv_e8_positive_overflow() {
    // i8::MAX - (-1) overflows
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 127);
    // -1
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 0xFFu64);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VssubVv {
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
        read_elem(&state, VReg::V4, 0, Vsew::E8),
        127,
        "clamped at i8::MAX"
    );
    assert!(vxsat(&state));
}

#[test]
fn vssub_vx_e32_no_overflow() {
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 100);
    // -256
    write_elem(&mut state, VReg::V2, 1, Vsew::E32, 0xFFFF_FF00u64);
    state.regs.write(Reg::A1, 50u64);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VssubVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A1,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 50);
    assert_eq!(
        read_elem(&state, VReg::V4, 1, Vsew::E32),
        (0xFFFF_FF00u64.wrapping_sub(50)) & 0xFFFF_FFFF
    );
    assert!(!vxsat(&state));
}

// vaaddu

#[test]
fn vaaddu_vv_e8_rnu_basic() {
    // (3 + 4) >> 1 = 3 with rnu (round-up): (7 >> 1) + round = 3 + 1 = 4? No:
    // rnu: increment = bit[0] of sum before shift = 7 & 1 = 1, so result = 3 + 1 = 4
    let mut state = setup_with_vxrm(1, Vsew::E8, Vlmul::M1, Vxrm::Rnu);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 3);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 4);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VaadduVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // sum=7 (odd), rnu: round bit = 7[0] = 1, result = 3+1 = 4
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 4);
    assert!(!vxsat(&state), "averaging does not set vxsat");
}

#[test]
fn vaaddu_vv_e8_rdn_truncates() {
    let mut state = setup_with_vxrm(1, Vsew::E8, Vlmul::M1, Vxrm::Rdn);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 3);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 4);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VaadduVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // sum=7, rdn: truncate -> 3
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 3);
}

#[test]
fn vaaddu_vv_e8_even_sum_all_modes_same() {
    // sum = 6 (even): all rounding modes give 3
    for mode in [Vxrm::Rnu, Vxrm::Rne, Vxrm::Rdn, Vxrm::Rod] {
        let mut state = setup_with_vxrm(1, Vsew::E8, Vlmul::M1, mode);
        write_elem(&mut state, VReg::V2, 0, Vsew::E8, 2);
        write_elem(&mut state, VReg::V1, 0, Vsew::E8, 4);
        exec(
            &mut state,
            Zve64xFixedPointInstruction::VaadduVv {
                vd: VReg::V4,
                vs2: VReg::V2,
                vs1: VReg::V1,
                vm: true,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
        )
        .unwrap();
        assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 3, "mode={mode:?}");
    }
}

#[test]
fn vaaddu_vv_e8_overflow_wraps_correctly() {
    // 255 + 255 = 510; (510) >> 1 = 255 - no overflow because we use the extra bit
    let mut state = setup_with_vxrm(1, Vsew::E8, Vlmul::M1, Vxrm::Rdn);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 255);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 255);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VaadduVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 255);
}

#[test]
fn vaaddu_vx_e32() {
    let mut state = setup_with_vxrm(2, Vsew::E32, Vlmul::M1, Vxrm::Rnu);
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 5);
    write_elem(&mut state, VReg::V2, 1, Vsew::E32, 6);
    state.regs.write(Reg::A0, 5u64);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VaadduVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // (5+5)>>1 = 5 exactly, (6+5)=11 odd, rnu: 5+1=6
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 5);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E32), 6);
}

// vaadd

#[test]
fn vaadd_vv_e8_rnu_signed() {
    // -3 + -4 = -7. Truncated arithmetic >>1 = -4 (floor toward -inf).
    // Round bit = bit[0] of the i128 sum (-7) = 1. Rnu increments by 1: -4 + 1 = -3.
    let mut state = setup_with_vxrm(1, Vsew::E8, Vlmul::M1, Vxrm::Rnu);
    // -3
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xFDu64);
    // -4
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 0xFCu64);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VaaddVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    let result = read_elem(&state, VReg::V4, 0, Vsew::E8);
    assert_eq!(sign_extend(result, Vsew::E8), -3);
}

#[test]
fn vaadd_vv_e8_rdn_signed() {
    let mut state = setup_with_vxrm(1, Vsew::E8, Vlmul::M1, Vxrm::Rdn);
    // -3
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xFDu64);
    // -4
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 0xFCu64);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VaaddVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // -7 >> 1 = -4 (truncate), rdn: no round -> -4
    let result = read_elem(&state, VReg::V4, 0, Vsew::E8);
    assert_eq!(sign_extend(result, Vsew::E8), -4);
}

#[test]
fn vaadd_vv_e8_no_overflow() {
    // 127 + (-1) = 126, /2 = 63
    let mut state = setup_with_vxrm(1, Vsew::E8, Vlmul::M1, Vxrm::Rdn);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 127);
    // -1
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 0xFFu64);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VaaddVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // (127 + (-1)) / 2 = 63
    assert_eq!(
        sign_extend(read_elem(&state, VReg::V4, 0, Vsew::E8), Vsew::E8),
        63
    );
    assert!(!vxsat(&state));
}

#[test]
fn vaadd_vx_e64_rne() {
    // Test rne: (3 + 4) = 7, truncated = 3, round bit = 1, sticky = 0, result_lsb = 1
    // Rne: increment = round_bit & (sticky | result_lsb) = 1 & 1 = 1 -> 4
    let mut state = setup_with_vxrm(1, Vsew::E64, Vlmul::M1, Vxrm::Rne);
    write_elem(&mut state, VReg::V2, 0, Vsew::E64, 3);
    state.regs.write(Reg::A0, 4u64);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VaaddVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E64), 4);
}

// vasubu

#[test]
fn vasubu_vv_e8_rdn() {
    // (5 - 2) = 3 >> 1 = 1 with rdn
    let mut state = setup_with_vxrm(1, Vsew::E8, Vlmul::M1, Vxrm::Rdn);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 5);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 2);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VasubuVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 1);
    assert!(!vxsat(&state));
}

#[test]
fn vasubu_vv_e8_rnu_odd_diff() {
    // (5 - 2) = 3, rnu: round bit = 1 -> 1+1 = 2
    let mut state = setup_with_vxrm(1, Vsew::E8, Vlmul::M1, Vxrm::Rnu);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 5);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 2);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VasubuVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 2);
}

#[test]
fn vasubu_vx_e8_underflow_wraps_to_large() {
    // vasubu computes the exact (SEW+1)-bit difference and right-shifts by 1.
    // 2 - 5 borrows: the 9-bit two's-complement result is 0b1_1111_1101 = 509.
    // 509 >> 1 = 254 with rdn (no round bit since bit[0] of 509 = 1 is discarded, but rdn ignores
    // it).
    // Equivalently: borrow=1 fills the sign bit, diff byte wraps to 0xFD=253.
    // Arithmetic >>1: 0x80 | (0xFD >> 1) = 0x80 | 0x7E = 0xFE = 254.
    let mut state = setup_with_vxrm(1, Vsew::E8, Vlmul::M1, Vxrm::Rdn);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 2);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 5);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VasubuVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 254);
    assert!(!vxsat(&state));
}

// vasub

#[test]
fn vasub_vv_e8_rnu() {
    // (-3 - 2) = -5, >> 1 with rnu: -5 has LSB=1, rnu increments -> -2
    let mut state = setup_with_vxrm(1, Vsew::E8, Vlmul::M1, Vxrm::Rnu);
    // -3
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xFDu64);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 2);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VasubVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // -5 >> 1 = -3 (arithmetic), rnu: bit[-1] = 1 -> -3 + 1 = -2
    assert_eq!(
        sign_extend(read_elem(&state, VReg::V4, 0, Vsew::E8), Vsew::E8),
        -2
    );
    assert!(!vxsat(&state));
}

#[test]
fn vasub_vx_e16_rdn() {
    let mut state = setup_with_vxrm(1, Vsew::E16, Vlmul::M1, Vxrm::Rdn);
    // i16::MIN
    write_elem(&mut state, VReg::V2, 0, Vsew::E16, 0x8000);
    // -32767
    state.regs.write(Reg::A0, 0xFFFF_FFFF_FFFF_8001u64);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VasubVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // -32768 - (-32767) = -1, >> 1 with rdn: -1 >> 1 = -1 (arithmetic), but -1 is odd, rdn: 0 +
    // (-1) = -1 -1 >> 1 = 0 (truncate toward zero in integer arithmetic); but rdn truncates
    // toward -inf i128: -1 >> 1 = -1 (arithmetic), rdn adds 0 -> result = -1
    assert_eq!(
        sign_extend(read_elem(&state, VReg::V4, 0, Vsew::E16), Vsew::E16),
        -1
    );
}

// vsmul

#[test]
fn vsmul_vv_e8_basic() {
    // vsmul: (a * b * 2 + round) >> SEW
    // 2 * 3 = 6, *2 = 12, >> 8 = 0 with any rounding (12 < 128)
    // But for small values the result is always 0: (2*3*2)>>8 = 12>>8 = 0
    let mut state = setup_with_vxrm(1, Vsew::E8, Vlmul::M1, Vxrm::Rdn);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 2);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 3);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VsmulVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 0);
    assert!(!vxsat(&state));
}

#[test]
fn vsmul_vv_e8_larger_values() {
    // 64 * 64 = 4096, *2 = 8192, >> 8 = 32 (no rounding needed)
    let mut state = setup_with_vxrm(1, Vsew::E8, Vlmul::M1, Vxrm::Rdn);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 64);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 64);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VsmulVv {
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
        sign_extend(read_elem(&state, VReg::V4, 0, Vsew::E8), Vsew::E8),
        32
    );
    assert!(!vxsat(&state));
}

#[test]
fn vsmul_vv_e8_int_min_saturates() {
    // i8::MIN * i8::MIN: -128 * -128 = 16384, *2 overflows i16; result saturates at i8::MAX = 127
    let mut state = setup_with_vxrm(1, Vsew::E8, Vlmul::M1, Vxrm::Rdn);
    // -128
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0x80u64);
    // -128
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 0x80u64);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VsmulVv {
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
        sign_extend(read_elem(&state, VReg::V4, 0, Vsew::E8), Vsew::E8),
        127
    );
    assert!(vxsat(&state));
}

#[test]
fn vsmul_vv_e64_int_min_saturates() {
    // i64::MIN * i64::MIN: product = 2^126, *2 = 2^127 which overflows i128.
    // The implementation must detect this before multiplying and return i64::MAX with vxsat set.
    let mut state = setup_with_vxrm(1, Vsew::E64, Vlmul::M1, Vxrm::Rdn);
    write_elem(&mut state, VReg::V2, 0, Vsew::E64, i64::MIN.cast_unsigned());
    write_elem(&mut state, VReg::V1, 0, Vsew::E64, i64::MIN.cast_unsigned());
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VsmulVv {
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
        sign_extend(read_elem(&state, VReg::V4, 0, Vsew::E64), Vsew::E64),
        i64::MAX,
        "INT64_MIN * INT64_MIN must saturate to INT64_MAX"
    );
    assert!(vxsat(&state));
}

#[test]
fn vsmul_vv_e16_rnu_rounding() {
    // Choose values where the round bit (bit SEW-1 of the doubled product) is 1.
    // a=b=181: a*b=32761, *2=65522. 65522>>16=0, round bit = (65522>>15)&1 = (65522/32768)&1.
    // 65522 = 0xFFB2; bit 15 = 1 -> rnu increments: 0+1 = 1.
    // Verify: 181*181=32761, *2=65522=0xFFB2, >>16=0, bit[15]=1 -> result=1
    let mut state = setup_with_vxrm(1, Vsew::E16, Vlmul::M1, Vxrm::Rnu);
    write_elem(&mut state, VReg::V2, 0, Vsew::E16, 181);
    write_elem(&mut state, VReg::V1, 0, Vsew::E16, 181);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VsmulVv {
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
        sign_extend(read_elem(&state, VReg::V4, 0, Vsew::E16), Vsew::E16),
        1
    );
    assert!(!vxsat(&state));
    // Contrast with rdn: same inputs -> 0 (round bit discarded)
    let mut state2 = setup_with_vxrm(1, Vsew::E16, Vlmul::M1, Vxrm::Rdn);
    write_elem(&mut state2, VReg::V2, 0, Vsew::E16, 181);
    write_elem(&mut state2, VReg::V1, 0, Vsew::E16, 181);
    exec(
        &mut state2,
        Zve64xFixedPointInstruction::VsmulVv {
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
        sign_extend(read_elem(&state2, VReg::V4, 0, Vsew::E16), Vsew::E16),
        0
    );
}

#[test]
fn vsmul_vx_e16_negative() {
    // -100 * 200 = -20000, *2 = -40000, >> 16 = -1 (arithmetic, since -40000 / 65536 = -0.6... ->
    // -1) -40000 = 0xFFFF_6300; bit[15] = 0 -> rdn gives -1
    let mut state = setup_with_vxrm(1, Vsew::E16, Vlmul::M1, Vxrm::Rdn);
    // -100 as u16
    write_elem(&mut state, VReg::V2, 0, Vsew::E16, 0xFF9Cu64);
    state.regs.write(Reg::A0, 200u64);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VsmulVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(
        sign_extend(read_elem(&state, VReg::V4, 0, Vsew::E16), Vsew::E16),
        -1
    );
    assert!(!vxsat(&state));
}

// vssrl

#[test]
fn vssrl_vv_e8_rdn_basic() {
    // 0b1010_1010 >> 2 = 0b0010_1010 = 42 with rdn (truncate)
    let mut state = setup_with_vxrm(1, Vsew::E8, Vlmul::M1, Vxrm::Rdn);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0b1010_1010);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 2);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VssrlVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 42);
    assert!(!vxsat(&state));
}

#[test]
fn vssrl_vv_e8_rnu_rounds_up() {
    // 0b0000_0011 >> 1 = 1, round bit = 1 -> 2
    let mut state = setup_with_vxrm(1, Vsew::E8, Vlmul::M1, Vxrm::Rnu);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 3);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 1);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VssrlVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 2);
}

#[test]
fn vssrl_vv_e8_shift_zero() {
    // Shift by 0: no shift, no rounding
    let mut state = setup_with_vxrm(1, Vsew::E8, Vlmul::M1, Vxrm::Rnu);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xAB);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 0);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VssrlVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 0xAB);
}

#[test]
fn vssrl_vv_e8_shift_masked_to_log2_sew() {
    // Shift amount is masked to log2(8) = 3 bits; shift of 11 = 0b1011 masked to 3 -> 3
    let mut state = setup_with_vxrm(1, Vsew::E8, Vlmul::M1, Vxrm::Rdn);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xFF);
    // vs1 = 11 = 0b1011; masked to low 3 bits = 0b011 = 3
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 11);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VssrlVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 0xFF >> 3 = 31 with rdn
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 31);
}

#[test]
fn vssrl_vx_e32_rne() {
    // 7 = 0b111, shift by 1: truncated = 3 (odd), round bit = bit[0] of 7 = 1, sticky = 0
    // (no bits below position 0 exist).
    // Rne: increment = round_bit & (sticky | result_lsb) = 1 & (0 | 1) = 1 -> 3 + 1 = 4.
    let mut state = setup_with_vxrm(1, Vsew::E32, Vlmul::M1, Vxrm::Rne);
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 7);
    state.regs.write(Reg::A0, 1u64);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VssrlVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 4);
}

#[test]
fn vssrl_vi_e16_rod() {
    // 0b0110 >> 2 = 1, round bit = bit[1] of 6 = 1, sticky = bit[0] of 6 = 0
    // rod: if result_lsb == 0 && (round_bit || sticky): set result to 1 | 1 = 1; but result already
    // odd? 6 >> 2 = 1 (odd), rod: result_lsb = 1, so no increment -> 1
    let mut state = setup_with_vxrm(1, Vsew::E16, Vlmul::M1, Vxrm::Rod);
    write_elem(&mut state, VReg::V2, 0, Vsew::E16, 6);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VssrlVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            imm: 2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E16), 1);
}

#[test]
fn vssrl_vi_e16_rod_sets_lsb() {
    // 0b1000 = 8 >> 2 = 2 (even), rod: round bit = bit[1] of 8 = 0, sticky = bit[0] of 8 = 0
    // No discarded bits set -> no increment. Result = 2.
    // Try 0b1100 = 12 >> 2 = 3 (odd), no change needed.
    // Try 0b1010 = 10 >> 2 = 2 (even), round bit = bit[1] of 10 = 1, sticky = 0
    // rod: result_lsb = 0 and discarded != 0 -> set lsb: 2 | 1 = 3
    let mut state = setup_with_vxrm(1, Vsew::E16, Vlmul::M1, Vxrm::Rod);
    write_elem(&mut state, VReg::V2, 0, Vsew::E16, 10);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VssrlVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            imm: 2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E16), 3);
}

// vssra

#[test]
fn vssra_vv_e8_rdn_negative() {
    // -8 >> 2 = -2 with rdn (arithmetic, truncate toward -inf)
    let mut state = setup_with_vxrm(1, Vsew::E8, Vlmul::M1, Vxrm::Rdn);
    // -8
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xF8u64);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 2);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VssraVv {
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
        sign_extend(read_elem(&state, VReg::V4, 0, Vsew::E8), Vsew::E8),
        -2
    );
    assert!(!vxsat(&state));
}

#[test]
fn vssra_vv_e8_rnu_negative() {
    // -7 >> 1: arithmetic = -4, round bit = 1 (bit[0] of -7 = 1) -> -4 + 1 = -3
    let mut state = setup_with_vxrm(1, Vsew::E8, Vlmul::M1, Vxrm::Rnu);
    // -7
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xF9u64);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 1);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VssraVv {
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
        sign_extend(read_elem(&state, VReg::V4, 0, Vsew::E8), Vsew::E8),
        -3
    );
}

#[test]
fn vssra_vv_e8_positive_rnu() {
    // 7 >> 1 = 3, round bit = 1 -> 4
    let mut state = setup_with_vxrm(1, Vsew::E8, Vlmul::M1, Vxrm::Rnu);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 7);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 1);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VssraVv {
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
        sign_extend(read_elem(&state, VReg::V4, 0, Vsew::E8), Vsew::E8),
        4
    );
}

#[test]
fn vssra_vx_e32() {
    let mut state = setup_with_vxrm(1, Vsew::E32, Vlmul::M1, Vxrm::Rdn);
    // -256
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 0xFFFF_FF00u64);
    state.regs.write(Reg::A0, 4u64);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VssraVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // -256 >> 4 = -16 exactly
    assert_eq!(
        sign_extend(read_elem(&state, VReg::V4, 0, Vsew::E32), Vsew::E32),
        -16
    );
}

#[test]
fn vssra_vi_e64_rne_tie_to_even() {
    // 6 >> 1 = 3 (odd result), round bit = 0 -> 3
    // 2 >> 1 = 1 (odd), round bit = 0 -> 1
    // Test tie-to-even: value whose lower bits create a half-way case with even result
    // 4 >> 1 = 2, round bit = 0 -> 2 (no tie)
    // 6 >> 1 = 3, round bit = 0 -> 3
    // Try 0b110 (6) shifted by 2: truncated = 1, round_bit = 1, sticky = 1
    // rne: increment = 1 & (sticky | result_lsb) = 1 & (1 | 1) = 1 -> 2
    let mut state = setup_with_vxrm(1, Vsew::E64, Vlmul::M1, Vxrm::Rne);
    write_elem(&mut state, VReg::V2, 0, Vsew::E64, 6);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VssraVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            imm: 2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(
        sign_extend(read_elem(&state, VReg::V4, 0, Vsew::E64), Vsew::E64),
        2
    );
}

// vnclipu

#[test]
fn vnclipu_wv_e8_no_clip() {
    // dest SEW=8, source SEW=16; shift right by 4 with rdn; result fits in u8
    let mut state = setup_with_vxrm(2, Vsew::E8, Vlmul::M1, Vxrm::Rdn);
    // Write 16-bit wide source values
    // >> 4 = 15
    write_wide_elem(&mut state, VReg::V4, 0, Vsew::E8, 0x00F0);
    // >> 4 = 8
    write_wide_elem(&mut state, VReg::V4, 1, Vsew::E8, 0x0080);
    // Write shift amount (SEW=8) in vs1
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 4);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 4);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VnclipuWv {
            vd: VReg::V8,
            vs2: VReg::V4,
            vs1: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E8), 15);
    assert_eq!(read_elem(&state, VReg::V8, 1, Vsew::E8), 8);
    assert!(!vxsat(&state));
}

#[test]
fn vnclipu_wv_e8_saturates() {
    // 0x0200 >> 1 = 0x100 = 256, saturates to 255
    let mut state = setup_with_vxrm(1, Vsew::E8, Vlmul::M1, Vxrm::Rdn);
    write_wide_elem(&mut state, VReg::V4, 0, Vsew::E8, 0x0200);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 1);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VnclipuWv {
            vd: VReg::V8,
            vs2: VReg::V4,
            vs1: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E8), 255);
    assert!(vxsat(&state));
}

#[test]
fn vnclipu_wx_e16_rnu() {
    // 0x0001_FFFF >> 16 = 1, round bit = bit[15] of 0x0001_FFFF = 1 -> saturates? No: 1+1=2 ≤
    // 0xFFFF
    let mut state = setup_with_vxrm(1, Vsew::E16, Vlmul::M1, Vxrm::Rnu);
    write_wide_elem(&mut state, VReg::V4, 0, Vsew::E16, 0x0001_FFFF);
    state.regs.write(Reg::A0, 16u64);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VnclipuWx {
            vd: VReg::V8,
            vs2: VReg::V4,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E16), 2);
    assert!(!vxsat(&state));
}

#[test]
fn vnclipu_wi_e8_shift_zero() {
    // shift=0: result = value itself; if > 255, saturate
    let mut state = setup_with_vxrm(1, Vsew::E8, Vlmul::M1, Vxrm::Rdn);
    // 511 > 255
    write_wide_elem(&mut state, VReg::V4, 0, Vsew::E8, 0x01FF);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VnclipuWi {
            vd: VReg::V8,
            vs2: VReg::V4,
            imm: 0,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E8), 255);
    assert!(vxsat(&state));
}

#[test]
fn vnclipu_e64_illegal() {
    // Narrowing with dest SEW=64 requires source SEW=128 which exceeds ELEN=64; must fault
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xFixedPointInstruction::VnclipuWi {
            vd: VReg::V8,
            vs2: VReg::V4,
            imm: 0,
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
fn vnclipu_shamt_masked_to_log2_2sew() {
    // For SEW=8, shamt masked to log2(16)=4 bits. shamt=0x1F masked = 0x0F=15
    // 0xFFFF >> 15 = 1 with rdn; 0xFFFF >> 15 = 1 remainder 0x7FFF, rdn gives 1
    let mut state = setup_with_vxrm(1, Vsew::E8, Vlmul::M1, Vxrm::Rdn);
    write_wide_elem(&mut state, VReg::V4, 0, Vsew::E8, 0xFFFF);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VnclipuWi {
            vd: VReg::V8,
            vs2: VReg::V4,
            imm: 0x1F,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 0x1F & 0x0F = 15; 0xFFFF >> 15 = 1
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E8), 1);
}

#[test]
fn vnclipu_lmul8_illegal() {
    // Per v-spec §5.2: narrowing requires 2*LMUL <= 8, so LMUL=8 is reserved.
    let mut state = setup(1, Vsew::E8, Vlmul::M8);
    let result = exec(
        &mut state,
        Zve64xFixedPointInstruction::VnclipuWi {
            vd: VReg::V8,
            vs2: VReg::V0,
            imm: 0,
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
fn vnclip_lmul8_illegal() {
    let mut state = setup(1, Vsew::E8, Vlmul::M8);
    let result = exec(
        &mut state,
        Zve64xFixedPointInstruction::VnclipWi {
            vd: VReg::V8,
            vs2: VReg::V0,
            imm: 0,
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

// vnclip

#[test]
fn vnclip_wv_e8_no_clip() {
    // -10 (as i16 = 0xFFF6) >> 2 = -3 (arithmetic with rdn), fits in i8
    let mut state = setup_with_vxrm(1, Vsew::E8, Vlmul::M1, Vxrm::Rdn);
    // -10 as i16
    write_wide_elem(&mut state, VReg::V4, 0, Vsew::E8, 0xFFF6u64);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 2);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VnclipWv {
            vd: VReg::V8,
            vs2: VReg::V4,
            vs1: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // -10 >> 2 = -3 (arithmetic with rdn: floor(-10/4) = -3)
    assert_eq!(
        sign_extend(read_elem(&state, VReg::V8, 0, Vsew::E8), Vsew::E8),
        -3
    );
    assert!(!vxsat(&state));
}

#[test]
fn vnclip_wv_e8_positive_saturates_at_max() {
    // 0x7FFF >> 7 = 0xFF > 127; saturates to 127
    let mut state = setup_with_vxrm(1, Vsew::E8, Vlmul::M1, Vxrm::Rdn);
    write_wide_elem(&mut state, VReg::V4, 0, Vsew::E8, 0x7FFF);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 7);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VnclipWv {
            vd: VReg::V8,
            vs2: VReg::V4,
            vs1: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 0x7FFF >> 7 = 0xFF = 255 > 127 -> saturate to 127
    assert_eq!(
        sign_extend(read_elem(&state, VReg::V8, 0, Vsew::E8), Vsew::E8),
        127
    );
    assert!(vxsat(&state));
}

#[test]
fn vnclip_wv_e8_negative_saturates_at_min() {
    // 0x8000 = -32768 (i16) >> 7 = -256; saturates to -128
    let mut state = setup_with_vxrm(1, Vsew::E8, Vlmul::M1, Vxrm::Rdn);
    write_wide_elem(&mut state, VReg::V4, 0, Vsew::E8, 0x8000);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 7);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VnclipWv {
            vd: VReg::V8,
            vs2: VReg::V4,
            vs1: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(
        sign_extend(read_elem(&state, VReg::V8, 0, Vsew::E8), Vsew::E8),
        -128
    );
    assert!(vxsat(&state));
}

#[test]
fn vnclip_wx_e16_rnu() {
    // -1 (0xFFFF_FFFF as i32) >> 1 with rnu: -1>>1 = -1 (arithmetic), round bit = 1 -> 0
    let mut state = setup_with_vxrm(1, Vsew::E16, Vlmul::M1, Vxrm::Rnu);
    // -1 as i32
    write_wide_elem(&mut state, VReg::V4, 0, Vsew::E16, 0xFFFF_FFFFu64);
    state.regs.write(Reg::A0, 1u64);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VnclipWx {
            vd: VReg::V8,
            vs2: VReg::V4,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // -1 >> 1 = -1 (floor), round bit = bit[0] of -1 = 1, rnu: -1 + 1 = 0
    assert_eq!(
        sign_extend(read_elem(&state, VReg::V8, 0, Vsew::E16), Vsew::E16),
        0
    );
    assert!(!vxsat(&state));
}

#[test]
fn vnclip_wi_e8() {
    // 0x007F >> 0 = 127; fits exactly in i8
    let mut state = setup_with_vxrm(1, Vsew::E8, Vlmul::M1, Vxrm::Rdn);
    write_wide_elem(&mut state, VReg::V4, 0, Vsew::E8, 127);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VnclipWi {
            vd: VReg::V8,
            vs2: VReg::V4,
            imm: 0,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(
        sign_extend(read_elem(&state, VReg::V8, 0, Vsew::E8), Vsew::E8),
        127
    );
    assert!(!vxsat(&state));
}

#[test]
fn vnclip_e64_illegal() {
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xFixedPointInstruction::VnclipWi {
            vd: VReg::V8,
            vs2: VReg::V4,
            imm: 0,
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

// masking (vm=false)

#[test]
fn vsaddu_masked_skips_inactive_elements() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 100);
        write_elem(&mut state, VReg::V1, i, Vsew::E8, 200);
        // Set destination to a sentinel value
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 0x55);
    }
    // Mask: only elements 0 and 2 are active
    set_mask_bit(&mut state, VReg::V0, 0, true);
    set_mask_bit(&mut state, VReg::V0, 1, false);
    set_mask_bit(&mut state, VReg::V0, 2, true);
    set_mask_bit(&mut state, VReg::V0, 3, false);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VsadduVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // Elements 0 and 2: saturated to 255
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 255);
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E8), 255);
    // Elements 1 and 3: undisturbed
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E8), 0x55);
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E8), 0x55);
}

#[test]
fn vsaddu_masked_vd_overlap_v0_illegal() {
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    // vd = v0 with vm=false is illegal
    let result = exec(
        &mut state,
        Zve64xFixedPointInstruction::VsadduVv {
            vd: VReg::V0,
            vs2: VReg::V2,
            vs1: VReg::V1,
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

#[test]
fn vssrl_masked_only_active_written() {
    let mut state = setup_with_vxrm(4, Vsew::E8, Vlmul::M1, Vxrm::Rdn);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0xFF);
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 0xAA);
    }
    set_mask_bit(&mut state, VReg::V0, 0, true);
    set_mask_bit(&mut state, VReg::V0, 1, false);
    set_mask_bit(&mut state, VReg::V0, 2, false);
    set_mask_bit(&mut state, VReg::V0, 3, true);
    state.regs.write(Reg::A0, 4u64);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VssrlVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: false,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 0xFF >> 4u8);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E8), 0xAA);
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E8), 0xAA);
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E8), 0xFF >> 4u8);
}

// vstart partial execution

#[test]
fn vsaddu_vstart_skips_early_elements() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 200);
        write_elem(&mut state, VReg::V1, i, Vsew::E8, 100);
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 0x55);
    }
    // Set vstart = 2: skip elements 0 and 1
    state.ext_state.set_vstart(2);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VsadduVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // Elements 0, 1 are untouched
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 0x55);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E8), 0x55);
    // Elements 2, 3 are written
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E8), 255);
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E8), 255);
    // vstart is reset to 0 after execution
    assert_eq!(state.ext_state.vstart(), 0);
}

// vector not allowed

#[test]
fn vsaddu_vector_not_allowed_faults() {
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    state.ext_state.set_vector_allowed(false);
    let result = exec(
        &mut state,
        Zve64xFixedPointInstruction::VsadduVv {
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
fn vsmul_vector_not_allowed_faults() {
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    state.ext_state.set_vector_allowed(false);
    let result = exec(
        &mut state,
        Zve64xFixedPointInstruction::VsmulVv {
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
fn vnclip_vector_not_allowed_faults() {
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    state.ext_state.set_vector_allowed(false);
    let result = exec(
        &mut state,
        Zve64xFixedPointInstruction::VnclipWi {
            vd: VReg::V8,
            vs2: VReg::V4,
            imm: 0,
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

// vtype = None (vill=1) faults

#[test]
fn vsaddu_vtype_none_faults() {
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    state.ext_state.set_vtype(None);
    let result = exec(
        &mut state,
        Zve64xFixedPointInstruction::VsadduVv {
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
fn vssrl_vtype_none_faults() {
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    state.ext_state.set_vtype(None);
    let result = exec(
        &mut state,
        Zve64xFixedPointInstruction::VssrlVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            imm: 1,
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

// register alignment checks

#[test]
fn vsaddu_vd_misaligned_m2_faults() {
    // With M2, group_regs=2; vd must be even
    let mut state = setup(2, Vsew::E8, Vlmul::M2);
    // V3 is odd -> misaligned for M2
    let result = exec(
        &mut state,
        Zve64xFixedPointInstruction::VsadduVv {
            vd: VReg::V3,
            vs2: VReg::V2,
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
fn vsaddu_vs2_misaligned_m2_faults() {
    let mut state = setup(2, Vsew::E8, Vlmul::M2);
    let result = exec(
        &mut state,
        Zve64xFixedPointInstruction::VsadduVv {
            vd: VReg::V2,
            vs2: VReg::V3,
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
fn vnclipu_vs2_misaligned_m1_faults() {
    // Narrowing: vs2 must be aligned to 2*group_regs = 2; V3 is not divisible by 2
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xFixedPointInstruction::VnclipuWi {
            vd: VReg::V8,
            vs2: VReg::V3,
            imm: 0,
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
fn vnclip_wi_mf2_odd_vs2_ok() {
    // Regression: `vnclip.wi v15, v13, 4`.
    // With fractional LMUL the source EMUL = 2*LMUL <= 1, so vs2 occupies a single register with no
    // alignment constraint; an odd-numbered vs2 (V13) must be accepted. The previous check used
    // 2*register_count() = 2 and wrongly rejected it.
    let mut state = setup(1, Vsew::E8, Vlmul::Mf2);
    // 256 (as i16) >> 4 = 16, fits in i8, no rounding/saturation
    write_wide_elem(&mut state, VReg::V13, 0, Vsew::E8, 0x0100);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VnclipWi {
            vd: VReg::V15,
            vs2: VReg::V13,
            imm: 4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(
        sign_extend(read_elem(&state, VReg::V15, 0, Vsew::E8), Vsew::E8),
        16
    );
    assert!(!vxsat(&state));
}

#[test]
fn vnclipu_wi_mf4_odd_vs2_ok() {
    // Same fractional-LMUL alignment relaxation for the unsigned variant: Mf4 -> source EMUL = 1/2,
    // so a single, arbitrarily-aligned source register is legal.
    let mut state = setup(1, Vsew::E8, Vlmul::Mf4);
    write_wide_elem(&mut state, VReg::V7, 0, Vsew::E8, 0x00F0);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VnclipuWi {
            vd: VReg::V9,
            vs2: VReg::V7,
            imm: 4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 0xF0 >> 4 = 0x0F
    assert_eq!(read_elem(&state, VReg::V9, 0, Vsew::E8), 0x0F);
    assert!(!vxsat(&state));
}

#[test]
fn vnclip_vs2_misaligned_m2_faults() {
    // M2: source EMUL = 4, so vs2 must be aligned to 4; V2 is not.
    let mut state = setup(1, Vsew::E8, Vlmul::M2);
    let result = exec(
        &mut state,
        Zve64xFixedPointInstruction::VnclipWi {
            vd: VReg::V8,
            vs2: VReg::V2,
            imm: 0,
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
fn vsaddu_aligned_m4_ok() {
    // M4: group_regs=4; vd=V4 (divisible by 4), vs2=V8, vs1=V12 -> all valid
    let mut state = setup(1, Vsew::E8, Vlmul::M4);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V8, i, Vsew::E8, 1);
        write_elem(&mut state, VReg::V12, i, Vsew::E8, 2);
    }
    let result = exec(
        &mut state,
        Zve64xFixedPointInstruction::VsadduVv {
            vd: VReg::V4,
            vs2: VReg::V8,
            vs1: VReg::V12,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    );
    assert!(result.is_ok());
}

// vs_dirty_count and vstart reset

#[test]
fn vs_dirty_increments_per_instruction() {
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 1);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 1);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VsadduVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VsadduVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(state.ext_state.vs_dirty_count(), 2);
}

#[test]
fn vstart_resets_to_zero_after_execution() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    state.ext_state.set_vstart(2);
    write_elem(&mut state, VReg::V2, 2, Vsew::E8, 1);
    write_elem(&mut state, VReg::V1, 2, Vsew::E8, 1);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VsaddVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(state.ext_state.vstart(), 0, "vstart must be reset to 0");
}

// vl=0 does nothing

#[test]
fn vsaddu_vl_zero_no_writes() {
    let mut state = setup(0, Vsew::E8, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 0xAB);
    }
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VsadduVv {
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
            read_elem(&state, VReg::V4, i, Vsew::E8),
            0xAB,
            "elem {i} must be undisturbed"
        );
    }
    // mark_vs_dirty is still called; vstart still reset
    assert_eq!(state.ext_state.vstart(), 0);
}

// multiple SEW sizes for saturation arithmetic

#[test]
fn vsadd_all_sew_sizes_max_overflow() {
    for (vsew, max_val, min_val) in [
        (Vsew::E8, 0x7Fu64, 0x80u64),
        (Vsew::E16, 0x7FFFu64, 0x8000u64),
        (Vsew::E32, 0x7FFF_FFFFu64, 0x8000_0000u64),
        (
            Vsew::E64,
            i64::MAX.cast_unsigned(),
            i64::MIN.cast_unsigned(),
        ),
    ] {
        let mut state = setup(1, vsew, Vlmul::M1);
        write_elem(&mut state, VReg::V2, 0, vsew, max_val);
        write_elem(&mut state, VReg::V1, 0, vsew, 1);
        exec(
            &mut state,
            Zve64xFixedPointInstruction::VsaddVv {
                vd: VReg::V4,
                vs2: VReg::V2,
                vs1: VReg::V1,
                vm: true,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
        )
        .unwrap();
        assert_eq!(read_elem(&state, VReg::V4, 0, vsew), max_val, "SEW={vsew}");
        assert!(vxsat(&state), "SEW={vsew}");

        // Reset vxsat by reinitializing
        let mut state2 = setup(1, vsew, Vlmul::M1);
        write_elem(&mut state2, VReg::V2, 0, vsew, min_val);
        // -1 for any SEW
        write_elem(&mut state2, VReg::V1, 0, vsew, 0xFFFF_FFFF_FFFF_FFFFu64);
        exec(
            &mut state2,
            Zve64xFixedPointInstruction::VsaddVv {
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
            read_elem(&state2, VReg::V4, 0, vsew),
            min_val,
            "SEW={vsew} underflow"
        );
        assert!(vxsat(&state2), "SEW={vsew} underflow vxsat");
    }
}

#[test]
fn vssubu_all_sew_sizes_clamps_zero() {
    for vsew in [Vsew::E8, Vsew::E16, Vsew::E32, Vsew::E64] {
        let mut state = setup(1, vsew, Vlmul::M1);
        write_elem(&mut state, VReg::V2, 0, vsew, 5);
        write_elem(&mut state, VReg::V1, 0, vsew, 10);
        exec(
            &mut state,
            Zve64xFixedPointInstruction::VssubuVv {
                vd: VReg::V4,
                vs2: VReg::V2,
                vs1: VReg::V1,
                vm: true,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
        )
        .unwrap();
        assert_eq!(read_elem(&state, VReg::V4, 0, vsew), 0, "SEW={vsew}");
        assert!(vxsat(&state), "SEW={vsew}");
    }
}

// vnclipu e32 (largest valid narrowing dest)

#[test]
fn vnclipu_e32_no_clip() {
    // dest SEW=32, source SEW=64; value 0x0000_0001_0000_0000 >> 32 = 1
    let mut state = setup_with_vxrm(1, Vsew::E32, Vlmul::M1, Vxrm::Rdn);
    write_wide_elem(&mut state, VReg::V4, 0, Vsew::E32, 0x0000_0001_0000_0000u64);
    state.regs.write(Reg::A0, 32u64);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VnclipuWx {
            vd: VReg::V8,
            vs2: VReg::V4,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E32), 1);
    assert!(!vxsat(&state));
}

#[test]
fn vnclipu_e32_saturates() {
    // 0xFFFF_FFFF_FFFF_FFFFu64 >> 0 = 0xFFFF_FFFF_FFFF_FFFF > u32::MAX -> saturate
    let mut state = setup_with_vxrm(1, Vsew::E32, Vlmul::M1, Vxrm::Rdn);
    write_wide_elem(&mut state, VReg::V4, 0, Vsew::E32, u64::MAX);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VnclipuWi {
            vd: VReg::V8,
            vs2: VReg::V4,
            imm: 0,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(
        read_elem(&state, VReg::V8, 0, Vsew::E32),
        u64::from(u32::MAX)
    );
    assert!(vxsat(&state));
}

// vnclip e32

#[test]
fn vnclip_e32_no_clip() {
    // -1 (as i64) >> 32 = -1, fits in i32
    let mut state = setup_with_vxrm(1, Vsew::E32, Vlmul::M1, Vxrm::Rdn);
    // -1 as i64
    write_wide_elem(&mut state, VReg::V4, 0, Vsew::E32, u64::MAX);
    state.regs.write(Reg::A0, 32u64);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VnclipWx {
            vd: VReg::V8,
            vs2: VReg::V4,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // -1 >> 32 = -1 (arithmetic), fits in i32 -> -1
    assert_eq!(
        sign_extend(read_elem(&state, VReg::V8, 0, Vsew::E32), Vsew::E32),
        -1
    );
    assert!(!vxsat(&state));
}

#[test]
fn vnclip_e32_saturates_positive() {
    // i64::MAX >> 31 = 1 (>i32::MAX = 0x7FFF_FFFF), saturates
    // Actually i64::MAX >> 31 = 0xFFFF_FFFF which is > i32::MAX
    let mut state = setup_with_vxrm(1, Vsew::E32, Vlmul::M1, Vxrm::Rdn);
    write_wide_elem(&mut state, VReg::V4, 0, Vsew::E32, i64::MAX as u64);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VnclipWi {
            vd: VReg::V8,
            vs2: VReg::V4,
            imm: 31,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E32), i32::MAX as u64);
    assert!(vxsat(&state));
}

// rounding mode rod

#[test]
fn vssrl_rod_result_even_sets_lsb() {
    // 0b1100 = 12 >> 2 = 3 (odd), no change: rod doesn't set when already odd
    // 0b1000 = 8 >> 2 = 2 (even), round_bit=0, sticky=0 -> no increment; result = 2
    // 0b1010 = 10 >> 2 = 2 (even), round_bit=1, sticky=0 -> rod sets lsb: 3
    let mut state = setup_with_vxrm(2, Vsew::E8, Vlmul::M1, Vxrm::Rod);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 8);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 10);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VssrlVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            imm: 2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(
        read_elem(&state, VReg::V4, 0, Vsew::E8),
        2,
        "no discarded bits"
    );
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E8), 3, "rod sets lsb");
}

#[test]
fn vssra_rod_result_even_sets_lsb() {
    // -8 (0xF8) >> 2 = -2 (even), round_bit=0, sticky=0 -> no increment: -2
    // -6 (0xFA) >> 2 = -2 (even), round_bit=1, sticky=0 -> rod sets lsb: -2 | 1? No:
    // rod adds 1 when result is even and any discarded bit is set: -2 + 1 = -1
    let mut state = setup_with_vxrm(2, Vsew::E8, Vlmul::M1, Vxrm::Rod);
    // -8
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xF8u64);
    // -6
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0xFAu64);
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VssraVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            imm: 2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(
        sign_extend(read_elem(&state, VReg::V4, 0, Vsew::E8), Vsew::E8),
        -2,
        "no discarded bits"
    );
    assert_eq!(
        sign_extend(read_elem(&state, VReg::V4, 1, Vsew::E8), Vsew::E8),
        -1,
        "rod sets lsb"
    );
}

// multiple active elements with mixed saturation

#[test]
fn vsaddu_mixed_sat_e16_m1() {
    let mut state = setup(8, Vsew::E16, Vlmul::M1);
    let inputs: [(u64, u64); 8] = [
        (0, 0),
        (0xFFFF, 1),      // saturates
        (0x8000, 0x7FFF), // saturates
        (100, 200),
        (0xFFFE, 1), // saturates
        (0xFFFE, 0), // no saturate: result 0xFFFE
        (1, 1),
        (0xFFFF, 0),
    ];
    for (i, (a, b)) in inputs.iter().enumerate() {
        write_elem(&mut state, VReg::V2, i, Vsew::E16, *a);
        write_elem(&mut state, VReg::V1, i, Vsew::E16, *b);
    }
    exec(
        &mut state,
        Zve64xFixedPointInstruction::VsadduVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E16), 0);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E16), 0xFFFF);
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E16), 0xFFFF);
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E16), 300);
    assert_eq!(read_elem(&state, VReg::V4, 4, Vsew::E16), 0xFFFF);
    assert_eq!(read_elem(&state, VReg::V4, 5, Vsew::E16), 0xFFFE);
    assert_eq!(read_elem(&state, VReg::V4, 6, Vsew::E16), 2);
    assert_eq!(read_elem(&state, VReg::V4, 7, Vsew::E16), 0xFFFF);
    assert!(vxsat(&state));
}
