use crate::rv64::test_utils::{TestInterpreterState, initialize_state};
use crate::v::vector_registers::{VectorRegisters, VectorRegistersExt};
use crate::v::zve64x::muldiv::zve64x_muldiv_helpers::widening_dest_register_count;
use crate::{
    ExecutableInstruction, ExecutableInstructionOperands, ExecutionError, RegisterFile,
    Rs1Rs2OperandValues, Rs1Rs2Operands,
};
use ab_riscv_primitives::prelude::*;

// With TEST_VLEN=256, VLENB=32:
//   E8/M1  -> VLMAX=32, 1 reg
//   E16/M1 -> VLMAX=16, 1 reg
//   E32/M1 -> VLMAX=8,  1 reg
//   E64/M1 -> VLMAX=4,  1 reg
//   E8/M2  -> VLMAX=64, 2 regs
//   E16/M2 -> VLMAX=32, 2 regs
//   E32/M2 -> VLMAX=16, 2 regs (vd for widening E16 uses 2 regs)
//   E8/M4  -> VLMAX=128, 4 regs (vd for widening E32 uses 4 regs - but VLMAX=8 at E32/M1)

fn encode_vtype(vsew: Vsew, vlmul: Vlmul) -> u64 {
    u64::from(vlmul.to_bits()) | (u64::from(vsew.to_bits()) << 3u8)
}

fn setup(
    vl: u32,
    vsew: Vsew,
    vlmul: Vlmul,
) -> TestInterpreterState<Zve64xMulDivInstruction<Reg<u64>>> {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    let vtype = Vtype::from_raw::<Reg<u64>>(encode_vtype(vsew, vlmul)).unwrap();
    state.ext_state.set_vtype(Some(vtype));
    state.ext_state.set_vl(vl);
    state.ext_state.set_vstart(0);
    state
}

fn exec(
    state: &mut TestInterpreterState<Zve64xMulDivInstruction<Reg<u64>>>,
    instr: Zve64xMulDivInstruction<Reg<u64>>,
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
    state: &TestInterpreterState<Zve64xMulDivInstruction<Reg<u64>>>,
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

fn read_wide_elem(
    state: &TestInterpreterState<Zve64xMulDivInstruction<Reg<u64>>>,
    base_reg: VReg,
    elem_i: usize,
    sew: Vsew,
) -> u64 {
    let wide_bytes = usize::from(sew.bytes_width()) * 2;
    let elems_per_reg = 16 / wide_bytes;
    let reg_off = elem_i / elems_per_reg;
    let byte_off = (elem_i % elems_per_reg) * wide_bytes;
    let reg = state
        .ext_state
        .read_vregs()
        .get(VReg::from_bits(base_reg.to_bits() + reg_off as u8).unwrap());
    let mut buf = [0u8; 8];
    buf[..wide_bytes].copy_from_slice(&reg[byte_off..byte_off + wide_bytes]);
    u64::from_le_bytes(buf)
}

fn write_elem(
    state: &mut TestInterpreterState<Zve64xMulDivInstruction<Reg<u64>>>,
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

fn write_wide_elem(
    state: &mut TestInterpreterState<Zve64xMulDivInstruction<Reg<u64>>>,
    base_reg: VReg,
    elem_i: usize,
    sew: Vsew,
    value: u64,
) {
    let wide_bytes = usize::from(sew.bytes_width()) * 2;
    let elems_per_reg = 16 / wide_bytes;
    let reg_off = elem_i / elems_per_reg;
    let byte_off = (elem_i % elems_per_reg) * wide_bytes;
    let reg = state
        .ext_state
        .write_vregs()
        .get_mut(VReg::from_bits(base_reg.to_bits() + reg_off as u8).unwrap());
    let buf = value.to_le_bytes();
    reg[byte_off..byte_off + wide_bytes].copy_from_slice(&buf[..wide_bytes]);
}

fn set_mask_bit(
    state: &mut TestInterpreterState<Zve64xMulDivInstruction<Reg<u64>>>,
    elem_i: u32,
    val: bool,
) {
    let reg = state.ext_state.write_vregs().get_mut(VReg::V0);
    let byte = &mut reg[(elem_i / u8::BITS) as usize];
    if val {
        *byte |= 1 << (elem_i % u8::BITS);
    } else {
        *byte &= !(1 << (elem_i % u8::BITS));
    }
}

// vmul

#[test]
fn vmul_vv_e32_m1_basic() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, (i + 1) as u64);
        write_elem(&mut state, VReg::V4, i, Vsew::E32, 3);
    }
    exec(
        &mut state,
        Zve64xMulDivInstruction::VmulVv {
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
            read_elem(&state, VReg::V8, i, Vsew::E32),
            (i + 1) as u64 * 3,
            "elem {i}"
        );
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vmul_vv_e8_wraps() {
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    // 200 * 2 = 400, truncated to 8 bits = 144
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 200);
    write_elem(&mut state, VReg::V4, 0, Vsew::E8, 2);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VmulVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E8), 400u64 & 0xFF);
}

#[test]
fn vmul_vx_e64_m1() {
    let mut state = setup(2, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E64, 7);
    write_elem(&mut state, VReg::V2, 1, Vsew::E64, u64::MAX);
    state.regs.write(Reg::A0, 3u64);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VmulVx {
            vd: VReg::V8,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E64), 21);
    // u64::MAX * 3 wraps to u64::MAX - 2
    assert_eq!(
        read_elem(&state, VReg::V8, 1, Vsew::E64),
        u64::MAX.wrapping_mul(3)
    );
}

#[test]
fn vmul_masked_skips_inactive() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    // mask: only elements 0 and 2 active (bits 0 and 2 set)
    state.ext_state.write_vregs().get_mut(VReg::V0)[0] = 0b0000_0101;
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, 5);
        write_elem(&mut state, VReg::V4, i, Vsew::E32, 10);
        // vd pre-filled with sentinel
        write_elem(&mut state, VReg::V8, i, Vsew::E32, 0xDEAD);
    }
    exec(
        &mut state,
        Zve64xMulDivInstruction::VmulVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // Active elements written
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E32), 50);
    assert_eq!(read_elem(&state, VReg::V8, 2, Vsew::E32), 50);
    // Inactive elements undisturbed
    assert_eq!(read_elem(&state, VReg::V8, 1, Vsew::E32), 0xDEAD);
    assert_eq!(read_elem(&state, VReg::V8, 3, Vsew::E32), 0xDEAD);
}

