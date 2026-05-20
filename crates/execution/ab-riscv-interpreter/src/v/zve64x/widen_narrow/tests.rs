use crate::rv64::test_utils::{TestInterpreterState, initialize_state};
use crate::v::vector_registers::{VectorRegisters, VectorRegistersExt};
use crate::{
    ExecutableInstruction, ExecutableInstructionOperands, ExecutionError, RegisterFile,
    Rs1Rs2OperandValues, Rs1Rs2Operands,
};
use ab_riscv_primitives::prelude::*;

fn encode_vtype(vsew: Vsew, vlmul: Vlmul) -> u64 {
    u64::from(vlmul.to_bits()) | (u64::from(vsew.to_bits()) << 3)
}

fn setup(
    vl: u32,
    vsew: Vsew,
    vlmul: Vlmul,
) -> TestInterpreterState<Zve64xWidenNarrowInstruction<Reg<u64>>> {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    let vtype = Vtype::from_raw::<Reg<u64>>(encode_vtype(vsew, vlmul)).unwrap();
    state.ext_state.set_vtype(Some(vtype));
    state.ext_state.set_vl(vl);
    state.ext_state.set_vstart(0);
    state
}

fn exec(
    state: &mut TestInterpreterState<Zve64xWidenNarrowInstruction<Reg<u64>>>,
    instr: Zve64xWidenNarrowInstruction<Reg<u64>>,
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

fn read_elem(
    state: &TestInterpreterState<Zve64xWidenNarrowInstruction<Reg<u64>>>,
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

fn write_elem(
    state: &mut TestInterpreterState<Zve64xWidenNarrowInstruction<Reg<u64>>>,
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

fn write_mask(state: &mut TestInterpreterState<Zve64xWidenNarrowInstruction<Reg<u64>>>, bits: u32) {
    let reg = &mut state.ext_state.write_vreg()[0];
    reg.fill(0);
    for i in 0..32 {
        if (bits >> i) & 1 != 0 {
            reg[(i / u8::BITS) as usize] |= 1 << (i % u8::BITS);
        }
    }
}
// With TEST_VLEN=128, VLENB=16:
// Widening from SEW produces 2*SEW destination.
// E8/M1 -> narrow VLMAX=16; wide dest uses M2 (2 regs, E16)
// E16/M1 -> narrow VLMAX=8; wide dest uses M2 (2 regs, E32)
// E32/M1 -> narrow VLMAX=4; wide dest uses M2 (2 regs, E64)
// E8/M2 -> narrow VLMAX=32; wide dest uses M4 (4 regs, E16)
// Narrowing from 2*SEW to SEW:
// vl set to narrow VLMAX; vs2 is a wide group

// vwaddu.vv

#[test]
fn vwaddu_vv_e8_m1_zero_extends() {
    // Ensures both operands are zero-extended (0xff + 0x01 = 0x0100, not -1+1=0)
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0xff);
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 1);
    }
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwadduVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        assert_eq!(
            read_elem(&state, VReg::V8, i, Vsew::E16),
            0x0100u64,
            "elem {i}"
        );
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vwaddu_vv_e16_m1_basic() {
    let mut state = setup(4, Vsew::E16, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E16, 1000);
        write_elem(&mut state, VReg::V4, i, Vsew::E16, 2000);
    }
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwadduVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        assert_eq!(read_elem(&state, VReg::V8, i, Vsew::E32), 3000, "elem {i}");
    }
}

#[test]
fn vwaddu_vv_e32_m1_basic() {
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 0xffff_ffff);
    write_elem(&mut state, VReg::V4, 0, Vsew::E32, 1);
    write_elem(&mut state, VReg::V2, 1, Vsew::E32, 0xffff_ffff);
    write_elem(&mut state, VReg::V4, 1, Vsew::E32, 0xffff_ffff);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwadduVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E64), 0x1_0000_0000u64);
    assert_eq!(read_elem(&state, VReg::V8, 1, Vsew::E64), 0x1_ffff_fffe);
}

// vwaddu.vx

#[test]
fn vwaddu_vx_e8_m1_zero_extends_scalar() {
    // Scalar from rs1 is zero-extended as a u64; low SEW bits matter
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0x80);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0x01);
    // rs1 = 0x80 zero-extended to u64 = 0x80
    state.regs.write(Reg::A0, 0x80u64);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwadduVx {
            vd: VReg::V8,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 0x80 + 0x80 = 0x100 (both zero-extended)
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E16), 0x100u64);
    assert_eq!(read_elem(&state, VReg::V8, 1, Vsew::E16), 0x81u64);
}

#[test]
fn vwaddu_vx_e8_m1_scalar_not_truncated_to_sew() {
    // Per spec §11.2, the rs1 scalar is the full XLEN value zero-extended to 2*SEW
    // (NOT truncated to SEW). With rs1=0x1ff and SEW=8, the operand is 0x1ff, not 0xff.
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0x01);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0x00);
    state.regs.write(Reg::A0, 0x1ffu64);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwadduVx {
            vd: VReg::V8,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E16), 0x200u64);
    assert_eq!(read_elem(&state, VReg::V8, 1, Vsew::E16), 0x1ffu64);
}

