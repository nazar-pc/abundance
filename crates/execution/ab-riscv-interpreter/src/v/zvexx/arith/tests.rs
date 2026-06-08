use crate::rv64::test_utils::{TestInterpreterState, initialize_state};
use crate::v::vector_registers::{VectorRegisters, VectorRegistersExt};
use crate::{
    ExecutableInstruction, ExecutableInstructionOperands, ExecutionError, RegisterFile,
    Rs1Rs2OperandValues, Rs1Rs2Operands,
};
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

/// Encode a raw vtype value (vta=false, vma=false)
fn encode_vtype(vsew: Vsew, vlmul: Vlmul) -> u64 {
    u64::from(vlmul.to_bits()) | (u64::from(vsew.to_bits()) << 3u8)
}

/// Build a fresh state with vector CSRs initialized and vtype/vl configured
fn setup(
    vl: u32,
    vsew: Vsew,
    vlmul: Vlmul,
) -> TestInterpreterState<ZveXxArithInstruction<Reg<u64>>> {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    let vtype = Vtype::from_raw::<Reg<u64>>(encode_vtype(vsew, vlmul)).unwrap();
    state.ext_state.set_vtype(Some(vtype));
    state.ext_state.set_vl(vl);
    state.ext_state.set_vstart(0);
    state
}

/// Execute a single instruction directly
fn exec(
    state: &mut TestInterpreterState<ZveXxArithInstruction<Reg<u64>>>,
    instr: ZveXxArithInstruction<Reg<u64>>,
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

/// Write bytes into a vector register
fn set_vreg(
    state: &mut TestInterpreterState<ZveXxArithInstruction<Reg<u64>>>,
    reg: VReg,
    data: &[u8],
) {
    let dst = state.ext_state.write_vregs().get_mut(reg);
    dst.fill(0);
    dst[..data.len()].copy_from_slice(data);
}

/// Read a full vector register as bytes
fn get_vreg(state: &TestInterpreterState<ZveXxArithInstruction<Reg<u64>>>, reg: VReg) -> [u8; 32] {
    *state.ext_state.read_vregs().get(reg)
}

/// Read element `i` from a register group as a u64 (zero-extended), given SEW
fn read_elem(
    state: &TestInterpreterState<ZveXxArithInstruction<Reg<u64>>>,
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

/// Write element `i` into a register group, given SEW
fn write_elem(
    state: &mut TestInterpreterState<ZveXxArithInstruction<Reg<u64>>>,
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

/// Read mask bit `i` from an arbitrary vector register
fn mask_bit(
    state: &TestInterpreterState<ZveXxArithInstruction<Reg<u64>>>,
    reg: VReg,
    i: u32,
) -> bool {
    let byte = state.ext_state.read_vregs().get(reg)[(i / u8::BITS) as usize];
    (byte >> (i % u8::BITS)) & 1 != 0
}

// With TEST_VLEN=256, VLENB=32:
//   E8/M1  -> VLMAX=32, 1 reg,  32 elems/reg
//   E16/M1 -> VLMAX=16, 1 reg,  16 elems/reg
//   E32/M1 -> VLMAX=8,  1 reg,  8 elems/reg
//   E64/M1 -> VLMAX=4,  1 reg,  4 elems/reg
//   E8/M2  -> VLMAX=64, 2 regs, 32 elems/reg
//   E32/M2 -> VLMAX=16, 2 regs, 8 elems/reg
//   E64/Mf2-> VLMAX=2,  1 reg,  4 elems/reg

// vadd

#[test]
fn vadd_vv_e8_m1_basic() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    // vs2 = [1, 2, 3, 4, ...], vs1 = [10, 20, 30, 40, ...]
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, (i + 1) as u64);
        write_elem(&mut state, VReg::V1, i, Vsew::E8, ((i + 1) * 10) as u64);
    }
    exec(
        &mut state,
        ZveXxArithInstruction::VaddVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E8),
            ((i + 1) * 11) as u64,
            "elem {i}"
        );
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vadd_vv_e64_m1_wraps() {
    let mut state = setup(2, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E64, u64::MAX);
    write_elem(&mut state, VReg::V1, 0, Vsew::E64, 1);
    exec(
        &mut state,
        ZveXxArithInstruction::VaddVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E64), 0);
}