// vmulh (signed×signed high half)

#[test]
fn vmulh_vv_e8_positive() {
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    // 10 * 10 = 100; high 8 bits of 16-bit product = 0
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 10);
    write_elem(&mut state, VReg::V4, 0, Vsew::E8, 10);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VmulhVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E8), 0);
}

#[test]
fn vmulh_vv_e16_large() {
    let mut state = setup(1, Vsew::E16, Vlmul::M1);
    // -32768 * -32768 = 2^30; high 16 bits = 2^30 >> 16 = 2^14 = 16384
    // as i16: -32768 stored as 0x8000
    write_elem(&mut state, VReg::V2, 0, Vsew::E16, 0x8000);
    write_elem(&mut state, VReg::V4, 0, Vsew::E16, 0x8000);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VmulhVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // (-32768) * (-32768) = 1073741824 = 0x40000000
    // high 16 bits = 0x4000 = 16384
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E16), 0x4000);
}

#[test]
fn vmulh_vv_e16_signed_negative_result() {
    let mut state = setup(1, Vsew::E16, Vlmul::M1);
    // 32767 * (-1) = -32767; as i32 = 0xFFFF8001; high 16 bits = 0xFFFF = -1 as i16
    write_elem(&mut state, VReg::V2, 0, Vsew::E16, 32767);
    // -1 as i16 = 0xFFFF
    write_elem(&mut state, VReg::V4, 0, Vsew::E16, 0xFFFF);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VmulhVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 32767 * -1 = -32767 = 0xFFFF8001 as i32; high 16 = 0xFFFF
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E16), 0xFFFF);
}

#[test]
fn vmulh_vx_e32() {
    let mut state = setup(1, Vsew::E32, Vlmul::M1);
    // 0x7FFFFFFF * 2 = 0xFFFFFFFE; high 32 bits of 64-bit product = 0
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 0x7FFF_FFFF);
    state.regs.write(Reg::A0, 2u64);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VmulhVx {
            vd: VReg::V8,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E32), 0);
}

#[test]
fn vmulh_illegal_for_sew64() {
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xMulDivInstruction::VmulhVv {
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

// vmulhu (unsigned×unsigned high half)

#[test]
fn vmulhu_vv_e8() {
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    // 200 * 200 = 40000; high 8 bits = 40000 >> 8 = 156
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 200);
    write_elem(&mut state, VReg::V4, 0, Vsew::E8, 200);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VmulhuVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E8), 40000 >> 8u8);
}

#[test]
fn vmulhu_vx_e16() {
    let mut state = setup(1, Vsew::E16, Vlmul::M1);
    // 0xFFFF * 0xFFFF = 0xFFFE0001; high 16 bits = 0xFFFE
    write_elem(&mut state, VReg::V2, 0, Vsew::E16, 0xFFFF);
    state.regs.write(Reg::A0, 0xFFFFu64);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VmulhuVx {
            vd: VReg::V8,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E16), 0xFFFE);
}

#[test]
fn vmulhu_illegal_for_sew64() {
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xMulDivInstruction::VmulhuVv {
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

// vmulhsu (signed×unsigned high half)

#[test]
fn vmulhsu_vv_e8_positive_result() {
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    // vs2=3 (signed), vs1=100 (unsigned): 3*100=300; high 8 bits = 300>>8 = 1
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 3);
    write_elem(&mut state, VReg::V4, 0, Vsew::E8, 100);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VmulhsuVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E8), 1);
}

#[test]
fn vmulhsu_vv_e8_negative_signed() {
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    // vs2=-1 (0xFF signed=-1), vs1=200 (unsigned): -1*200=-200; high 8 = -200>>8 = -1 = 0xFF
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xFF);
    write_elem(&mut state, VReg::V4, 0, Vsew::E8, 200);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VmulhsuVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E8), 0xFF);
}

#[test]
fn vmulhsu_illegal_for_sew64() {
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xMulDivInstruction::VmulhsuVv {
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

// vdivu

#[test]
fn vdivu_vv_e32_basic() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    let dividends = [100u64, 255, 1024, 0xFFFF_FFFF];
    let divisors = [5u64, 3, 64, 2];
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, dividends[i]);
        write_elem(&mut state, VReg::V4, i, Vsew::E32, divisors[i]);
    }
    exec(
        &mut state,
        Zve64xMulDivInstruction::VdivuVv {
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
            read_elem(&state, VReg::V8, i, Vsew::E32),
            dividends[i] / divisors[i],
            "elem {i}"
        );
    }
}

#[test]
fn vdivu_vv_e32_div_by_zero_returns_all_ones() {
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 42);
    write_elem(&mut state, VReg::V4, 0, Vsew::E32, 0);
    write_elem(&mut state, VReg::V2, 1, Vsew::E32, 0);
    write_elem(&mut state, VReg::V4, 1, Vsew::E32, 0);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VdivuVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // Spec §12.11: division by zero yields all-ones (0xFFFF_FFFF for E32)
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E32), 0xFFFF_FFFF);
    assert_eq!(read_elem(&state, VReg::V8, 1, Vsew::E32), 0xFFFF_FFFF);
}

#[test]
fn vdivu_vx_e8_div_by_zero() {
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 99);
    state.regs.write(Reg::A0, 0u64);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VdivuVx {
            vd: VReg::V8,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E8), 0xFF);
}

#[test]
fn vdivu_vv_e64_basic() {
    let mut state = setup(2, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E64, 1_000_000_000_000u64);
    write_elem(&mut state, VReg::V4, 0, Vsew::E64, 1_000_000u64);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VdivuVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E64), 1_000_000u64);
}