// vwadd.vv

#[test]
fn vwadd_vv_e8_m1_sign_extends() {
    // Ensures both operands are sign-extended: (-1) + (-1) = -2, in 16-bit = 0xfffe
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xff);
    write_elem(&mut state, VReg::V4, 0, Vsew::E8, 0xff);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0x7f);
    write_elem(&mut state, VReg::V4, 1, Vsew::E8, 0x01);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwaddVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // -1 + -1 = -2 in signed 16-bit = 0xfffe
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E16), 0xfffeu64);
    // 127 + 1 = 128 = 0x0080
    assert_eq!(read_elem(&state, VReg::V8, 1, Vsew::E16), 0x0080u64);
}

#[test]
fn vwadd_vv_e32_m1_sign_extends() {
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    // -1 as i32 = 0xffff_ffff; sign-extended to 64-bit = 0xffff_ffff_ffff_ffff
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 0xffff_ffff);
    write_elem(&mut state, VReg::V4, 0, Vsew::E32, 1);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwaddVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // -1 + 1 = 0
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E64), 0u64);
}

// vwadd.vx

#[test]
fn vwadd_vx_e16_m1_sign_extends_scalar() {
    // rs1 is treated as sign-extended; a negative 64-bit scalar is the right treatment
    let mut state = setup(2, Vsew::E16, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E16, 0x8000);
    // scalar = -1 in 64 bits = 0xffff_ffff_ffff_ffff (sign-extended)
    state.regs.write(Reg::A1, u64::MAX);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwaddVx {
            vd: VReg::V8,
            vs2: VReg::V2,
            rs1: Reg::A1,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // sext(0x8000 as i16) = -32768 as i64 = 0xffff_ffff_ffff_8000
    // + scalar (-1) = 0xffff_ffff_ffff_7fff
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E32), 0xffff_7fffu64);
}

#[test]
fn vwadd_vx_e8_m1_scalar_sign_extended_from_xlen_not_sew() {
    // rs1=0x1ff as i64 is +511 (sign-extended from XLEN, not from SEW).
    // element 0x01 sext to i16 = 1; 1 + 511 = 512 = 0x0200.
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0x01);
    state.regs.write(Reg::A0, 0x1ffu64);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwaddVx {
            vd: VReg::V8,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E16), 0x0200u64);
}

#[test]
fn vwadd_vx_e8_m1_negative_xlen_scalar() {
    // rs1 = u64::MAX = -1 signed. 0x01 sext = 1. 1 + (-1) = 0.
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0x01);
    state.regs.write(Reg::A0, u64::MAX);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwaddVx {
            vd: VReg::V8,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E16), 0u64);
}

// vwsubu.vv

#[test]
fn vwsubu_vv_e8_m1_zero_extends() {
    // 1 - 2 unsigned at 8-bit zero-extended to 16: 0x0001 - 0x0002 = 0xffff (wraps)
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 1);
    write_elem(&mut state, VReg::V4, 0, Vsew::E8, 2);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwsubuVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E16), 0xffffu64);
}

// vwsub.vv

#[test]
fn vwsub_vv_e8_m1_sign_extends() {
    // -128 - (-127) = -1 as i16 = 0xffff
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    // 0x80 = -128 as i8; 0x81 = -127 as i8
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0x80);
    write_elem(&mut state, VReg::V4, 0, Vsew::E8, 0x81);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwsubVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E16), 0xffffu64);
}

#[test]
fn vwsub_vx_e32_m1_sign_extends() {
    // sext(0x8000_0000 as i32) = -2147483648i64; scalar = -1
    // result = -2147483648 - (-1) = -2147483647
    let mut state = setup(1, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 0x8000_0000);
    state.regs.write(Reg::A0, u64::MAX);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwsubVx {
            vd: VReg::V8,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // -2147483648 - (-1) = -2147483647 as u64 = 0xffff_ffff_8000_0001
    assert_eq!(
        read_elem(&state, VReg::V8, 0, Vsew::E64),
        0xffff_ffff_8000_0001u64
    );
}

// vwaddu.wv

#[test]
fn vwaddu_wv_e8_m1_wide_plus_narrow() {
    // vs2 holds 2*SEW=E16 values; vs1 holds SEW=E8 (zero-extended)
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    // vs2 is the wide group (uses 2 regs for M1 -> M2 dest)
    // vwaddu.wv: vd(E16,M2) = vs2(E16,M2) + zext(vs1(E8,M1))
    // Use V8 as wide vs2, V4 as narrow vs1, V0 group for vd but v0 conflicts with mask
    // Use V16 as vd(M2), V8 as vs2(M2), V4 as vs1(M1)
    for i in 0..4usize {
        write_elem(&mut state, VReg::V8, i, Vsew::E16, 1000);
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 0xff);
    }
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwadduWv {
            vd: VReg::V16,
            vs2: VReg::V8,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        // 1000 + 255 = 1255
        assert_eq!(read_elem(&state, VReg::V16, i, Vsew::E16), 1255, "elem {i}");
    }
}