#[test]
fn vadd_vx_e32_m1() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, i as u64);
    }
    state.regs.write(Reg::A0, 100);
    exec(
        &mut state,
        ZveXxArithInstruction::VaddVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E32),
            i as u64 + 100,
            "elem {i}"
        );
    }
}

#[test]
fn vadd_vi_e16_m1_negative_imm() {
    let mut state = setup(4, Vsew::E16, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E16, 10);
    }
    // imm = -1 sign-extended: wrapping_add(-1 as u64) = wrapping sub 1
    exec(
        &mut state,
        ZveXxArithInstruction::VaddVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            imm: -1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E16), 9, "elem {i}");
    }
}

#[test]
fn vadd_vv_e8_m2_spans_two_regs() {
    // VLMAX=64: elements 0..31 in v2, 32..63 in v3 (32 E8 elems per VLENB=32 register)
    let mut state = setup(64, Vsew::E8, Vlmul::M2);
    for i in 0..64usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, i as u64);
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 1);
    }
    exec(
        &mut state,
        ZveXxArithInstruction::VaddVv {
            vd: VReg::V6,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..64usize {
        assert_eq!(
            read_elem(&state, VReg::V6, i, Vsew::E8),
            i as u64 + 1,
            "elem {i}"
        );
    }
}

// vsub / vrsub

#[test]
fn vsub_vv_e8_m1() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, (i + 10) as u64);
        write_elem(&mut state, VReg::V1, i, Vsew::E8, i as u64);
    }
    exec(
        &mut state,
        ZveXxArithInstruction::VsubVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 10, "elem {i}");
    }
}

#[test]
fn vsub_vx_e32_wraps() {
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 0);
    state.regs.write(Reg::A0, 1);
    exec(
        &mut state,
        ZveXxArithInstruction::VsubVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 0u32 wrapping_sub 1 = 0xFFFFFFFF, zero-extended to u64 = 0xFFFFFFFF
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0xFFFF_FFFF);
}

#[test]
fn vrsub_vx_e8_m1() {
    // vrsub: vd[i] = rs1 - vs2[i]
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, i as u64);
    }
    state.regs.write(Reg::A0, 10);
    exec(
        &mut state,
        ZveXxArithInstruction::VrsubVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E8),
            (10 - i) as u64,
            "elem {i}"
        );
    }
}

#[test]
fn vrsub_vi_e16_m1() {
    let mut state = setup(4, Vsew::E16, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E16, i as u64);
    }
    exec(
        &mut state,
        ZveXxArithInstruction::VrsubVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            imm: 5,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        // 5 - i; for i > 5 this wraps mod 2^16
        let expected = (5i64 - (i as u64).cast_signed()).rem_euclid(1 << 16u8) as u64;
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E16),
            expected,
            "elem {i}"
        );
    }
}

// vand / vor / vxor

#[test]
fn vand_vv_e32_m1() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, 0xFF00_FF00);
        write_elem(&mut state, VReg::V1, i, Vsew::E32, 0xF0F0_F0F0);
    }
    exec(
        &mut state,
        ZveXxArithInstruction::VandVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E32),
            0xF000_F000,
            "elem {i}"
        );
    }
}

#[test]
fn vand_vi_sign_extends_imm() {
    // imm = -1 (0b11111 sign-extended) should AND as all-ones within SEW
    let mut state = setup(2, Vsew::E16, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E16, 0xABCD);
    write_elem(&mut state, VReg::V2, 1, Vsew::E16, 0x1234);
    exec(
        &mut state,
        ZveXxArithInstruction::VandVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            imm: -1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // AND with 0xFFFF...FF leaves SEW-wide bits unchanged
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E16), 0xABCD);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E16), 0x1234);
}

#[test]
fn vor_vx_e64_m1() {
    let mut state = setup(2, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E64, 0x0F0F_0F0F_0F0F_0F0F);
    write_elem(&mut state, VReg::V2, 1, Vsew::E64, 0);
    state.regs.write(Reg::A0, 0xF0F0_F0F0_F0F0_F0F0_u64);
    exec(
        &mut state,
        ZveXxArithInstruction::VorVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E64), u64::MAX);
    assert_eq!(
        read_elem(&state, VReg::V4, 1, Vsew::E64),
        0xF0F0_F0F0_F0F0_F0F0_u64
    );
}