// vdiv (signed)

#[test]
fn vdiv_vv_e32_basic() {
    let mut state = setup(3, Vsew::E32, Vlmul::M1);
    // -10 / 3 = -3 (truncation toward zero)
    // as u32: -10 = 0xFFFF_FFF6, -3 = 0xFFFF_FFFD
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 0xFFFF_FFF6);
    write_elem(&mut state, VReg::V4, 0, Vsew::E32, 3);
    // 100 / -7 = -14
    write_elem(&mut state, VReg::V2, 1, Vsew::E32, 100);
    // -7
    write_elem(&mut state, VReg::V4, 1, Vsew::E32, 0xFFFF_FFF9);
    // 0 / 5 = 0
    write_elem(&mut state, VReg::V2, 2, Vsew::E32, 0);
    write_elem(&mut state, VReg::V4, 2, Vsew::E32, 5);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VdivVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // -3
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E32), 0xFFFF_FFFD);
    // -14
    assert_eq!(read_elem(&state, VReg::V8, 1, Vsew::E32), 0xFFFF_FFF2);
    assert_eq!(read_elem(&state, VReg::V8, 2, Vsew::E32), 0);
}

#[test]
fn vdiv_vv_e32_div_by_zero_returns_neg1() {
    let mut state = setup(1, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 42);
    write_elem(&mut state, VReg::V4, 0, Vsew::E32, 0);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VdivVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // Spec §12.11: signed division by zero yields all-ones (= -1 signed = MAX unsigned)
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E32), 0xFFFF_FFFF);
}

#[test]
fn vdiv_vv_e16_signed_overflow_returns_min() {
    let mut state = setup(1, Vsew::E16, Vlmul::M1);
    // MIN / -1 = MIN (overflow case per spec §12.11)
    // i16::MIN
    write_elem(&mut state, VReg::V2, 0, Vsew::E16, 0x8000);
    // -1
    write_elem(&mut state, VReg::V4, 0, Vsew::E16, 0xFFFF);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VdivVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E16), 0x8000);
}

#[test]
fn vdiv_vx_e64_neg() {
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    // -1000 / 7 = -142
    write_elem(
        &mut state,
        VReg::V2,
        0,
        Vsew::E64,
        (-1000i64).cast_unsigned(),
    );
    state.regs.write(Reg::A0, 7u64);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VdivVx {
            vd: VReg::V8,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(
        read_elem(&state, VReg::V8, 0, Vsew::E64).cast_signed(),
        -142i64
    );
}

// vremu

#[test]
fn vremu_vv_e32_basic() {
    let mut state = setup(3, Vsew::E32, Vlmul::M1);
    let cases = [(17u64, 5u64, 2u64), (100, 11, 1), (0, 7, 0)];
    for (i, (a, b, _)) in cases.iter().enumerate() {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, *a);
        write_elem(&mut state, VReg::V4, i, Vsew::E32, *b);
    }
    exec(
        &mut state,
        Zve64xMulDivInstruction::VremuVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for (i, (_, _, expected)) in cases.iter().enumerate() {
        assert_eq!(
            read_elem(&state, VReg::V8, i, Vsew::E32),
            *expected,
            "elem {i}"
        );
    }
}

#[test]
fn vremu_vv_e8_div_by_zero_returns_dividend() {
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 77);
    write_elem(&mut state, VReg::V4, 0, Vsew::E8, 0);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VremuVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // Spec §12.11: unsigned remainder by zero = dividend
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E8), 77);
}

#[test]
fn vremu_vx_e16() {
    let mut state = setup(2, Vsew::E16, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E16, 1000);
    // 65535
    write_elem(&mut state, VReg::V2, 1, Vsew::E16, 0xFFFF);
    state.regs.write(Reg::A0, 7u64);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VremuVx {
            vd: VReg::V8,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E16), 1000 % 7);
    assert_eq!(read_elem(&state, VReg::V8, 1, Vsew::E16), 65535 % 7);
}

// vrem (signed)

#[test]
fn vrem_vv_e32_basic() {
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    // -13 % 5 = -3 (Rust truncation semantics, same as RISC-V)
    write_elem(
        &mut state,
        VReg::V2,
        0,
        Vsew::E32,
        u64::from((-13i32).cast_unsigned()),
    );
    write_elem(&mut state, VReg::V4, 0, Vsew::E32, 5);
    // 13 % -5 = 3
    write_elem(&mut state, VReg::V2, 1, Vsew::E32, 13);
    write_elem(
        &mut state,
        VReg::V4,
        1,
        Vsew::E32,
        u64::from((-5i32).cast_unsigned()),
    );
    exec(
        &mut state,
        Zve64xMulDivInstruction::VremVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E32) as i32, -3i32);
    assert_eq!(read_elem(&state, VReg::V8, 1, Vsew::E32) as i32, 3i32);
}

#[test]
fn vrem_vv_e16_div_by_zero_returns_dividend() {
    let mut state = setup(1, Vsew::E16, Vlmul::M1);
    // some negative value
    write_elem(&mut state, VReg::V2, 0, Vsew::E16, 0x8042);
    write_elem(&mut state, VReg::V4, 0, Vsew::E16, 0);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VremVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // Spec §12.11: signed remainder by zero = dividend
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E16), 0x8042);
}

#[test]
fn vrem_vv_e32_signed_overflow_returns_zero() {
    let mut state = setup(1, Vsew::E32, Vlmul::M1);
    // MIN % -1 = 0 per spec §12.11
    // i32::MIN
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 0x8000_0000);
    // -1
    write_elem(&mut state, VReg::V4, 0, Vsew::E32, 0xFFFF_FFFF);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VremVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E32), 0);
}

#[test]
fn vrem_vx_e8() {
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    // -127 % 7: -127 = 0x81 as i8, result = -127 % 7 = -1
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0x81);
    state.regs.write(Reg::A0, 7u64);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VremVx {
            vd: VReg::V8,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // -127 % 7 = -1 (truncation toward zero)
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E8) as i8, -1i8);
}