#[test]
fn vwaddu_wx_e16_m1_wide_plus_scalar() {
    let mut state = setup(4, Vsew::E16, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V8, i, Vsew::E32, 0x1_0000u64);
    }
    state.regs.write(Reg::A0, 1u64);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwadduWx {
            vd: VReg::V16,
            vs2: VReg::V8,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        assert_eq!(
            read_elem(&state, VReg::V16, i, Vsew::E32),
            0x1_0001u64,
            "elem {i}"
        );
    }
}

#[test]
fn vwaddu_wx_e8_m1_scalar_not_truncated() {
    // Wide source = 0x200, scalar = 0x1ff (full XLEN value, zero-extended).
    // 0x200 + 0x1ff = 0x3ff.
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V8, 0, Vsew::E16, 0x200u64);
    state.regs.write(Reg::A0, 0x1ffu64);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwadduWx {
            vd: VReg::V16,
            vs2: VReg::V8,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V16, 0, Vsew::E16), 0x3ffu64);
}

// vwadd.wv / vwadd.wx

#[test]
fn vwadd_wv_e8_m1_sign_extends_narrow() {
    // vs1 narrow source is sign-extended; 0xff = -1 as i8
    // wide source = 0, result = 0 + (-1) = -1 as E16 = 0xffff
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    for i in 0..2usize {
        write_elem(&mut state, VReg::V8, i, Vsew::E16, 0u64);
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 0xff);
    }
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwaddWv {
            vd: VReg::V16,
            vs2: VReg::V8,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..2usize {
        assert_eq!(
            read_elem(&state, VReg::V16, i, Vsew::E16),
            0xffffu64,
            "elem {i}"
        );
    }
}

#[test]
fn vwadd_wx_e32_m1_sign_extends_scalar() {
    // scalar = -1 (u64::MAX), wide source = 0; result = -1 as E64 = 0xffff_ffff_ffff_ffff
    let mut state = setup(1, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V8, 0, Vsew::E64, 0u64);
    state.regs.write(Reg::A0, u64::MAX);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwaddWx {
            vd: VReg::V16,
            vs2: VReg::V8,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V16, 0, Vsew::E64), u64::MAX);
}

// vwsubu.wv / vwsubu.wx

#[test]
fn vwsubu_wv_e8_m1_zero_extends_narrow() {
    // wide source = 0x200, narrow vs1 = 0xff (zero-extended = 255)
    // result = 0x200 - 0xff = 0x101
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V8, 0, Vsew::E16, 0x200u64);
    write_elem(&mut state, VReg::V8, 1, Vsew::E16, 0x100u64);
    write_elem(&mut state, VReg::V4, 0, Vsew::E8, 0xff);
    write_elem(&mut state, VReg::V4, 1, Vsew::E8, 0x01);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwsubuWv {
            vd: VReg::V16,
            vs2: VReg::V8,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V16, 0, Vsew::E16), 0x101u64);
    assert_eq!(read_elem(&state, VReg::V16, 1, Vsew::E16), 0xffu64);
}

#[test]
fn vwsubu_wx_e16_m1_scalar() {
    let mut state = setup(2, Vsew::E16, Vlmul::M1);
    write_elem(&mut state, VReg::V8, 0, Vsew::E32, 0x1_0000u64);
    write_elem(&mut state, VReg::V8, 1, Vsew::E32, 5u64);
    state.regs.write(Reg::A0, 1u64);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwsubuWx {
            vd: VReg::V16,
            vs2: VReg::V8,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V16, 0, Vsew::E32), 0xffffu64);
    assert_eq!(read_elem(&state, VReg::V16, 1, Vsew::E32), 4u64);
}

// vwsub.wv / vwsub.wx

#[test]
fn vwsub_wv_e8_m1_sign_extends_narrow() {
    // wide = 0, narrow = 0x80 = -128 as i8; result = 0 - (-128) = 128 = 0x0080
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V8, 0, Vsew::E16, 0u64);
    write_elem(&mut state, VReg::V4, 0, Vsew::E8, 0x80);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwsubWv {
            vd: VReg::V16,
            vs2: VReg::V8,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V16, 0, Vsew::E16), 0x0080u64);
}

#[test]
fn vwsub_wx_e32_m1_sign_extends_scalar() {
    // wide = 0, scalar = -1 (u64::MAX); 0 - (-1) = 1
    let mut state = setup(1, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V8, 0, Vsew::E64, 0u64);
    state.regs.write(Reg::A0, u64::MAX);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwsubWx {
            vd: VReg::V16,
            vs2: VReg::V8,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V16, 0, Vsew::E64), 1u64);
}