#[test]
fn vxor_vi_e8_m1() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0xAA);
    }
    exec(
        &mut state,
        ZveXxArithInstruction::VxorVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            imm: -1, // 0xFF within E8
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        // 0xAA ^ 0xFF = 0x55
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0x55, "elem {i}");
    }
}

// vsll / vsrl / vsra

#[test]
fn vsll_vv_e8_masks_shamt_to_3_bits() {
    // SEW=8: shift amount is masked to 3 bits (log2(8)=3), so shamt 9 = shamt 1
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0x01);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0xFF);
    // vs1[0] = 9 -> effective shamt = 9 & 7 = 1
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 9);
    // 8 & 7 = 0
    write_elem(&mut state, VReg::V1, 1, Vsew::E8, 8);
    exec(
        &mut state,
        ZveXxArithInstruction::VsllVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 1 << 1
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 0x02);
    // 0xFF << 0 = 0xFF (truncated to u8)
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E8), 0xFF);
}

#[test]
fn vsll_vi_e16_m1() {
    let mut state = setup(4, Vsew::E16, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E16, 1);
    }
    exec(
        &mut state,
        ZveXxArithInstruction::VsllVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            uimm: 4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E16),
            1 << 4u8,
            "elem {i}"
        );
    }
}

#[test]
fn vsrl_vv_e32_logical_shift() {
    // vsrl is logical: high bit should not be sign-extended
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 0x8000_0000);
    write_elem(&mut state, VReg::V1, 0, Vsew::E32, 1);
    exec(
        &mut state,
        ZveXxArithInstruction::VsrlVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0x4000_0000);
}

#[test]
fn vsrl_vx_e8_does_not_bleed_upper_bits() {
    // Register holds 0xAB in the e8 slot; upper bits of u64 representation must not affect result
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    // Manually place 0xAB in the byte slot for element 0
    state.ext_state.write_vregs().get_mut(VReg::V2)[0] = 0xAB;
    state.regs.write(Reg::A0, 1);
    exec(
        &mut state,
        ZveXxArithInstruction::VsrlVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 0xAB >> 1 = 0x55 (logical, no sign extension)
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 0x55);
}

#[test]
fn vsra_vv_e8_arithmetic_shift() {
    // 0x80 as signed i8 = -128; >> 1 = -64 = 0xC0 (arithmetic)
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    state.ext_state.write_vregs().get_mut(VReg::V2)[0] = 0x80;
    state.ext_state.write_vregs().get_mut(VReg::V2)[1] = 0x40;
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 1);
    write_elem(&mut state, VReg::V1, 1, Vsew::E8, 1);
    exec(
        &mut state,
        ZveXxArithInstruction::VsraVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // -64 as u8
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 0xC0);
    // 32
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E8), 0x20);
}

#[test]
fn vsra_vi_e32_m1() {
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    // -2147483648 as i32
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 0x8000_0000);
    // 16
    write_elem(&mut state, VReg::V2, 1, Vsew::E32, 0x0000_0010);
    exec(
        &mut state,
        ZveXxArithInstruction::VsraVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            uimm: 4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // -2147483648 >> 4 = -134217728 = 0xF800_0000
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0xF800_0000);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E32), 1);
}

#[test]
fn vsra_vx_e64_m1() {
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    write_elem(
        &mut state,
        VReg::V2,
        0,
        Vsew::E64,
        0x8000_0000_0000_0000_u64,
    );
    state.regs.write(Reg::A0, 63);
    exec(
        &mut state,
        ZveXxArithInstruction::VsraVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E64), u64::MAX);
}

// vminu / vmin / vmaxu / vmax

#[test]
fn vminu_vv_e8_unsigned() {
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    // 0xFF as unsigned is 255; should beat 1
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xFF);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0x01);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 0x01);
    write_elem(&mut state, VReg::V1, 1, Vsew::E8, 0xFF);
    exec(
        &mut state,
        ZveXxArithInstruction::VminuVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 0x01);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E8), 0x01);
}

#[test]
fn vmin_vv_e8_signed() {
    // 0xFF = -1 as i8, should be less than 1
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    // -1
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xFF);
    // 1
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0x01);
    // 1
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 0x01);
    // -1
    write_elem(&mut state, VReg::V1, 1, Vsew::E8, 0xFF);
    exec(
        &mut state,
        ZveXxArithInstruction::VminVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // -1 < 1
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 0xFF);
    // -1 < 1
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E8), 0xFF);
}