// vwmulu

#[test]
fn vwmulu_vv_e8_to_e16() {
    // SEW=E8, LMUL=M1 -> vd is E16 with 2*group_regs=2 regs (V8 and V9)
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    let vals_a = [200u64, 255, 1, 128];
    let vals_b = [200u64, 255, 255, 3];
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, vals_a[i]);
        write_elem(&mut state, VReg::V4, i, Vsew::E8, vals_b[i]);
    }
    exec(
        &mut state,
        Zve64xMulDivInstruction::VwmuluVv {
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
            read_wide_elem(&state, VReg::V8, i, Vsew::E8),
            vals_a[i] * vals_b[i],
            "elem {i}"
        );
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
}

#[test]
fn vwmulu_vx_e16_to_e32() {
    let mut state = setup(4, Vsew::E16, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E16, (i + 1) as u64 * 1000);
    }
    state.regs.write(Reg::A0, 7u64);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VwmuluVx {
            vd: VReg::V8,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        assert_eq!(
            read_wide_elem(&state, VReg::V8, i, Vsew::E16),
            (i + 1) as u64 * 7000,
            "elem {i}"
        );
    }
}

#[test]
fn vwmulu_illegal_for_sew64() {
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xMulDivInstruction::VwmuluVv {
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
fn vwmulu_overlap_rejected() {
    // vd=V4 (occupies V4+V5), vs2=V4 - overlap -> illegal
    let mut state = setup(2, Vsew::E16, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xMulDivInstruction::VwmuluVv {
            vd: VReg::V4,
            vs2: VReg::V4,
            vs1: VReg::V2,
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
fn vwmulu_m8_is_illegal() {
    // LMUL=M8 would require EMUL=16 for vd, which is out of range
    let mut state = setup(4, Vsew::E8, Vlmul::M8);
    let result = exec(
        &mut state,
        Zve64xMulDivInstruction::VwmuluVv {
            vd: VReg::V0,
            vs2: VReg::V0,
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
fn vwmulu_mf2_e8_correct_result() {
    // LMUL=Mf2, SEW=E8: VLMAX = VLEN/2 / 8 = 256/2/8 = 16 elements
    // EMUL = 2 * (1/2) = 1, so vd occupies 1 register (same as vs2/vs1)
    // With VLENB=32: 16 E8 elements fit in half a register, so VLMAX=16
    let mut state = setup(4, Vsew::E8, Vlmul::Mf2);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, (i + 1) as u64 * 10);
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 3);
    }
    exec(
        &mut state,
        Zve64xMulDivInstruction::VwmuluVv {
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
            read_wide_elem(&state, VReg::V8, i, Vsew::E8),
            (i + 1) as u64 * 30,
            "elem {i}"
        );
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
}

#[test]
fn vwmulu_mf2_no_false_overlap_rejection() {
    // With Mf2, vd has dest_group_regs=1, vs2 has group_regs=1.
    // V8 and V2 do not overlap - this must succeed.
    let mut state = setup(2, Vsew::E8, Vlmul::Mf2);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 5);
    write_elem(&mut state, VReg::V4, 0, Vsew::E8, 6);
    let result = exec(
        &mut state,
        Zve64xMulDivInstruction::VwmuluVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    );
    assert!(result.is_ok());
    assert_eq!(read_wide_elem(&state, VReg::V8, 0, Vsew::E8), 30u64);
}

#[test]
fn vwmulu_mf2_overlap_still_rejected() {
    // With Mf2, dest_group_regs=1, src_group_regs=1.
    // vd=V2 and vs2=V2 overlap: both occupy register index 2.
    let mut state = setup(2, Vsew::E8, Vlmul::Mf2);
    let result = exec(
        &mut state,
        Zve64xMulDivInstruction::VwmuluVv {
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
fn vwmulu_m1_overlap_uses_2_dest_regs() {
    // With M1, dest_group_regs=2: vd=V4 occupies V4+V5.
    // vs2=V4: overlaps with vd -> illegal.
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xMulDivInstruction::VwmuluVv {
            vd: VReg::V4,
            vs2: VReg::V4,
            vs1: VReg::V2,
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
fn vwmulu_m1_vs2_in_upper_dest_reg_is_legal() {
    // With M1, vd=V4 occupies V4+V5 (source EMUL=1). vs2=V5 occupies exactly the
    // highest-numbered register of the destination group, which the RISC-V "V" spec §5.2
    // permits for widening instructions. With vl=4 the wide results land entirely in V4, so the
    // V5 source is not clobbered before it is read.
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V5, i, Vsew::E8, (i + 1) as u64);
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 2);
    }
    let result = exec(
        &mut state,
        Zve64xMulDivInstruction::VwmuluVv {
            vd: VReg::V4,
            vs2: VReg::V5,
            vs1: VReg::V2,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    );
    assert!(result.is_ok());
    for i in 0..4usize {
        assert_eq!(
            read_wide_elem(&state, VReg::V4, i, Vsew::E8),
            (i + 1) as u64 * 2,
            "elem {i}"
        );
    }
}

#[test]
fn vwmulu_m1_vs1_in_upper_dest_reg_is_legal() {
    // Same legal top-overlap as above, but for vs1 (the second source operand). This mirrors the
    // `vwmul.vv v10, v7, v11` case: vd=V10 occupies V10+V11 and vs1=V11 is the highest-numbered
    // register of the destination group.
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V7, i, Vsew::E8, (i + 1) as u64);
        write_elem(&mut state, VReg::V11, i, Vsew::E8, 3);
    }
    let result = exec(
        &mut state,
        Zve64xMulDivInstruction::VwmuluVv {
            vd: VReg::V10,
            vs2: VReg::V7,
            vs1: VReg::V11,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    );
    assert!(result.is_ok());
    for i in 0..4usize {
        assert_eq!(
            read_wide_elem(&state, VReg::V10, i, Vsew::E8),
            (i + 1) as u64 * 3,
            "elem {i}"
        );
    }
}

#[test]
fn vwmulu_m1_vs2_in_lower_dest_reg_is_illegal() {
    // With M1, vd=V4 occupies V4+V5. vs2=V4 overlaps the *lowest*-numbered register of the
    // destination group, which is not the permitted highest-numbered overlap -> illegal.
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xMulDivInstruction::VwmuluVv {
            vd: VReg::V4,
            vs2: VReg::V4,
            vs1: VReg::V2,
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
fn vwmulu_m2_lower_half_overlap_is_illegal() {
    // With M2, vd=V8 occupies V8..V12 (4 regs) and a source occupies 2 regs. The only legal
    // overlap is the top half (V10+V11). vs1=V8 occupies the lowest-numbered half of the
    // destination group, which is not the permitted highest-numbered overlap -> illegal.
    let mut state = setup(4, Vsew::E8, Vlmul::M2);
    let result = exec(
        &mut state,
        Zve64xMulDivInstruction::VwmuluVv {
            vd: VReg::V8,
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
fn vwmulu_m2_top_half_overlap_is_legal() {
    // With M2, vd=V8 occupies V8..V12. vs1=V10 occupies exactly the top half (V10+V11) of the
    // destination group -> legal per spec §5.2.
    let mut state = setup(4, Vsew::E8, Vlmul::M2);
    let result = exec(
        &mut state,
        Zve64xMulDivInstruction::VwmuluVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V10,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    );
    assert!(result.is_ok());
}

// vwmul (signed widening)

#[test]
fn vwmul_vv_e8_signed() {
    let mut state = setup(3, Vsew::E8, Vlmul::M1);
    // -1 * -1 = 1
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xFF);
    write_elem(&mut state, VReg::V4, 0, Vsew::E8, 0xFF);
    // -128 * 2 = -256 = 0xFF00 as u16
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0x80);
    write_elem(&mut state, VReg::V4, 1, Vsew::E8, 2);
    // 127 * 127 = 16129
    write_elem(&mut state, VReg::V2, 2, Vsew::E8, 127);
    write_elem(&mut state, VReg::V4, 2, Vsew::E8, 127);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VwmulVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_wide_elem(&state, VReg::V8, 0, Vsew::E8), 1u64);
    // -256 as u16
    assert_eq!(
        read_wide_elem(&state, VReg::V8, 1, Vsew::E8),
        u64::from((-256i16).cast_unsigned())
    );
    assert_eq!(read_wide_elem(&state, VReg::V8, 2, Vsew::E8), 16129u64);
}

#[test]
fn vwmul_vx_e16_signed() {
    let mut state = setup(2, Vsew::E16, Vlmul::M1);
    // -100 * 3 = -300 as i32 = 0xFFFF_FECC as u32
    write_elem(
        &mut state,
        VReg::V2,
        0,
        Vsew::E16,
        u64::from((-100i16).cast_unsigned()),
    );
    state.regs.write(Reg::A0, 3u64);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VwmulVx {
            vd: VReg::V8,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(
        read_wide_elem(&state, VReg::V8, 0, Vsew::E16) as i32,
        -300i32
    );
}

// vwmulsu

#[test]
fn vwmulsu_vv_e8_signed_unsigned() {
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    // -1 (signed) * 200 (unsigned) = -200; as u16 = 0xFF38
    // -1 signed
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xFF);
    // 200 unsigned
    write_elem(&mut state, VReg::V4, 0, Vsew::E8, 200);
    // 2 (signed) * 200 (unsigned) = 400
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 2);
    write_elem(&mut state, VReg::V4, 1, Vsew::E8, 200);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VwmulsuVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(
        read_wide_elem(&state, VReg::V8, 0, Vsew::E8) as i16,
        -200i16
    );
    assert_eq!(read_wide_elem(&state, VReg::V8, 1, Vsew::E8), 400u64);
}

// vmacc

#[test]
fn vmacc_vv_e32_basic() {
    // vmacc: vd[i] = vd[i] + vs1[i] * vs2[i]
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        // accumulator
        write_elem(&mut state, VReg::V8, i, Vsew::E32, 100);
        // vs1
        write_elem(&mut state, VReg::V2, i, Vsew::E32, 3);
        // vs2
        write_elem(&mut state, VReg::V4, i, Vsew::E32, 7);
    }
    exec(
        &mut state,
        Zve64xMulDivInstruction::VmaccVv {
            vd: VReg::V8,
            vs1: VReg::V2,
            vs2: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        // 100 + 3 * 7 = 121
        assert_eq!(read_elem(&state, VReg::V8, i, Vsew::E32), 121, "elem {i}");
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vmacc_vx_e64_basic() {
    let mut state = setup(2, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V8, 0, Vsew::E64, 1000);
    write_elem(&mut state, VReg::V4, 0, Vsew::E64, 50);
    write_elem(&mut state, VReg::V8, 1, Vsew::E64, u64::MAX);
    write_elem(&mut state, VReg::V4, 1, Vsew::E64, 1);
    state.regs.write(Reg::A0, 2u64);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VmaccVx {
            vd: VReg::V8,
            rs1: Reg::A0,
            vs2: VReg::V4,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 1000 + 2*50 = 1100
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E64), 1100);
    // u64::MAX + 2*1 wraps
    assert_eq!(
        read_elem(&state, VReg::V8, 1, Vsew::E64),
        u64::MAX.wrapping_add(2)
    );
}

// vnmsac

#[test]
fn vnmsac_vv_e32() {
    // vnmsac: vd[i] = vd[i] - vs1[i] * vs2[i]
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    // acc
    write_elem(&mut state, VReg::V8, 0, Vsew::E32, 200);
    // vs1
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 5);
    // vs2
    write_elem(&mut state, VReg::V4, 0, Vsew::E32, 7);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VnmsacVv {
            vd: VReg::V8,
            vs1: VReg::V2,
            vs2: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 200 - 5*7 = 200 - 35 = 165
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E32), 165);
}

#[test]
fn vnmsac_vx_e8_wraps() {
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    // acc
    write_elem(&mut state, VReg::V8, 0, Vsew::E8, 0);
    // vs2
    write_elem(&mut state, VReg::V4, 0, Vsew::E8, 5);
    state.regs.write(Reg::A0, 3u64);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VnmsacVx {
            vd: VReg::V8,
            rs1: Reg::A0,
            vs2: VReg::V4,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 0 - 3*5 = -15 wraps to 241 as u8
    assert_eq!(
        read_elem(&state, VReg::V8, 0, Vsew::E8),
        u64::from(0u8.wrapping_sub(15))
    );
}

// vmadd

#[test]
fn vmadd_vv_e32() {
    // vmadd: vd[i] = vs1[i] * vd[i] + vs2[i]
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    // vd (multiplicand)
    write_elem(&mut state, VReg::V8, 0, Vsew::E32, 4);
    // vs1 (multiplier)
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 5);
    // vs2 (addend)
    write_elem(&mut state, VReg::V4, 0, Vsew::E32, 10);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VmaddVv {
            vd: VReg::V8,
            vs1: VReg::V2,
            vs2: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 5 * 4 + 10 = 30
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E32), 30);
}

#[test]
fn vmadd_vx_e16() {
    // vmadd: vd[i] = rs1 * vd[i] + vs2[i]
    let mut state = setup(2, Vsew::E16, Vlmul::M1);
    // vd
    write_elem(&mut state, VReg::V8, 0, Vsew::E16, 6);
    // vs2
    write_elem(&mut state, VReg::V4, 0, Vsew::E16, 20);
    state.regs.write(Reg::A0, 3u64);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VmaddVx {
            vd: VReg::V8,
            rs1: Reg::A0,
            vs2: VReg::V4,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 3 * 6 + 20 = 38
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E16), 38);
}

// vnmsub

#[test]
fn vnmsub_vv_e32() {
    // vnmsub: vd[i] = -(vs1[i] * vd[i]) + vs2[i]  =  vs2[i] - vs1[i]*vd[i]
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    // vd (multiplicand)
    write_elem(&mut state, VReg::V8, 0, Vsew::E32, 4);
    // vs1 (multiplier)
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 3);
    // vs2 (minuend)
    write_elem(&mut state, VReg::V4, 0, Vsew::E32, 100);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VnmsubVv {
            vd: VReg::V8,
            vs1: VReg::V2,
            vs2: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 100 - 3*4 = 88
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E32), 88);
}

#[test]
fn vnmsub_vx_e64_wraps() {
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V8, 0, Vsew::E64, 2);
    write_elem(&mut state, VReg::V4, 0, Vsew::E64, 0);
    state.regs.write(Reg::A0, u64::MAX);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VnmsubVx {
            vd: VReg::V8,
            rs1: Reg::A0,
            vs2: VReg::V4,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 0 - u64::MAX * 2 = 0 - (u64::MAX.wrapping_mul(2)) = 0 - 0xFFFFFFFFFFFFFFFE = 2
    assert_eq!(
        read_elem(&state, VReg::V8, 0, Vsew::E64),
        0u64.wrapping_sub(u64::MAX.wrapping_mul(2))
    );
}

// vwmaccu

#[test]
fn vwmaccu_vv_e8_basic() {
    // vwmaccu: vd[i] = vd[i] + zext(vs1[i]) * zext(vs2[i]), vd is 2*SEW wide
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    // acc in vd at 2*SEW (E16)
    write_wide_elem(&mut state, VReg::V8, 0, Vsew::E8, 1000);
    write_wide_elem(&mut state, VReg::V8, 1, Vsew::E8, 0);
    // vs1
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 200);
    // vs2
    write_elem(&mut state, VReg::V4, 0, Vsew::E8, 200);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 255);
    write_elem(&mut state, VReg::V4, 1, Vsew::E8, 255);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VwmaccuVv {
            vd: VReg::V8,
            vs1: VReg::V2,
            vs2: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 1000 + 200*200 = 1000 + 40000 = 41000
    assert_eq!(read_wide_elem(&state, VReg::V8, 0, Vsew::E8), 41000u64);
    // 0 + 255*255 = 65025
    assert_eq!(read_wide_elem(&state, VReg::V8, 1, Vsew::E8), 65025u64);
}

#[test]
fn vwmaccu_vx_e16() {
    let mut state = setup(2, Vsew::E16, Vlmul::M1);
    write_wide_elem(&mut state, VReg::V8, 0, Vsew::E16, 500);
    write_wide_elem(&mut state, VReg::V8, 1, Vsew::E16, 0);
    // vs2
    write_elem(&mut state, VReg::V4, 0, Vsew::E16, 1000);
    write_elem(&mut state, VReg::V4, 1, Vsew::E16, 0xFFFF);
    state.regs.write(Reg::A0, 3u64);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VwmaccuVx {
            vd: VReg::V8,
            rs1: Reg::A0,
            vs2: VReg::V4,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 500 + 3*1000 = 3500
    assert_eq!(read_wide_elem(&state, VReg::V8, 0, Vsew::E16), 3_500);
    // 0 + 3*65535 = 196605
    assert_eq!(read_wide_elem(&state, VReg::V8, 1, Vsew::E16), 196_605);
}

// vwmacc (signed widening multiply-add)

#[test]
fn vwmacc_vv_e8_signed() {
    // vwmacc: vd[i] = vd[i] + sext(vs1[i]) * sext(vs2[i])
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    write_wide_elem(&mut state, VReg::V8, 0, Vsew::E8, 0);
    write_wide_elem(&mut state, VReg::V8, 1, Vsew::E8, 0);
    // -1 signed
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xFF);
    // -1 signed
    write_elem(&mut state, VReg::V4, 0, Vsew::E8, 0xFF);
    // -128 signed
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0x80);
    // 2
    write_elem(&mut state, VReg::V4, 1, Vsew::E8, 0x02);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VwmaccVv {
            vd: VReg::V8,
            vs1: VReg::V2,
            vs2: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 0 + (-1)*(-1) = 1; as u16 = 1
    assert_eq!(read_wide_elem(&state, VReg::V8, 0, Vsew::E8), 1u64);
    // 0 + (-128) * 2 = -256; as u16 = 0xFF00
    assert_eq!(
        read_wide_elem(&state, VReg::V8, 1, Vsew::E8) as i16,
        -256i16
    );
}

#[test]
fn vwmacc_mf2_e16_basic() {
    // LMUL=Mf2, SEW=E16: VLMAX = 256/2/16 = 8 elements
    // EMUL_dest = 1, so vd is 1 register wide at E32 (2*SEW)
    let mut state = setup(4, Vsew::E16, Vlmul::Mf2);
    for i in 0..4usize {
        // acc in vd at E32 width
        write_wide_elem(&mut state, VReg::V8, i, Vsew::E16, 100);
        write_elem(&mut state, VReg::V2, i, Vsew::E16, (i + 1) as u64);
        write_elem(&mut state, VReg::V4, i, Vsew::E16, 10);
    }
    exec(
        &mut state,
        Zve64xMulDivInstruction::VwmaccVv {
            vd: VReg::V8,
            vs1: VReg::V2,
            vs2: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        // 100 + (i+1) * 10
        assert_eq!(
            read_wide_elem(&state, VReg::V8, i, Vsew::E16),
            100 + (i + 1) as u64 * 10,
            "elem {i}"
        );
    }
}

// vwmaccsu

#[test]
fn vwmaccsu_vv_e8() {
    // vwmaccsu: vd[i] = vd[i] + sext(vs1[i]) * zext(vs2[i])
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    write_wide_elem(&mut state, VReg::V8, 0, Vsew::E8, 0);
    write_wide_elem(&mut state, VReg::V8, 1, Vsew::E8, 0);
    // vs1=-1 signed
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xFF);
    // vs2=200 unsigned
    write_elem(&mut state, VReg::V4, 0, Vsew::E8, 200);
    // vs1=2
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 2);
    // vs2=200
    write_elem(&mut state, VReg::V4, 1, Vsew::E8, 200);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VwmaccsuVv {
            vd: VReg::V8,
            vs1: VReg::V2,
            vs2: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 0 + (-1) * 200 = -200 as u16 = 0xFF38
    assert_eq!(
        read_wide_elem(&state, VReg::V8, 0, Vsew::E8) as i16,
        -200i16
    );
    // 0 + 2 * 200 = 400
    assert_eq!(read_wide_elem(&state, VReg::V8, 1, Vsew::E8), 400u64);
}

// vwmaccus.vx: vd[i] = vd[i] + zext(rs1) * sext(vs2[i])
// rs1 is UNSIGNED, vs2 is SIGNED.

#[test]
fn vwmaccus_vx_e8() {
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    write_wide_elem(&mut state, VReg::V8, 0, Vsew::E8, 0);
    write_wide_elem(&mut state, VReg::V8, 1, Vsew::E8, 0);
    // vs2=-1 (signed)
    write_elem(&mut state, VReg::V4, 0, Vsew::E8, 0xFF);
    // vs2=50 (signed positive)
    write_elem(&mut state, VReg::V4, 1, Vsew::E8, 50);
    // rs1=255 (unsigned)
    state.regs.write(Reg::A0, 255u64);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VwmaccusVx {
            vd: VReg::V8,
            rs1: Reg::A0,
            vs2: VReg::V4,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 0 + zext(255) * sext(-1) = 255 * (-1) = -255 as i16
    assert_eq!(
        read_wide_elem(&state, VReg::V8, 0, Vsew::E8) as i16,
        -255i16
    );
    // 0 + zext(255) * sext(50) = 255 * 50 = 12750
    assert_eq!(read_wide_elem(&state, VReg::V8, 1, Vsew::E8), 12750u64);
}

// vwmaccsu.vx: vd[i] = vd[i] + sext(rs1) * zext(vs2[i])
// rs1 is SIGNED, vs2 is UNSIGNED.

#[test]
fn vwmaccsu_vx_e8() {
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    write_wide_elem(&mut state, VReg::V8, 0, Vsew::E8, 0);
    write_wide_elem(&mut state, VReg::V8, 1, Vsew::E8, 0);
    // vs2=200 (unsigned)
    write_elem(&mut state, VReg::V4, 0, Vsew::E8, 200);
    // vs2=50 (unsigned)
    write_elem(&mut state, VReg::V4, 1, Vsew::E8, 50);
    // rs1=0xFF stored in register; sign-extends from SEW (8 bits) to -1 signed
    state.regs.write(Reg::A0, 0xFFu64);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VwmaccsuVx {
            vd: VReg::V8,
            rs1: Reg::A0,
            vs2: VReg::V4,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 0 + sext(0xFF=-1) * zext(200) = -1 * 200 = -200 as i16
    assert_eq!(
        read_wide_elem(&state, VReg::V8, 0, Vsew::E8) as i16,
        -200i16
    );
    // 0 + sext(0xFF=-1) * zext(50) = -1 * 50 = -50 as i16
    assert_eq!(read_wide_elem(&state, VReg::V8, 1, Vsew::E8) as i16, -50i16);
}

// common error paths

#[test]
fn vector_instructions_not_allowed() {
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    state.ext_state.set_vector_allowed(false);
    let result = exec(
        &mut state,
        Zve64xMulDivInstruction::VmulVv {
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
fn vtype_not_configured_is_illegal() {
    let mut state = initialize_state::<Zve64xMulDivInstruction<Reg<u64>>, _>([]);
    state.ext_state.init_vector_csrs();
    // vtype left in illegal state (vill=1, no set_vtype called)
    let result = exec(
        &mut state,
        Zve64xMulDivInstruction::VmulVv {
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
fn vd_unaligned_is_illegal() {
    // M2 requires vd to be a multiple of 2; V3 is misaligned
    let mut state = setup(2, Vsew::E32, Vlmul::M2);
    let result = exec(
        &mut state,
        Zve64xMulDivInstruction::VmulVv {
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
fn masked_vd_v0_is_illegal() {
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    // vm=false with vd=V0 is always illegal
    let result = exec(
        &mut state,
        Zve64xMulDivInstruction::VmulVv {
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
fn vstart_respected_for_mul() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, 5);
        write_elem(&mut state, VReg::V4, i, Vsew::E32, 7);
        write_elem(&mut state, VReg::V8, i, Vsew::E32, 0xDEAD);
    }
    // Only elements 2..4 should be processed
    state.ext_state.set_vstart(2);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VmulVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // Elements 0 and 1 untouched
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E32), 0xDEAD);
    assert_eq!(read_elem(&state, VReg::V8, 1, Vsew::E32), 0xDEAD);
    // Elements 2 and 3 written
    assert_eq!(read_elem(&state, VReg::V8, 2, Vsew::E32), 35);
    assert_eq!(read_elem(&state, VReg::V8, 3, Vsew::E32), 35);
    // vstart reset to 0
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vl_zero_writes_nothing() {
    let mut state = setup(0, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V8, 0, Vsew::E32, 0xCAFE);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VmulVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // Nothing written; vd undisturbed
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E32), 0xCAFE);
    // mark_vs_dirty still called
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
}

#[test]
fn widening_mul_illegal_for_sew64() {
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    for instr in [
        Zve64xMulDivInstruction::VwmuluVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
        Zve64xMulDivInstruction::VwmulsuVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
        Zve64xMulDivInstruction::VwmulVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    ] {
        let result = exec(&mut state, instr);
        assert!(
            matches!(result, Err(ExecutionError::IllegalInstruction { .. })),
            "expected illegal for {instr:?}"
        );
    }
}

#[test]
fn widening_muladd_illegal_for_sew64() {
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    for instr in [
        Zve64xMulDivInstruction::VwmaccuVv {
            vd: VReg::V8,
            vs1: VReg::V2,
            vs2: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
        Zve64xMulDivInstruction::VwmaccVv {
            vd: VReg::V8,
            vs1: VReg::V2,
            vs2: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
        Zve64xMulDivInstruction::VwmaccsuVv {
            vd: VReg::V8,
            vs1: VReg::V2,
            vs2: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    ] {
        let result = exec(&mut state, instr);
        assert!(
            matches!(result, Err(ExecutionError::IllegalInstruction { .. })),
            "expected illegal for {instr:?}"
        );
    }
}

#[test]
fn vdivu_e64_div_by_zero() {
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E64, 12345);
    write_elem(&mut state, VReg::V4, 0, Vsew::E64, 0);
    exec(
        &mut state,
        Zve64xMulDivInstruction::VdivuVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E64), u64::MAX);
}

#[test]
fn vdiv_e64_signed_overflow() {
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E64, i64::MIN.cast_unsigned());
    write_elem(&mut state, VReg::V4, 0, Vsew::E64, (-1i64).cast_unsigned());
    exec(
        &mut state,
        Zve64xMulDivInstruction::VdivVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // Spec §12.11: MIN / -1 = MIN
    assert_eq!(
        read_elem(&state, VReg::V8, 0, Vsew::E64),
        i64::MIN.cast_unsigned()
    );
}

#[test]
fn vrem_e64_signed_overflow_returns_zero() {
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E64, i64::MIN.cast_unsigned());
    write_elem(&mut state, VReg::V4, 0, Vsew::E64, (-1i64).cast_unsigned());
    exec(
        &mut state,
        Zve64xMulDivInstruction::VremVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E64), 0);
}

#[test]
fn set_mask_bit_helper_works() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    // Verify the mask helper used in other tests is correct
    for i in 0..8 {
        set_mask_bit(&mut state, i, i % 2 == 0);
    }
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i * 2, Vsew::E8, 10);
        write_elem(&mut state, VReg::V4, i * 2, Vsew::E8, 5);
        write_elem(&mut state, VReg::V2, i * 2 + 1, Vsew::E8, 99);
        write_elem(&mut state, VReg::V4, i * 2 + 1, Vsew::E8, 99);
        write_elem(&mut state, VReg::V8, i * 2, Vsew::E8, 0xAA);
        write_elem(&mut state, VReg::V8, i * 2 + 1, Vsew::E8, 0xBB);
    }
    exec(
        &mut state,
        Zve64xMulDivInstruction::VmulVv {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V4,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        // Even elements (active): 10 * 5 = 50
        assert_eq!(
            read_elem(&state, VReg::V8, i * 2, Vsew::E8),
            50,
            "active elem {}",
            i * 2
        );
        // Odd elements (inactive): undisturbed
        assert_eq!(
            read_elem(&state, VReg::V8, i * 2 + 1, Vsew::E8),
            0xBB,
            "inactive elem {}",
            i * 2 + 1
        );
    }
}

#[test]
fn widening_dest_register_count_values() {
    // EMUL = 2 * LMUL:
    // Mf8 (1/8) -> 2/8 = 1/4 -> 1 reg
    // Mf4 (1/4) -> 2/4 = 1/2 -> 1 reg
    // Mf2 (1/2) -> 2/2 = 1   -> 1 reg
    // M1 (1)    -> 2/1 = 2   -> 2 regs
    // M2 (2)    -> 4/1 = 4   -> 4 regs
    // M4 (4)    -> 8/1 = 8   -> 8 regs
    // M8 (8)    -> 16/1 = 16 -> None (illegal)
    assert_eq!(widening_dest_register_count(Vlmul::Mf8), Some(1));
    assert_eq!(widening_dest_register_count(Vlmul::Mf4), Some(1));
    assert_eq!(widening_dest_register_count(Vlmul::Mf2), Some(1));
    assert_eq!(widening_dest_register_count(Vlmul::M1), Some(2));
    assert_eq!(widening_dest_register_count(Vlmul::M2), Some(4));
    assert_eq!(widening_dest_register_count(Vlmul::M4), Some(8));
    assert_eq!(widening_dest_register_count(Vlmul::M8), None);
}