// vnsrl.wv / vnsrl.wx / vnsrl.wi

#[test]
fn vnsrl_wv_e8_m1_logical_shift() {
    // Source is E16 (2*SEW=16, SEW=8); shift by 4 gives upper nibble in low byte
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4usize {
        // 0xab00 >> 4 = 0x0ab0; truncated to 8 bits = 0xb0
        write_elem(&mut state, VReg::V8, i, Vsew::E16, 0xab00u64);
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 4);
    }
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VnsrlWv {
            vd: VReg::V2,
            vs2: VReg::V8,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        assert_eq!(
            read_elem(&state, VReg::V2, i, Vsew::E8),
            0xb0u64,
            "elem {i}"
        );
    }
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vnsrl_wv_e16_m1_shamt_masked_to_5_bits() {
    // 2*SEW=32, shift amount mask = log2(32)-1 = 4 bits = 0x1f (5 bits: 0..31)
    // shift amount = 33 = 0b10_0001; masked to 5 bits = 1
    let mut state = setup(2, Vsew::E16, Vlmul::M1);
    write_elem(&mut state, VReg::V8, 0, Vsew::E32, 0xffff_ffffu64);
    write_elem(&mut state, VReg::V8, 1, Vsew::E32, 0x0002_0000u64);
    // shamt = 33; masked to log2(32)=5 bits = 33 & 31 = 1
    write_elem(&mut state, VReg::V4, 0, Vsew::E16, 33);
    write_elem(&mut state, VReg::V4, 1, Vsew::E16, 33);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VnsrlWv {
            vd: VReg::V2,
            vs2: VReg::V8,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 0xffff_ffff >> 1 = 0x7fff_ffff; truncated to 16 bits = 0xffff
    assert_eq!(read_elem(&state, VReg::V2, 0, Vsew::E16), 0xffffu64);
    // 0x0002_0000 >> 1 = 0x0001_0000; truncated to 16 bits = 0x0000
    assert_eq!(read_elem(&state, VReg::V2, 1, Vsew::E16), 0x0000u64);
}

#[test]
fn vnsrl_wx_e32_m1_logical_no_sign_fill() {
    // Source bit 63 set; logical shift must not sign-fill
    let mut state = setup(1, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V8, 0, Vsew::E64, 0x8000_0000_0000_0000u64);
    state.regs.write(Reg::A0, 1u64);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VnsrlWx {
            vd: VReg::V2,
            vs2: VReg::V8,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 0x8000_0000_0000_0000 >> 1 = 0x4000_0000_0000_0000; truncated to 32 bits = 0
    assert_eq!(read_elem(&state, VReg::V2, 0, Vsew::E32), 0u64);
}

#[test]
fn vnsrl_wi_e8_m1_immediate_shift() {
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V8, 0, Vsew::E16, 0xff00u64);
    write_elem(&mut state, VReg::V8, 1, Vsew::E16, 0x00ffu64);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VnsrlWi {
            vd: VReg::V2,
            vs2: VReg::V8,
            uimm: 8,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V2, 0, Vsew::E8), 0xffu64);
    assert_eq!(read_elem(&state, VReg::V2, 1, Vsew::E8), 0x00u64);
}

// vnsra.wv / vnsra.wx / vnsra.wi

#[test]
fn vnsra_wv_e8_m1_arithmetic_sign_fills() {
    // Source E16 = 0xff00 = -256 as i16 signed; >> 4 arithmetically = -16 as i16 = 0xfff0
    // truncated to 8 bits = 0xf0
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V8, 0, Vsew::E16, 0xff00u64);
    write_elem(&mut state, VReg::V8, 1, Vsew::E16, 0x7f00u64);
    write_elem(&mut state, VReg::V4, 0, Vsew::E8, 4);
    write_elem(&mut state, VReg::V4, 1, Vsew::E8, 4);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VnsraWv {
            vd: VReg::V2,
            vs2: VReg::V8,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // -256 >> 4 = -16; as u8 = 0xf0
    assert_eq!(read_elem(&state, VReg::V2, 0, Vsew::E8), 0xf0u64);
    // 0x7f00 = 32512; >> 4 = 2032 = 0x07f0; truncated to 8 = 0xf0
    assert_eq!(read_elem(&state, VReg::V2, 1, Vsew::E8), 0xf0u64);
}

#[test]
fn vnsra_wx_e32_m1_sign_fills() {
    // 0x8000_0000_0000_0000 as i64 = i64::MIN; >> 1 = 0xc000_0000_0000_0000
    // truncated to 32 bits = 0
    let mut state = setup(1, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V8, 0, Vsew::E64, 0x8000_0000_0000_0000u64);
    state.regs.write(Reg::A0, 1u64);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VnsraWx {
            vd: VReg::V2,
            vs2: VReg::V8,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V2, 0, Vsew::E32), 0u64);
}