#[test]
fn vmaxu_vx_e32() {
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    // max unsigned
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 0xFFFF_FFFF);
    write_elem(&mut state, VReg::V2, 1, Vsew::E32, 5);
    state.regs.write(Reg::A0, 10);
    exec(
        &mut state,
        ZveXxArithInstruction::VmaxuVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0xFFFF_FFFF);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E32), 10);
}

#[test]
fn vmax_vx_e16_signed() {
    // 0xFFFF = -1 as i16; max(-1, 0) = 0
    let mut state = setup(2, Vsew::E16, Vlmul::M1);
    // -1 signed
    write_elem(&mut state, VReg::V2, 0, Vsew::E16, 0xFFFF);
    write_elem(&mut state, VReg::V2, 1, Vsew::E16, 5);
    // 0
    state.regs.write(Reg::A0, 0);
    exec(
        &mut state,
        ZveXxArithInstruction::VmaxVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // max(-1, 0) = 0
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E16), 0);
    // max(5, 0) = 5
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E16), 5);
}

// Compare instructions

#[test]
fn vmseq_vv_e8_m1_writes_mask_bits() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    // vs2[i] == vs1[i] for even i
    for i in 0..8usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, i as u64);
        write_elem(
            &mut state,
            VReg::V1,
            i,
            Vsew::E8,
            if i % 2 == 0 { i as u64 } else { 99 },
        );
    }
    // Pre-fill vd (v4) with all-ones so we can detect undisturbed bits above vl
    set_vreg(&mut state, VReg::V4, &[0xFF; 32]);
    exec(
        &mut state,
        ZveXxArithInstruction::VmseqVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // Elements 0,2,4,6 equal -> bits 0,2,4,6 set; bits 1,3,5,7 clear
    assert!(mask_bit(&state, VReg::V4, 0));
    assert!(!mask_bit(&state, VReg::V4, 1));
    assert!(mask_bit(&state, VReg::V4, 2));
    assert!(!mask_bit(&state, VReg::V4, 3));
    assert!(mask_bit(&state, VReg::V4, 4));
    assert!(!mask_bit(&state, VReg::V4, 5));
    assert!(mask_bit(&state, VReg::V4, 6));
    assert!(!mask_bit(&state, VReg::V4, 7));
    // Bits above vl (8..VLEN) must remain undisturbed (all-ones from pre-fill)
    for i in 8..256usize {
        assert_eq!(
            (state.ext_state.read_vregs().get(VReg::V4)[i / u8::BITS as usize]
                >> (i % u8::BITS as usize))
                & 1,
            1,
            "tail bit {i} was disturbed"
        );
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vmseq_vx_e32_m1() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(
            &mut state,
            VReg::V2,
            i,
            Vsew::E32,
            if i == 2 { 42 } else { i as u64 },
        );
    }
    state.regs.write(Reg::A0, 42);
    exec(
        &mut state,
        ZveXxArithInstruction::VmseqVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    let vd = get_vreg(&state, VReg::V4);
    // only bit 2 set
    assert_eq!(vd[0] & 0x0F, 0b0100);
}

#[test]
fn vmseq_vi_e16() {
    let mut state = setup(4, Vsew::E16, Vlmul::M1);
    for i in 0..4usize {
        write_elem(
            &mut state,
            VReg::V2,
            i,
            Vsew::E16,
            if i == 1 { 3 } else { 0 },
        );
    }
    exec(
        &mut state,
        ZveXxArithInstruction::VmseqVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            imm: 3,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    let vd = get_vreg(&state, VReg::V4);
    assert_eq!(vd[0] & 0x0F, 0b0010);
}

#[test]
fn vmsne_vv_e8() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, i as u64);
        write_elem(
            &mut state,
            VReg::V1,
            i,
            Vsew::E8,
            if i == 1 { 99 } else { i as u64 },
        );
    }
    exec(
        &mut state,
        ZveXxArithInstruction::VmsneVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    let vd = get_vreg(&state, VReg::V4);
    // only element 1 differs
    assert_eq!(vd[0] & 0x0F, 0b0010);
}