#[test]
fn vnsra_wx_e16_m1_sign_fills_into_result() {
    // 0x8000_0000 as i32 = -2147483648; >> 16 = -32768 as i32 = 0xffff_8000
    // truncated to 16 bits = 0x8000
    let mut state = setup(2, Vsew::E16, Vlmul::M1);
    write_elem(&mut state, VReg::V8, 0, Vsew::E32, 0x8000_0000u64);
    write_elem(&mut state, VReg::V8, 1, Vsew::E32, 0x0001_0000u64);
    state.regs.write(Reg::A0, 16u64);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VnsraWx {
            vd: VReg::V2,
            vs2: VReg::V8,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V2, 0, Vsew::E16), 0x8000u64);
    assert_eq!(read_elem(&state, VReg::V2, 1, Vsew::E16), 1u64);
}

#[test]
fn vnsra_wi_e8_m1_immediate() {
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V8, 0, Vsew::E16, 0x8000u64);
    write_elem(&mut state, VReg::V8, 1, Vsew::E16, 0x0080u64);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VnsraWi {
            vd: VReg::V2,
            vs2: VReg::V8,
            uimm: 8,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // -32768 >> 8 = -128 as i8 = 0x80
    assert_eq!(read_elem(&state, VReg::V2, 0, Vsew::E8), 0x80u64);
    // 0x0080 = 128; >> 8 = 0; truncated to 8 = 0
    assert_eq!(read_elem(&state, VReg::V2, 1, Vsew::E8), 0x00u64);
}

// vzext.vf2

#[test]
fn vzext_vf2_e16_m1_zero_extends() {
    // SEW=16, source SEW/2=8; 0xff zero-extended to 16 = 0x00ff
    let mut state = setup(4, Vsew::E16, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 0xff);
    }
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VzextVf2 {
            vd: VReg::V8,
            vs2: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        assert_eq!(
            read_elem(&state, VReg::V8, i, Vsew::E16),
            0x00ffu64,
            "elem {i}"
        );
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vzext_vf2_e32_m1_zero_extends() {
    // SEW=32, source SEW/2=16; 0xffff zero-extended to 32 = 0x0000_ffff
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V4, 0, Vsew::E16, 0xffff);
    write_elem(&mut state, VReg::V4, 1, Vsew::E16, 0x1234);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VzextVf2 {
            vd: VReg::V8,
            vs2: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E32), 0x0000_ffffu64);
    assert_eq!(read_elem(&state, VReg::V8, 1, Vsew::E32), 0x1234u64);
}

#[test]
fn vzext_vf2_e64_m1_zero_extends() {
    let mut state = setup(2, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V4, 0, Vsew::E32, 0xffff_ffff);
    write_elem(&mut state, VReg::V4, 1, Vsew::E32, 0);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VzextVf2 {
            vd: VReg::V8,
            vs2: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(
        read_elem(&state, VReg::V8, 0, Vsew::E64),
        0x0000_0000_ffff_ffffu64
    );
    assert_eq!(read_elem(&state, VReg::V8, 1, Vsew::E64), 0u64);
}

// vzext.vf4

#[test]
fn vzext_vf4_e32_m1_zero_extends() {
    // SEW=32, source SEW/4=8; 0xff zero-extended to 32 = 0x0000_00ff
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 0xff);
    }
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VzextVf4 {
            vd: VReg::V8,
            vs2: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        assert_eq!(
            read_elem(&state, VReg::V8, i, Vsew::E32),
            0x0000_00ffu64,
            "elem {i}"
        );
    }
}

#[test]
fn vzext_vf4_e64_m1_zero_extends() {
    // SEW=64, source SEW/4=16; 0xffff zero-extended to 64
    let mut state = setup(2, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V4, 0, Vsew::E16, 0xffff);
    write_elem(&mut state, VReg::V4, 1, Vsew::E16, 0x0001);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VzextVf4 {
            vd: VReg::V8,
            vs2: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E64), 0x0000_ffffu64);
    assert_eq!(read_elem(&state, VReg::V8, 1, Vsew::E64), 1u64);
}

// vzext.vf8

#[test]
fn vzext_vf8_e64_m1_zero_extends() {
    // SEW=64, source SEW/8=8; only legal when SEW=64 in Zve64x
    let mut state = setup(2, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V4, 0, Vsew::E8, 0xff);
    write_elem(&mut state, VReg::V4, 1, Vsew::E8, 0x42);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VzextVf8 {
            vd: VReg::V8,
            vs2: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E64), 0xffu64);
    assert_eq!(read_elem(&state, VReg::V8, 1, Vsew::E64), 0x42u64);
}

// vsext.vf2