#[test]
fn vmsltu_vv_e8_unsigned() {
    // 0xFF (255u) is NOT < 0x01; 0x01 IS < 0xFF
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xFF);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0x01);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 0x01);
    write_elem(&mut state, VReg::V1, 1, Vsew::E8, 0xFF);
    exec(
        &mut state,
        ZveXxArithInstruction::VmsltuVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    let vd = get_vreg(&state, VReg::V4);
    // only bit 1
    assert_eq!(vd[0] & 0x03, 0b10);
}

#[test]
fn vmslt_vv_e8_signed() {
    // 0xFF (-1) IS signed < 0x01 (1); 0x01 is NOT < 0xFF (-1)
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    // -1
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xFF);
    // 1
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0x01);
    // 1
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 0x01);
    // -1
    write_elem(&mut state, VReg::V1, 1, Vsew::E8, 0xFF);
    exec(
        &mut state,
        ZveXxArithInstruction::VmsltVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    let vd = get_vreg(&state, VReg::V4);
    // only bit 0: -1 < 1
    assert_eq!(vd[0] & 0x03, 0b01);
}

#[test]
fn vmsleu_vv_e16() {
    let mut state = setup(3, Vsew::E16, Vlmul::M1);
    // vs2[0]=5 <= vs1[0]=5 (equal), vs2[1]=6 <= vs1[1]=10, vs2[2]=0xFFFF <= vs1[2]=0 (false
    // unsigned)
    write_elem(&mut state, VReg::V2, 0, Vsew::E16, 5);
    write_elem(&mut state, VReg::V2, 1, Vsew::E16, 6);
    write_elem(&mut state, VReg::V2, 2, Vsew::E16, 0xFFFF);
    write_elem(&mut state, VReg::V1, 0, Vsew::E16, 5);
    write_elem(&mut state, VReg::V1, 1, Vsew::E16, 10);
    write_elem(&mut state, VReg::V1, 2, Vsew::E16, 0);
    exec(
        &mut state,
        ZveXxArithInstruction::VmsleuVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    let vd = get_vreg(&state, VReg::V4);
    // bits 0,1 set; bit 2 clear
    assert_eq!(vd[0] & 0x07, 0b011);
}

#[test]
fn vmsleu_vi_negative_imm_always_true() {
    // vmsleu.vi with a negative immediate: the i8 is sign-extended to a full u64, giving
    // 0xFFFF...FF. Both operands are then masked to SEW width before the unsigned compare,
    // so the effective immediate is (0xFFFF...FF & 0xFF) = 0xFF. Every E8 element value
    // is in [0, 255] and 255 <= 255 is the worst case, which is still true. Therefore
    // vs2[i] <= imm is always true for all elements regardless of their value.
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, i as u64);
    }
    // Pre-clear vd bits so we can confirm they all get set
    state.ext_state.write_vregs().get_mut(VReg::V4)[0] = 0x00;
    exec(
        &mut state,
        ZveXxArithInstruction::VmsleuVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            imm: -1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    let vd = get_vreg(&state, VReg::V4);
    assert_eq!(vd[0] & 0x0F, 0x0F);
}

#[test]
fn vmsle_vi_e8_signed() {
    // vmsle.vi imm=-1: vs2[i] <= -1 (signed)
    // 0xFF = -1 as i8: -1 <= -1 = true  (bit 0)
    // 0x00 =  0 as i8:  0 <= -1 = false (bit 1)
    // 0x01 =  1 as i8:  1 <= -1 = false (bit 2)
    // 0xFE = -2 as i8: -2 <= -1 = true  (bit 3)
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xFF);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0x00);
    write_elem(&mut state, VReg::V2, 2, Vsew::E8, 0x01);
    write_elem(&mut state, VReg::V2, 3, Vsew::E8, 0xFE);
    exec(
        &mut state,
        ZveXxArithInstruction::VmsleVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            imm: -1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    let vd = get_vreg(&state, VReg::V4);
    assert_eq!(vd[0] & 0x0F, 0b1001);
}

#[test]
fn vmsgtu_vi_e8_unsigned() {
    // vmsgtu.vi imm=5: vs2[i] > 5 (unsigned)
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 4);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 5);
    write_elem(&mut state, VReg::V2, 2, Vsew::E8, 6);
    // 255 > 5
    write_elem(&mut state, VReg::V2, 3, Vsew::E8, 0xFF);
    exec(
        &mut state,
        ZveXxArithInstruction::VmsgtuVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            imm: 5,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    let vd = get_vreg(&state, VReg::V4);
    // elements 2,3
    assert_eq!(vd[0] & 0x0F, 0b1100);
}