#[test]
fn vsext_vf2_e16_m1_sign_extends() {
    // 0xff = -1 as i8; sign-extended to 16 = 0xffff
    let mut state = setup(4, Vsew::E16, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 0xff);
    }
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VsextVf2 {
            vd: VReg::V8,
            vs2: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        assert_eq!(
            read_elem(&state, VReg::V8, i, Vsew::E16),
            0xffffu64,
            "elem {i}"
        );
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vsext_vf2_e32_m1_positive_unchanged() {
    // 0x7f = 127 as i8; sign-extended to 32 = 0x0000_007f
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V4, 0, Vsew::E16, 0x7fff);
    write_elem(&mut state, VReg::V4, 1, Vsew::E16, 0x8000);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VsextVf2 {
            vd: VReg::V8,
            vs2: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 0x7fff = 32767; sign-extended to 32 = 0x0000_7fff
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E32), 0x0000_7fffu64);
    // 0x8000 = -32768 as i16; sign-extended to 32 = 0xffff_8000
    assert_eq!(read_elem(&state, VReg::V8, 1, Vsew::E32), 0xffff_8000u64);
}

#[test]
fn vsext_vf2_e64_m1_sign_extends() {
    let mut state = setup(2, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V4, 0, Vsew::E32, 0x8000_0000);
    write_elem(&mut state, VReg::V4, 1, Vsew::E32, 0x7fff_ffff);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VsextVf2 {
            vd: VReg::V8,
            vs2: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(
        read_elem(&state, VReg::V8, 0, Vsew::E64),
        0xffff_ffff_8000_0000u64
    );
    assert_eq!(
        read_elem(&state, VReg::V8, 1, Vsew::E64),
        0x0000_0000_7fff_ffffu64
    );
}

// vsext.vf4

#[test]
fn vsext_vf4_e32_m1_sign_extends() {
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V4, 0, Vsew::E8, 0x80);
    write_elem(&mut state, VReg::V4, 1, Vsew::E8, 0x7f);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VsextVf4 {
            vd: VReg::V8,
            vs2: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E32), 0xffff_ff80u64);
    assert_eq!(read_elem(&state, VReg::V8, 1, Vsew::E32), 0x0000_007fu64);
}

#[test]
fn vsext_vf4_e64_m1_sign_extends() {
    let mut state = setup(2, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V4, 0, Vsew::E16, 0x8000);
    write_elem(&mut state, VReg::V4, 1, Vsew::E16, 0x0001);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VsextVf4 {
            vd: VReg::V8,
            vs2: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(
        read_elem(&state, VReg::V8, 0, Vsew::E64),
        0xffff_ffff_ffff_8000u64
    );
    assert_eq!(read_elem(&state, VReg::V8, 1, Vsew::E64), 1u64);
}

// vsext.vf8

#[test]
fn vsext_vf8_e64_m1_sign_extends() {
    let mut state = setup(2, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V4, 0, Vsew::E8, 0x80);
    write_elem(&mut state, VReg::V4, 1, Vsew::E8, 0x42);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VsextVf8 {
            vd: VReg::V8,
            vs2: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(
        read_elem(&state, VReg::V8, 0, Vsew::E64),
        0xffff_ffff_ffff_ff80u64
    );
    assert_eq!(read_elem(&state, VReg::V8, 1, Vsew::E64), 0x42u64);
}

// Masking

#[test]
fn vwaddu_vv_e8_m1_masked_skips_inactive() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    // Mask: elements 0 and 2 active, 1 and 3 inactive
    write_mask(&mut state, 0b0101);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 10);
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 20);
        // Pre-fill destination with sentinel 0xdead so inactive elements are undisturbed
        write_elem(&mut state, VReg::V8, i, Vsew::E16, 0xdeadu64);
    }
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwadduVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E16), 30u64);
    assert_eq!(read_elem(&state, VReg::V8, 1, Vsew::E16), 0xdeadu64);
    assert_eq!(read_elem(&state, VReg::V8, 2, Vsew::E16), 30u64);
    assert_eq!(read_elem(&state, VReg::V8, 3, Vsew::E16), 0xdeadu64);
}

#[test]
fn vnsrl_wv_e8_m1_masked_skips_inactive() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    // Mask: only element 1 active
    write_mask(&mut state, 0b0010);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V8, i, Vsew::E16, 0xff00u64);
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 8);
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0xabu64);
    }
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VnsrlWv {
            vd: VReg::V2,
            vs2: VReg::V8,
            vs1: VReg::V4,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // Element 0 undisturbed
    assert_eq!(read_elem(&state, VReg::V2, 0, Vsew::E8), 0xabu64);
    // Element 1 active: 0xff00 >> 8 = 0xff; truncated to 8 = 0xff
    assert_eq!(read_elem(&state, VReg::V2, 1, Vsew::E8), 0xffu64);
    // Elements 2, 3 undisturbed
    assert_eq!(read_elem(&state, VReg::V2, 2, Vsew::E8), 0xabu64);
    assert_eq!(read_elem(&state, VReg::V2, 3, Vsew::E8), 0xabu64);
}