#[test]
fn vmsgt_vx_e32_signed() {
    // vmsgt.vx: vs2[i] > rs1 (signed)
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    // -1
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 0xFFFF_FFFF);
    write_elem(&mut state, VReg::V2, 1, Vsew::E32, 0);
    write_elem(&mut state, VReg::V2, 2, Vsew::E32, 1);
    write_elem(&mut state, VReg::V2, 3, Vsew::E32, 100);
    // rs1 = 0; signed > 0: elements 2 and 3
    state.regs.write(Reg::A0, 0u64);
    exec(
        &mut state,
        ZveXxArithInstruction::VmsgtVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    let vd = get_vreg(&state, VReg::V4);
    assert_eq!(vd[0] & 0x0F, 0b1100);
}

#[test]
fn vmsgt_vi_e8_signed() {
    // vmsgt.vi imm=-1 (i.e. vs2[i] > -1 signed, so vs2[i] >= 0)
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    // -1: NOT > -1
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xFF);
    // 0: > -1
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0x00);
    // 127: > -1
    write_elem(&mut state, VReg::V2, 2, Vsew::E8, 0x7F);
    // -2: NOT > -1
    write_elem(&mut state, VReg::V2, 3, Vsew::E8, 0xFE);
    exec(
        &mut state,
        ZveXxArithInstruction::VmsgtVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            imm: -1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    let vd = get_vreg(&state, VReg::V4);
    assert_eq!(vd[0] & 0x0F, 0b0110);
}

// Masking behaviour

#[test]
fn masked_arith_leaves_inactive_elements_undisturbed() {
    // vm=false: only active (mask bit set) elements are written; others unchanged
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    // Mask: bits 0 and 2 active, bits 1 and 3 inactive
    state.ext_state.write_vregs().get_mut(VReg::V0)[0] = 0b0101;
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, 10);
        write_elem(&mut state, VReg::V1, i, Vsew::E32, 1);
    }
    // Pre-fill vd with sentinel 0xDEAD_BEEF
    for i in 0..4usize {
        write_elem(&mut state, VReg::V4, i, Vsew::E32, 0xDEAD_BEEF);
    }
    exec(
        &mut state,
        ZveXxArithInstruction::VaddVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // active
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 11);
    // undisturbed
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E32), 0xDEAD_BEEF);
    // active
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E32), 11);
    // undisturbed
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E32), 0xDEAD_BEEF);
}

#[test]
fn masked_compare_leaves_inactive_mask_bits_undisturbed() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    // Mask: only bits 0 and 2 active
    state.ext_state.write_vregs().get_mut(VReg::V0)[0] = 0b0101;
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 5);
        write_elem(&mut state, VReg::V1, i, Vsew::E8, 5);
    }
    // Pre-fill destination bits with known pattern: all 1s
    state.ext_state.write_vregs().get_mut(VReg::V4)[0] = 0xFF;
    exec(
        &mut state,
        ZveXxArithInstruction::VmseqVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // Active elements (0,2): eq -> bit set; inactive (1,3): undisturbed (were 1)
    let vd = state.ext_state.read_vregs().get(VReg::V4)[0];
    // bits 0,2 active and eq -> 1; bits 1,3 undisturbed from 1
    assert_eq!(vd & 0x0F, 0b1111);
}

#[test]
fn compare_can_write_to_v0_when_masked() {
    // Per spec, compare destination may be v0 even with masking (vm=false)
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    // Mask v0 all active
    state.ext_state.write_vregs().get_mut(VReg::V0)[0] = 0xFF;
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 5);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 3);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 5);
    write_elem(&mut state, VReg::V1, 1, Vsew::E8, 5);
    // Writing to vd=v0 with vm=false should succeed
    exec(
        &mut state,
        ZveXxArithInstruction::VmseqVv {
            vd: VReg::V0,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 5==5
    assert!(mask_bit(&state, VReg::V0, 0));
    // 3!=5
    assert!(!mask_bit(&state, VReg::V0, 1));
}

// vstart partial execution

#[test]
fn vstart_skips_elements_before_vstart() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, 1);
        write_elem(&mut state, VReg::V1, i, Vsew::E32, 1);
        // Pre-fill vd: sentinel 0xDEAD
        write_elem(&mut state, VReg::V4, i, Vsew::E32, 0xDEAD);
    }
    // Start at element 2: elements 0,1 should remain as sentinel
    state.ext_state.set_vstart(2);
    exec(
        &mut state,
        ZveXxArithInstruction::VaddVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // skipped
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0xDEAD);
    // skipped
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E32), 0xDEAD);
    // executed
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E32), 2);
    // executed
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E32), 2);
    // vstart must be reset to 0
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vstart_skips_elements_before_vstart_compare() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, i as u64);
        write_elem(&mut state, VReg::V1, i, Vsew::E8, i as u64);
    }
    // Pre-fill vd bits with 0
    state.ext_state.write_vregs().get_mut(VReg::V4)[0] = 0x00;
    state.ext_state.set_vstart(2);
    exec(
        &mut state,
        ZveXxArithInstruction::VmseqVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // Bits 0,1 undisturbed (0); bits 2,3 written (eq = 1)
    let vd = state.ext_state.read_vregs().get(VReg::V4)[0];
    assert_eq!(vd & 0x0F, 0b1100);
    assert_eq!(state.ext_state.vstart(), 0);
}

// vl=0: no writes, dirty still incremented

#[test]
fn vl_zero_no_elements_written() {
    let mut state = setup(0, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V4, i, Vsew::E32, 0xDEAD_BEEF);
    }
    exec(
        &mut state,
        ZveXxArithInstruction::VaddVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E32),
            0xDEAD_BEEF,
            "elem {i}"
        );
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
}

// Error paths