#[test]
fn vsext_vf2_e16_m1_masked_skips_inactive() {
    let mut state = setup(4, Vsew::E16, Vlmul::M1);
    write_mask(&mut state, 0b1001);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 0x80);
        write_elem(&mut state, VReg::V8, i, Vsew::E16, 0x1234u64);
    }
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VsextVf2 {
            vd: VReg::V8,
            vs2: VReg::V4,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // Active: 0 and 3; -128 sign-extended to 16 = 0xff80
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E16), 0xff80u64);
    assert_eq!(read_elem(&state, VReg::V8, 1, Vsew::E16), 0x1234u64);
    assert_eq!(read_elem(&state, VReg::V8, 2, Vsew::E16), 0x1234u64);
    assert_eq!(read_elem(&state, VReg::V8, 3, Vsew::E16), 0xff80u64);
}

// vstart

#[test]
fn vwaddu_vv_e8_m1_vstart_skips_early_elements() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 1);
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 2);
        write_elem(&mut state, VReg::V8, i, Vsew::E16, 0xdead);
    }
    state.ext_state.set_vstart(2);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwadduVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // Elements 0 and 1 skipped (vstart=2), remain sentinel
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E16), 0xdeadu64);
    assert_eq!(read_elem(&state, VReg::V8, 1, Vsew::E16), 0xdeadu64);
    // Elements 2 and 3 executed
    assert_eq!(read_elem(&state, VReg::V8, 2, Vsew::E16), 3u64);
    assert_eq!(read_elem(&state, VReg::V8, 3, Vsew::E16), 3u64);
    // vstart must be reset to 0 after execution
    assert_eq!(state.ext_state.vstart(), 0);
}

// Illegal instruction: SEW=64 for widening