#[test]
fn error_vector_instructions_not_allowed() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    state.ext_state.set_vector_allowed(false);
    let result = exec(
        &mut state,
        ZveXxArithInstruction::VaddVv {
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
fn error_vill_set_vtype() {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    // vtype = None (vill set)
    state.ext_state.set_vtype(None);
    state.ext_state.set_vl(0);
    let result = exec(
        &mut state,
        ZveXxArithInstruction::VaddVv {
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
fn error_vd_misaligned_for_m2() {
    // M2: group_regs=2; vd must be even. V3 (odd) is illegal.
    let mut state = setup(4, Vsew::E32, Vlmul::M2);
    let result = exec(
        &mut state,
        ZveXxArithInstruction::VaddVv {
            vd: VReg::V3, // odd -> misaligned for M2
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
fn error_vs2_misaligned_for_m2() {
    let mut state = setup(4, Vsew::E32, Vlmul::M2);
    let result = exec(
        &mut state,
        ZveXxArithInstruction::VaddVv {
            vd: VReg::V4,
            vs2: VReg::V3, // misaligned
            vs1: VReg::V6,
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
fn error_masked_arith_vd_is_v0() {
    // vm=false with vd=v0 is illegal for arithmetic (not compare)
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    let result = exec(
        &mut state,
        ZveXxArithInstruction::VaddVv {
            vd: VReg::V0,
            vs2: VReg::V2,
            vs1: VReg::V4,
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
fn error_vector_not_allowed_compare() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    state.ext_state.set_vector_allowed(false);
    let result = exec(
        &mut state,
        ZveXxArithInstruction::VmseqVv {
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

// Cross-SEW correctness (each SEW for a representative op)

#[test]
fn vadd_wraps_at_sew_boundary() {
    // MAX + 1 must wrap to 0 within SEW, with no bleed into higher bits.
    for (vsew, sew_max) in [
        (Vsew::E8, 0xFFu64),
        (Vsew::E16, 0xFFFF),
        (Vsew::E32, 0xFFFF_FFFF),
        (Vsew::E64, 0xFFFF_FFFF_FFFF_FFFF_u64),
    ] {
        let mut state = setup(1, vsew, Vlmul::M1);
        write_elem(&mut state, VReg::V2, 0, vsew, sew_max);
        write_elem(&mut state, VReg::V1, 0, vsew, 1);
        exec(
            &mut state,
            ZveXxArithInstruction::VaddVv {
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
            read_elem(&state, VReg::V4, 0, vsew),
            0,
            "SEW={vsew:?}: MAX+1 should wrap to 0"
        );
    }
}

#[test]
fn vsra_all_sew_widths_sign_extends_correctly() {
    // For each SEW, 0x80..0 >> (SEW-1) should give 0xFF..F (all ones, i.e. -1)
    for (vsew, msb_val) in [
        (Vsew::E8, 0x80u64),
        (Vsew::E16, 0x8000),
        (Vsew::E32, 0x8000_0000),
        (Vsew::E64, 0x8000_0000_0000_0000_u64),
    ] {
        let mut state = setup(1, vsew, Vlmul::M1);
        write_elem(&mut state, VReg::V2, 0, vsew, msb_val);
        let shamt = vsew.bits_width() - 1;
        state.regs.write(Reg::A0, u64::from(shamt));
        exec(
            &mut state,
            ZveXxArithInstruction::VsraVx {
                vd: VReg::V4,
                vs2: VReg::V2,
                rs1: Reg::A0,
                vm: true,
                rs2: Reg::Zero,
            },
        )
        .unwrap();
        let sew_mask = if vsew.bits_width() == 64 {
            u64::MAX
        } else {
            (1u64 << vsew.bits_width()) - 1
        };
        assert_eq!(
            read_elem(&state, VReg::V4, 0, vsew),
            sew_mask,
            "SEW={vsew:?}"
        );
    }
}

// vs_dirty and vstart invariants

#[test]
fn every_instruction_marks_vs_dirty_exactly_once() {
    // Spot-check a handful of instructions each with vl > 0
    let instrs = &[
        ZveXxArithInstruction::VaddVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
        ZveXxArithInstruction::VsubVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
        ZveXxArithInstruction::VandVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
        ZveXxArithInstruction::VsllVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
        ZveXxArithInstruction::VminuVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
        ZveXxArithInstruction::VmseqVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    ];
    for (n, instr) in instrs.iter().enumerate() {
        let mut state = setup(4, Vsew::E32, Vlmul::M1);
        exec(&mut state, *instr).unwrap();
        assert_eq!(
            state.ext_state.vs_dirty_count(),
            1,
            "instr #{n} didn't mark dirty exactly once"
        );
        assert_eq!(
            state.ext_state.vstart(),
            0,
            "instr #{n} didn't reset vstart"
        );
    }
}

#[test]
fn error_compare_mask_dest_overlaps_vs2_lmul_gt_1() {
    // vmseq.vv with LMUL=2 and vd inside the vs2 group [v2, v3] is reserved
    let mut state = setup(8, Vsew::E32, Vlmul::M2);
    let result = exec(
        &mut state,
        ZveXxArithInstruction::VmseqVv {
            vd: VReg::V3,
            vs2: VReg::V2,
            vs1: VReg::V6,
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
fn error_compare_mask_dest_overlaps_vs1_lmul_gt_1() {
    let mut state = setup(8, Vsew::E32, Vlmul::M2);
    let result = exec(
        &mut state,
        ZveXxArithInstruction::VmseqVv {
            vd: VReg::V7,
            vs2: VReg::V2,
            vs1: VReg::V6,
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
fn error_compare_mask_dest_overlaps_vs2_lmul_gt_1_vx() {
    let mut state = setup(8, Vsew::E32, Vlmul::M2);
    state.regs.write(Reg::A0, 0);
    let result = exec(
        &mut state,
        ZveXxArithInstruction::VmseqVx {
            vd: VReg::V2,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    );
    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

#[test]
fn compare_mask_dest_may_overlap_source_at_lmul_1() {
    // With LMUL=1 the group is a single register, so vd == vs2 is explicitly allowed
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, 42);
    }
    state.regs.write(Reg::A0, 42);
    exec(
        &mut state,
        ZveXxArithInstruction::VmseqVx {
            vd: VReg::V2,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // All 4 elements equal 42 -> low 4 bits set
    assert_eq!(state.ext_state.read_vregs().get(VReg::V2)[0] & 0x0F, 0x0F);
}

#[test]
fn compare_mask_dest_outside_source_group_lmul_gt_1_ok() {
    // vd=v8, group vs2=[v2,v3], vs1=[v6,v7]: no overlap, should succeed
    let mut state = setup(8, Vsew::E32, Vlmul::M2);
    for i in 0..8usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, i as u64);
        write_elem(&mut state, VReg::V6, i, Vsew::E32, i as u64);
    }
    exec(
        &mut state,
        ZveXxArithInstruction::VmseqVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V6,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(state.ext_state.read_vregs().get(VReg::V8)[0], 0xFF);
}