#[test]
fn vwaddu_vv_e64_m1_illegal() {
    // Widening from SEW=64 would require 128-bit elements, illegal in Zve64x (ELEN=64)
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwadduVv {
            vd: VReg::V8,
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
fn vwadd_vv_e64_m1_illegal() {
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwaddVv {
            vd: VReg::V8,
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
fn vwsubu_vv_e64_m1_illegal() {
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwsubuVv {
            vd: VReg::V8,
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
fn vnsrl_e64_m1_illegal() {
    // Narrowing from 2*SEW=128 not supported in Zve64x
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VnsrlWi {
            vd: VReg::V2,
            vs2: VReg::V8,
            uimm: 0,
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
fn vnsra_e64_m1_illegal() {
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VnsraWi {
            vd: VReg::V2,
            vs2: VReg::V8,
            uimm: 0,
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

// Illegal: SEW too small for extension factor

#[test]
fn vzext_vf4_e16_illegal_sew_too_small() {
    // SEW=16, factor=4 -> source would be 4-bit which is < 8, illegal
    let mut state = setup(4, Vsew::E16, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VzextVf4 {
            vd: VReg::V8,
            vs2: VReg::V4,
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
fn vsext_vf4_e16_illegal_sew_too_small() {
    let mut state = setup(4, Vsew::E16, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VsextVf4 {
            vd: VReg::V8,
            vs2: VReg::V4,
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
fn vzext_vf8_e32_illegal_sew_too_small() {
    // SEW=32, factor=8 -> source would be 4-bit, illegal
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VzextVf8 {
            vd: VReg::V8,
            vs2: VReg::V4,
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
fn vsext_vf8_e32_illegal_sew_too_small() {
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VsextVf8 {
            vd: VReg::V8,
            vs2: VReg::V4,
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
fn vzext_vf2_e8_illegal_sew_too_small() {
    // SEW=8, factor=2 -> source would be 4-bit, illegal
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VzextVf2 {
            vd: VReg::V8,
            vs2: VReg::V4,
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

// Illegal: vm=false and vd=v0

#[test]
fn vwaddu_vv_masked_vd_v0_illegal() {
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    // vd=V0 with vm=false is always illegal (vd overlaps mask register)
    let result = exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwadduVv {
            vd: VReg::V0,
            vs2: VReg::V4,
            vs1: VReg::V8,
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
fn vnsrl_masked_vd_v0_illegal() {
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VnsrlWi {
            vd: VReg::V0,
            vs2: VReg::V4,
            uimm: 0,
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
fn vzext_vf2_masked_vd_v0_illegal() {
    let mut state = setup(2, Vsew::E16, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VzextVf2 {
            vd: VReg::V0,
            vs2: VReg::V4,
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

// Illegal: vtype not set (vill)

#[test]
fn vwaddu_vv_vtype_not_set_illegal() {
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    // Explicitly invalidate vtype
    state.ext_state.set_vtype(None);
    let result = exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwadduVv {
            vd: VReg::V8,
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
fn vnsrl_vtype_not_set_illegal() {
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    state.ext_state.set_vtype(None);
    let result = exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VnsrlWi {
            vd: VReg::V2,
            vs2: VReg::V8,
            uimm: 0,
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

// Illegal: vector instructions not allowed

#[test]
fn vwaddu_vv_not_allowed_illegal() {
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    state.ext_state.set_vector_allowed(false);
    let result = exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwadduVv {
            vd: VReg::V8,
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
fn vsext_vf2_not_allowed_illegal() {
    let mut state = setup(2, Vsew::E16, Vlmul::M1);
    state.ext_state.set_vector_allowed(false);
    let result = exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VsextVf2 {
            vd: VReg::V8,
            vs2: VReg::V4,
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

// Illegal: vd alignment / overlap

#[test]
fn vwaddu_vv_vd_misaligned_illegal() {
    // vd must be aligned to 2*group_regs; M1 -> wide group=2; V1 not aligned to 2
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwadduVv {
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
fn vwaddu_vv_vd_overlaps_vs2_illegal() {
    // vd(V2,wide=2 regs: V2,V3) overlaps vs2(V2)
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwadduVv {
            vd: VReg::V2,
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
fn vwaddu_vv_vd_overlaps_vs1_illegal() {
    // vd(V4,wide=2 regs: V4,V5) overlaps vs1(V4)
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwadduVv {
            vd: VReg::V4,
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
fn vzext_vf2_vd_overlaps_vs2_illegal() {
    // E16/M1: vd group=1 reg, vs2 src_group=1 reg; V8 overlaps V8
    let mut state = setup(4, Vsew::E16, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VzextVf2 {
            vd: VReg::V8,
            vs2: VReg::V8,
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

// LMUL>1: multi-register groups

#[test]
fn vwaddu_vv_e8_m2_multi_register_group() {
    // E8/M2: vl=8, narrow group=2 regs (V8,V9); wide dest=M4 (V16,V17,V18,V19)
    let mut state = setup(8, Vsew::E8, Vlmul::M2);
    for i in 0..8usize {
        write_elem(&mut state, VReg::V8, i, Vsew::E8, 100u64);
        write_elem(&mut state, VReg::V12, i, Vsew::E8, 200u64);
    }
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwadduVv {
            vd: VReg::V16,
            vs2: VReg::V8,
            vs1: VReg::V12,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..8usize {
        assert_eq!(
            read_elem(&state, VReg::V16, i, Vsew::E16),
            300u64,
            "elem {i}"
        );
    }
}

#[test]
fn vnsrl_e16_m2_multi_register_group() {
    // E16/M2: vl=8, narrow dest=M2 (V8,V9); wide source=M4 (V16,V17,V18,V19)
    let mut state = setup(8, Vsew::E16, Vlmul::M2);
    for i in 0..8usize {
        write_elem(&mut state, VReg::V16, i, Vsew::E32, 0xffff_0000u64);
    }
    state.regs.write(Reg::A0, 16u64);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VnsrlWx {
            vd: VReg::V8,
            vs2: VReg::V16,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..8usize {
        // 0xffff_0000 >> 16 = 0xffff; truncated to 16 = 0xffff
        assert_eq!(
            read_elem(&state, VReg::V8, i, Vsew::E16),
            0xffffu64,
            "elem {i}"
        );
    }
}

// vl=0: no-op

#[test]
fn vwaddu_vv_vl_zero_nop() {
    let mut state = setup(0, Vsew::E8, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V8, i, Vsew::E16, 0xdeadu64);
    }
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwadduVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // No elements written; destination unchanged
    for i in 0..4usize {
        assert_eq!(
            read_elem(&state, VReg::V8, i, Vsew::E16),
            0xdeadu64,
            "elem {i}"
        );
    }
    // vs_dirty and vstart still updated (instruction did execute)
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

// vnsrl shamt mask boundary

#[test]
fn vnsrl_wi_e8_m1_uimm_masked_to_log2_2sew() {
    // SEW=8; 2*SEW=16; log2(16)=4 bits; uimm=16=0b1_0000; masked to 4 bits = 0
    // shift by 0: result = full 16-bit value truncated to 8
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V8, 0, Vsew::E16, 0xab_cdu64);
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VnsrlWi {
            vd: VReg::V2,
            vs2: VReg::V8,
            // 16 in 5-bit uimm field; masked to 4 bits = 0
            uimm: 16,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // shift by 0 -> 0xabcd; truncated to 8 = 0xcd
    assert_eq!(read_elem(&state, VReg::V2, 0, Vsew::E8), 0xcdu64);
}

// vwaddu.wv vd may alias vs2

#[test]
fn vwaddu_wv_vd_aliases_vs2_legal() {
    // For .wv/.wx variants vd and vs2 occupy the same wide group; aliasing is allowed
    // vd=V8(M2), vs2=V8(M2), vs1=V4(M1)
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V8, i, Vsew::E16, 1000u64);
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 1u64);
    }
    exec(
        &mut state,
        Zve64xWidenNarrowInstruction::VwadduWv {
            vd: VReg::V8,
            vs2: VReg::V8,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        assert_eq!(
            read_elem(&state, VReg::V8, i, Vsew::E16),
            1001u64,
            "elem {i}"
        );
    }
}
