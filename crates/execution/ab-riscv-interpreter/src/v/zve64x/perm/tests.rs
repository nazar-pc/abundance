use crate::rv64::test_utils::initialize_state;
use crate::v::vector_registers::{VectorRegisters, VectorRegistersExt};
use crate::{
    ExecutableInstruction, ExecutableInstructionOperands, ExecutionError, RegisterFile,
    Rs1Rs2OperandValues, Rs1Rs2Operands,
};
use ab_riscv_primitives::prelude::*;

// With TEST_VLEN=256, VLENB=32:
//   E8/M1   -> VLMAX=32, 1 reg,  32 elems/reg
//   E16/M1  -> VLMAX=16, 1 reg,  16 elems/reg
//   E32/M1  -> VLMAX=8,  1 reg,  8 elems/reg
//   E64/M1  -> VLMAX=4,  1 reg,  4 elems/reg
//   E8/M2   -> VLMAX=64, 2 regs, 32 elems/reg
//   E16/M2  -> VLMAX=32, 2 regs, 16 elems/reg
//   E32/M2  -> VLMAX=16, 2 regs, 8 elems/reg
//   E64/M2  -> VLMAX=8,  2 regs, 4 elems/reg
//   E64/Mf2 -> VLMAX=2,  1 reg,  4 elems/reg

fn encode_vtype(vsew: Vsew, vlmul: Vlmul) -> u64 {
    u64::from(vlmul.to_bits()) | (u64::from(vsew.to_bits()) << 3)
}

fn setup(
    vl: u32,
    vsew: Vsew,
    vlmul: Vlmul,
) -> crate::rv64::test_utils::TestInterpreterState<Zve64xPermInstruction<Reg<u64>>> {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    let vtype = Vtype::from_raw::<Reg<u64>>(encode_vtype(vsew, vlmul)).unwrap();
    state.ext_state.set_vtype(Some(vtype));
    state.ext_state.set_vl(vl);
    state.ext_state.set_vstart(0);
    state
}

fn exec(
    state: &mut crate::rv64::test_utils::TestInterpreterState<Zve64xPermInstruction<Reg<u64>>>,
    instr: Zve64xPermInstruction<Reg<u64>>,
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
    state: &crate::rv64::test_utils::TestInterpreterState<Zve64xPermInstruction<Reg<u64>>>,
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

fn write_elem(
    state: &mut crate::rv64::test_utils::TestInterpreterState<Zve64xPermInstruction<Reg<u64>>>,
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

fn set_vreg_bytes(
    state: &mut crate::rv64::test_utils::TestInterpreterState<Zve64xPermInstruction<Reg<u64>>>,
    reg: VReg,
    value: u8,
) {
    state.ext_state.write_vregs().get_mut(reg).fill(value);
}

fn get_vreg_bytes(
    state: &crate::rv64::test_utils::TestInterpreterState<Zve64xPermInstruction<Reg<u64>>>,
    reg: VReg,
) -> [u8; 32] {
    *state.ext_state.read_vregs().get(reg)
}

fn set_mask_bit(
    state: &mut crate::rv64::test_utils::TestInterpreterState<Zve64xPermInstruction<Reg<u64>>>,
    reg: VReg,
    i: u32,
    val: bool,
) {
    let byte = &mut state.ext_state.write_vregs().get_mut(reg)[(i / u8::BITS) as usize];
    if val {
        *byte |= 1 << (i % u8::BITS);
    } else {
        *byte &= !(1 << (i % u8::BITS));
    }
}

// vmv.x.s

#[test]
fn vmv_x_s_e8_reads_element_0() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0x42);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0xFF);
    exec(
        &mut state,
        Zve64xPermInstruction::VmvXS {
            rd: Reg::A0,
            vs2: VReg::V2,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0x42u64);
    assert_eq!(state.ext_state.vstart(), 0);
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
}

#[test]
fn vmv_x_s_e8_sign_extends_negative() {
    // 0x80 = -128 as i8; sign-extended to i64 = 0xFFFF_FFFF_FFFF_FF80
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0x80);
    exec(
        &mut state,
        Zve64xPermInstruction::VmvXS {
            rd: Reg::A0,
            vs2: VReg::V2,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0xFFFF_FFFF_FFFF_FF80_u64);
}

#[test]
fn vmv_x_s_e16_sign_extends_negative() {
    // 0x8000 = -32768 as i16; sign-extended = 0xFFFF_FFFF_FFFF_8000
    let mut state = setup(4, Vsew::E16, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E16, 0x8000);
    exec(
        &mut state,
        Zve64xPermInstruction::VmvXS {
            rd: Reg::A0,
            vs2: VReg::V2,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0xFFFF_FFFF_FFFF_8000_u64);
}

#[test]
fn vmv_x_s_e32_sign_extends_negative() {
    // 0x8000_0000 sign-extended to 64 bits = 0xFFFF_FFFF_8000_0000
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 0x8000_0000);
    exec(
        &mut state,
        Zve64xPermInstruction::VmvXS {
            rd: Reg::A0,
            vs2: VReg::V2,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0xFFFF_FFFF_8000_0000_u64);
}

#[test]
fn vmv_x_s_e64_full_width() {
    let mut state = setup(2, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E64, 0xDEAD_BEEF_CAFE_F00D);
    exec(
        &mut state,
        Zve64xPermInstruction::VmvXS {
            rd: Reg::A0,
            vs2: VReg::V2,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0xDEAD_BEEF_CAFE_F00Du64);
}

#[test]
fn vmv_x_s_vl_zero_still_reads() {
    // vmv.x.s reads element 0 regardless of vl.
    let mut state = setup(0, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 0x1234_5678);
    exec(
        &mut state,
        Zve64xPermInstruction::VmvXS {
            rd: Reg::A0,
            vs2: VReg::V2,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0x1234_5678u64);
}

#[test]
fn vmv_x_s_illegal_when_vector_disabled() {
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    state.ext_state.set_vector_allowed(false);
    let err = exec(
        &mut state,
        Zve64xPermInstruction::VmvXS {
            rd: Reg::A0,
            vs2: VReg::V2,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::IllegalInstruction { .. }));
}

#[test]
fn vmv_x_s_illegal_when_vtype_invalid() {
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    state.ext_state.set_vtype(None);
    let err = exec(
        &mut state,
        Zve64xPermInstruction::VmvXS {
            rd: Reg::A0,
            vs2: VReg::V2,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::IllegalInstruction { .. }));
}

// vmv.s.x

#[test]
fn vmv_s_x_e8_writes_element_0() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    state.regs.write(Reg::A0, 0xAB);
    set_vreg_bytes(&mut state, VReg::V4, 0xFF);
    exec(
        &mut state,
        Zve64xPermInstruction::VmvSX {
            vd: VReg::V4,
            rs1: Reg::A0,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 0xAB);
    // Elements 1..16 must be undisturbed.
    for i in 1..16 {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0xFF, "elem {i}");
    }
    assert_eq!(state.ext_state.vstart(), 0);
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
}

#[test]
fn vmv_s_x_e64_writes_element_0() {
    let mut state = setup(2, Vsew::E64, Vlmul::M1);
    state.regs.write(Reg::A1, 0x0102_0304_0506_0708u64);
    exec(
        &mut state,
        Zve64xPermInstruction::VmvSX {
            vd: VReg::V4,
            rs1: Reg::A1,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(
        read_elem(&state, VReg::V4, 0, Vsew::E64),
        0x0102_0304_0506_0708u64
    );
}

#[test]
fn vmv_s_x_vl_zero_suppresses_write() {
    // When vl == 0 the destination must not be updated.
    let mut state = setup(0, Vsew::E32, Vlmul::M1);
    set_vreg_bytes(&mut state, VReg::V4, 0xCC);
    state.regs.write(Reg::A0, 0x1234_5678);
    exec(
        &mut state,
        Zve64xPermInstruction::VmvSX {
            vd: VReg::V4,
            rs1: Reg::A0,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // Element 0 (4 bytes 0xCC in little-endian) = 0xCCCC_CCCC
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0xCCCC_CCCCu64);
}

#[test]
fn vmv_s_x_truncates_to_sew() {
    // Only the low SEW bits of rs1 are written.
    let mut state = setup(1, Vsew::E8, Vlmul::M1);
    state.regs.write(Reg::A0, 0xABCD);
    exec(
        &mut state,
        Zve64xPermInstruction::VmvSX {
            vd: VReg::V4,
            rs1: Reg::A0,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 0xCD);
}

#[test]
fn vmv_s_x_illegal_when_vector_disabled() {
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    state.ext_state.set_vector_allowed(false);
    let err = exec(
        &mut state,
        Zve64xPermInstruction::VmvSX {
            vd: VReg::V4,
            rs1: Reg::A0,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::IllegalInstruction { .. }));
}

#[test]
fn vmv_s_x_vstart_ge_vl_suppresses_write() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    set_vreg_bytes(&mut state, VReg::V4, 0xAA);
    state.regs.write(Reg::A0, 0x1234_5678);
    state.ext_state.set_vstart(4);
    exec(
        &mut state,
        Zve64xPermInstruction::VmvSX {
            vd: VReg::V4,
            rs1: Reg::A0,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0xAAAA_AAAA);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vmv_s_x_vstart_nonzero_below_vl_still_writes() {
    // vstart=1, vl=4: vstart < vl, so write proceeds (spec says element 0 is updated).
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    state.regs.write(Reg::A0, 0x1234_5678);
    state.ext_state.set_vstart(1);
    exec(
        &mut state,
        Zve64xPermInstruction::VmvSX {
            vd: VReg::V4,
            rs1: Reg::A0,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0x1234_5678);
}

// vslideup

#[test]
fn vslideup_vx_e8_basic() {
    // vslideup by 2: vd[0..2] unchanged, vd[i] = vs2[i-2] for i in 2..8.
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    for i in 0..8usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, (i + 1) as u64);
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 0xDD);
    }
    state.regs.write(Reg::A0, 2u64);
    exec(
        &mut state,
        Zve64xPermInstruction::VslideupVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 0xDD);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E8), 0xDD);
    for i in 2..8usize {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E8),
            (i - 1) as u64,
            "elem {i}"
        );
    }
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vslideup_vi_e32_basic() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, (i * 100) as u64);
        write_elem(&mut state, VReg::V4, i, Vsew::E32, 0xFFFF_FFFF);
    }
    exec(
        &mut state,
        Zve64xPermInstruction::VslideupVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            uimm: 1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0xFFFF_FFFF);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E32), 0);
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E32), 100);
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E32), 200);
}

#[test]
fn vslideup_vx_offset_zero_copies_all() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, (i + 1) as u64);
    }
    state.regs.write(Reg::A0, 0u64);
    exec(
        &mut state,
        Zve64xPermInstruction::VslideupVx {
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
            (i + 1) as u64,
            "elem {i}"
        );
    }
}

#[test]
fn vslideup_vx_offset_ge_vl_no_write() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V4, i, Vsew::E32, 0xBEEF);
    }
    state.regs.write(Reg::A0, 4u64);
    exec(
        &mut state,
        Zve64xPermInstruction::VslideupVx {
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
            0xBEEF,
            "elem {i}"
        );
    }
}

#[test]
fn vslideup_vx_masked() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    for i in 0..8usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, (i + 1) as u64);
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 0xAA);
    }
    // Active bits: 2, 4, 6
    state.ext_state.write_vregs().get_mut(VReg::V0).fill(0);
    set_mask_bit(&mut state, VReg::V0, 2, true);
    set_mask_bit(&mut state, VReg::V0, 4, true);
    set_mask_bit(&mut state, VReg::V0, 6, true);
    state.regs.write(Reg::A0, 2u64);
    exec(
        &mut state,
        Zve64xPermInstruction::VslideupVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: false,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E8), 1);
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E8), 0xAA);
    assert_eq!(read_elem(&state, VReg::V4, 4, Vsew::E8), 3);
    assert_eq!(read_elem(&state, VReg::V4, 6, Vsew::E8), 5);
}

#[test]
fn vslideup_overlap_vd_vs2_illegal() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    state.regs.write(Reg::A0, 1u64);
    let err = exec(
        &mut state,
        Zve64xPermInstruction::VslideupVx {
            vd: VReg::V2,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::IllegalInstruction { .. }));
}

#[test]
fn vslideup_masked_vd_v0_illegal() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    state.regs.write(Reg::A0, 1u64);
    let err = exec(
        &mut state,
        Zve64xPermInstruction::VslideupVx {
            vd: VReg::V0,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: false,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::IllegalInstruction { .. }));
}

#[test]
fn vslideup_vstart_skips_lower_elements() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    for i in 0..8usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, (i + 10) as u64);
        write_elem(&mut state, VReg::V4, i, Vsew::E8, 0xBB);
    }
    state.ext_state.set_vstart(3);
    state.regs.write(Reg::A0, 2u64);
    exec(
        &mut state,
        Zve64xPermInstruction::VslideupVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // Elements 0..3 undisturbed (before vstart).
    for i in 0..3usize {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0xBB, "elem {i}");
    }
    // Elements 3..8: vd[i] = vs2[i-2]
    for i in 3..8usize {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E8),
            (i - 2 + 10) as u64,
            "elem {i}"
        );
    }
    assert_eq!(state.ext_state.vstart(), 0);
}

// vslidedown

#[test]
fn vslidedown_vx_e8_basic() {
    let mut state = setup(6, Vsew::E8, Vlmul::M1);
    for i in 0..16usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, (i + 1) as u64);
    }
    state.regs.write(Reg::A0, 2u64);
    exec(
        &mut state,
        Zve64xPermInstruction::VslidedownVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..6usize {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E8),
            (i + 3) as u64,
            "elem {i}"
        );
    }
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vslidedown_vi_e32_fills_zeros_past_end() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, (i + 1) as u64);
    }
    // Offset 4 == VLMAX: all source indices out of range.
    exec(
        &mut state,
        Zve64xPermInstruction::VslidedownVi {
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
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E32), 0, "elem {i}");
    }
}

#[test]
fn vslidedown_vx_partial_fill() {
    // Offset 2, vl 4, vlmax 4: elements 0..2 get vs2[2..4], elements 2..4 get 0.
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, ((i + 1) * 10) as u64);
    }
    state.regs.write(Reg::A0, 2u64);
    exec(
        &mut state,
        Zve64xPermInstruction::VslidedownVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 30);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E32), 40);
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E32), 0);
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E32), 0);
}

#[test]
fn vslidedown_vx_offset_zero_is_copy() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, (i + 1) as u64);
    }
    state.regs.write(Reg::A0, 0u64);
    exec(
        &mut state,
        Zve64xPermInstruction::VslidedownVx {
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
            (i + 1) as u64,
            "elem {i}"
        );
    }
}

#[test]
fn vslidedown_overlap_allowed() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, ((i + 1) * 10) as u64);
    }
    state.regs.write(Reg::A0, 1u64);
    exec(
        &mut state,
        Zve64xPermInstruction::VslidedownVx {
            vd: VReg::V2,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V2, 0, Vsew::E32), 20);
    assert_eq!(read_elem(&state, VReg::V2, 1, Vsew::E32), 30);
    assert_eq!(read_elem(&state, VReg::V2, 2, Vsew::E32), 40);
    assert_eq!(read_elem(&state, VReg::V2, 3, Vsew::E32), 0);
}

#[test]
fn vslidedown_masked() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, ((i + 1) * 100) as u64);
        write_elem(&mut state, VReg::V4, i, Vsew::E32, 0xDEAD);
    }
    state.ext_state.write_vregs().get_mut(VReg::V0).fill(0);
    set_mask_bit(&mut state, VReg::V0, 0, true);
    set_mask_bit(&mut state, VReg::V0, 2, true);
    state.regs.write(Reg::A0, 1u64);
    exec(
        &mut state,
        Zve64xPermInstruction::VslidedownVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: false,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 200);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E32), 0xDEAD);
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E32), 400);
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E32), 0xDEAD);
}

// vslide1up

#[test]
fn vslide1up_vx_e32_basic() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, ((i + 1) * 10) as u64);
    }
    state.regs.write(Reg::A0, 99u64);
    exec(
        &mut state,
        Zve64xPermInstruction::Vslide1upVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 99);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E32), 10);
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E32), 20);
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E32), 30);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vslide1up_vx_e64_scalar_inserted() {
    let mut state = setup(2, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E64, 0xAAAA_AAAA_AAAA_AAAA);
    write_elem(&mut state, VReg::V2, 1, Vsew::E64, 0xBBBB_BBBB_BBBB_BBBB);
    state.regs.write(Reg::A0, 0x1234_5678_9ABC_DEF0u64);
    exec(
        &mut state,
        Zve64xPermInstruction::Vslide1upVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(
        read_elem(&state, VReg::V4, 0, Vsew::E64),
        0x1234_5678_9ABC_DEF0
    );
    assert_eq!(
        read_elem(&state, VReg::V4, 1, Vsew::E64),
        0xAAAA_AAAA_AAAA_AAAA
    );
}

#[test]
fn vslide1up_overlap_vd_vs2_illegal() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    let err = exec(
        &mut state,
        Zve64xPermInstruction::Vslide1upVx {
            vd: VReg::V2,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::IllegalInstruction { .. }));
}

#[test]
fn vslide1up_masked() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, ((i + 1) * 10) as u64);
        write_elem(&mut state, VReg::V4, i, Vsew::E32, 0xDEAD);
    }
    state.ext_state.write_vregs().get_mut(VReg::V0).fill(0);
    set_mask_bit(&mut state, VReg::V0, 0, true);
    set_mask_bit(&mut state, VReg::V0, 2, true);
    state.regs.write(Reg::A0, 99u64);
    exec(
        &mut state,
        Zve64xPermInstruction::Vslide1upVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: false,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 99);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E32), 0xDEAD);
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E32), 20);
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E32), 0xDEAD);
}

// vslide1down

#[test]
fn vslide1down_vx_e32_basic() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, ((i + 1) * 10) as u64);
    }
    state.regs.write(Reg::A0, 999u64);
    exec(
        &mut state,
        Zve64xPermInstruction::Vslide1downVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 20);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E32), 30);
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E32), 40);
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E32), 999);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vslide1down_vx_e64_basic() {
    let mut state = setup(2, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E64, 0xAAAA_AAAA);
    write_elem(&mut state, VReg::V2, 1, Vsew::E64, 0xBBBB_BBBB);
    state.regs.write(Reg::A0, 0xCCCC_CCCCu64);
    exec(
        &mut state,
        Zve64xPermInstruction::Vslide1downVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E64), 0xBBBB_BBBB);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E64), 0xCCCC_CCCC);
}

#[test]
fn vslide1down_vl_one_only_scalar() {
    let mut state = setup(1, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 0xDEAD_BEEF);
    state.regs.write(Reg::A0, 42u64);
    exec(
        &mut state,
        Zve64xPermInstruction::Vslide1downVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 42);
}

#[test]
fn vslide1down_overlap_allowed() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, ((i + 1) * 10) as u64);
    }
    state.regs.write(Reg::A0, 50u64);
    exec(
        &mut state,
        Zve64xPermInstruction::Vslide1downVx {
            vd: VReg::V2,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V2, 0, Vsew::E32), 20);
    assert_eq!(read_elem(&state, VReg::V2, 1, Vsew::E32), 30);
    assert_eq!(read_elem(&state, VReg::V2, 2, Vsew::E32), 40);
    assert_eq!(read_elem(&state, VReg::V2, 3, Vsew::E32), 50);
}

#[test]
fn vslide1down_masked() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, ((i + 1) * 10) as u64);
        write_elem(&mut state, VReg::V4, i, Vsew::E32, 0xFFFF);
    }
    state.ext_state.write_vregs().get_mut(VReg::V0).fill(0);
    set_mask_bit(&mut state, VReg::V0, 1, true);
    set_mask_bit(&mut state, VReg::V0, 3, true);
    state.regs.write(Reg::A0, 77u64);
    exec(
        &mut state,
        Zve64xPermInstruction::Vslide1downVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: false,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0xFFFF);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E32), 30);
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E32), 0xFFFF);
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E32), 77);
}

// vrgather.vv

#[test]
fn vrgather_vv_e8_basic() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..16usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, ((i + 1) * 10) as u64);
    }
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 3);
    write_elem(&mut state, VReg::V1, 1, Vsew::E8, 0);
    write_elem(&mut state, VReg::V1, 2, Vsew::E8, 2);
    write_elem(&mut state, VReg::V1, 3, Vsew::E8, 1);
    exec(
        &mut state,
        Zve64xPermInstruction::VrgatherVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 40);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E8), 10);
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E8), 30);
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E8), 20);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vrgather_vv_index_out_of_range_gives_zero() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, (i + 1) as u64);
        write_elem(&mut state, VReg::V1, i, Vsew::E32, 100);
    }
    exec(
        &mut state,
        Zve64xPermInstruction::VrgatherVv {
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
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E32), 0, "elem {i}");
    }
}

#[test]
fn vrgather_vv_vd_overlap_vs2_illegal() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    let err = exec(
        &mut state,
        Zve64xPermInstruction::VrgatherVv {
            vd: VReg::V2,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::IllegalInstruction { .. }));
}

#[test]
fn vrgather_vv_vd_overlap_vs1_illegal() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    let err = exec(
        &mut state,
        Zve64xPermInstruction::VrgatherVv {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::IllegalInstruction { .. }));
}

#[test]
fn vrgather_vv_masked() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, ((i + 1) * 100) as u64);
        write_elem(&mut state, VReg::V1, i, Vsew::E32, (3 - i) as u64);
        write_elem(&mut state, VReg::V4, i, Vsew::E32, 0xABCD);
    }
    state.ext_state.write_vregs().get_mut(VReg::V0).fill(0);
    set_mask_bit(&mut state, VReg::V0, 0, true);
    set_mask_bit(&mut state, VReg::V0, 3, true);
    exec(
        &mut state,
        Zve64xPermInstruction::VrgatherVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 400);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E32), 0xABCD);
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E32), 0xABCD);
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E32), 100);
}

// vrgather.vx / vrgather.vi

#[test]
fn vrgather_vx_e32_all_same_element() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, ((i + 1) * 11) as u64);
    }
    state.regs.write(Reg::A0, 2u64);
    exec(
        &mut state,
        Zve64xPermInstruction::VrgatherVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E32), 33, "elem {i}");
    }
}

#[test]
fn vrgather_vx_index_out_of_range_gives_zero() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, (i + 1) as u64);
    }
    state.regs.write(Reg::A0, 99u64);
    exec(
        &mut state,
        Zve64xPermInstruction::VrgatherVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E32), 0, "elem {i}");
    }
}

#[test]
fn vrgather_vi_e8_immediate_index() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    for i in 0..16usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, (i * 3) as u64);
    }
    exec(
        &mut state,
        Zve64xPermInstruction::VrgatherVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            uimm: 5,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..8usize {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 15, "elem {i}");
    }
}

#[test]
fn vrgather_vi_index_zero() {
    let mut state = setup(4, Vsew::E16, Vlmul::M1);
    for i in 0..8usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E16, (i * 7) as u64);
    }
    exec(
        &mut state,
        Zve64xPermInstruction::VrgatherVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            uimm: 0,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E16), 0, "elem {i}");
    }
}

#[test]
fn vrgather_vx_vd_overlap_vs2_illegal() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    state.regs.write(Reg::A0, 0u64);
    let err = exec(
        &mut state,
        Zve64xPermInstruction::VrgatherVx {
            vd: VReg::V2,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::IllegalInstruction { .. }));
}

// vrgatherei16.vv

#[test]
fn vrgatherei16_vv_e8_m1_basic() {
    // SEW=8, LMUL=1: EMUL_vs1 = (16/8)*1 = 2. vs1 must be aligned to 2.
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    for i in 0..16usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, ((i + 1) * 5) as u64);
    }
    // Write 16-bit indices into V6,V7 (two registers, EMUL=2): [7,2,0,15,1,14,3,13]
    let indices: [u16; 8] = [7, 2, 0, 15, 1, 14, 3, 13];
    for (i, &idx) in indices.iter().enumerate() {
        let byte_off = i * 2;
        let reg_off = byte_off / 16;
        let b = byte_off % 16;
        let bytes = idx.to_le_bytes();
        state
            .ext_state
            .write_vregs()
            .get_mut(VReg::from_bits(VReg::V6.to_bits() + reg_off as u8).unwrap())[b] = bytes[0];
        state
            .ext_state
            .write_vregs()
            .get_mut(VReg::from_bits(VReg::V6.to_bits() + reg_off as u8).unwrap())[b + 1] =
            bytes[1];
    }
    exec(
        &mut state,
        Zve64xPermInstruction::Vrgatherei16Vv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V6,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    let expected: [u64; 8] = [40, 15, 5, 80, 10, 75, 20, 70];
    for (i, &exp) in expected.iter().enumerate() {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), exp, "elem {i}");
    }
}

#[test]
fn vrgatherei16_vv_index_out_of_range_gives_zero() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, (i + 1) as u64);
    }
    // EMUL_vs1 = (16/32)*1 = 1/2 -> 1 register. Place out-of-range index 100 everywhere.
    for i in 0..8usize {
        let byte_off = i * 2;
        let bytes = 100u16.to_le_bytes();
        state.ext_state.write_vregs().get_mut(VReg::V6)[byte_off] = bytes[0];
        state.ext_state.write_vregs().get_mut(VReg::V6)[byte_off + 1] = bytes[1];
    }
    exec(
        &mut state,
        Zve64xPermInstruction::Vrgatherei16Vv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V6,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E32), 0, "elem {i}");
    }
}

#[test]
fn vrgatherei16_vv_vd_overlap_vs2_illegal() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    let err = exec(
        &mut state,
        Zve64xPermInstruction::Vrgatherei16Vv {
            vd: VReg::V2,
            vs2: VReg::V2,
            vs1: VReg::V6,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::IllegalInstruction { .. }));
}

// vmerge.vvm / vmv.v.v

#[test]
fn vmv_v_v_broadcasts_all_elements() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V1, i, Vsew::E32, ((i + 1) * 10) as u64);
    }
    exec(
        &mut state,
        Zve64xPermInstruction::VmergeVvm {
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
            ((i + 1) * 10) as u64,
            "elem {i}"
        );
    }
    assert_eq!(state.ext_state.vstart(), 0);
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
}

#[test]
fn vmv_v_v_vl_zero_leaves_vd_undisturbed() {
    let mut state = setup(0, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V1, i, Vsew::E32, 0xABCD);
        write_elem(&mut state, VReg::V4, i, Vsew::E32, 0xDEAD);
    }
    exec(
        &mut state,
        Zve64xPermInstruction::VmergeVvm {
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
            0xDEAD,
            "elem {i}"
        );
    }
}

#[test]
fn vmerge_vvm_blends_vs2_and_vs1() {
    // Mask v0 = 0b1010 -> elements 1 and 3 from vs1, elements 0 and 2 from vs2.
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, (i * 100) as u64);
        write_elem(&mut state, VReg::V1, i, Vsew::E32, (i * 10 + 1) as u64);
    }
    state.ext_state.write_vregs().get_mut(VReg::V0).fill(0);
    set_mask_bit(&mut state, VReg::V0, 1, true);
    set_mask_bit(&mut state, VReg::V0, 3, true);
    exec(
        &mut state,
        Zve64xPermInstruction::VmergeVvm {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // v0[0]=0 -> vs2[0]=0
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0);
    // v0[1]=1 -> vs1[1]=11
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E32), 11);
    // v0[2]=0 -> vs2[2]=200
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E32), 200);
    // v0[3]=1 -> vs1[3]=31
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E32), 31);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vmerge_vvm_all_mask_bits_set_equals_vmv_v_v() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, 0xDEAD);
        write_elem(&mut state, VReg::V1, i, Vsew::E32, (i + 1) as u64);
    }
    state.ext_state.write_vregs().get_mut(VReg::V0).fill(0xFF);
    exec(
        &mut state,
        Zve64xPermInstruction::VmergeVvm {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E32),
            (i + 1) as u64,
            "elem {i}"
        );
    }
}

#[test]
fn vmerge_vvm_all_mask_bits_clear_equals_copy_vs2() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, ((i + 1) * 7) as u64);
        write_elem(&mut state, VReg::V1, i, Vsew::E32, 0xDEAD);
    }
    state.ext_state.write_vregs().get_mut(VReg::V0).fill(0x00);
    exec(
        &mut state,
        Zve64xPermInstruction::VmergeVvm {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E32),
            ((i + 1) * 7) as u64,
            "elem {i}"
        );
    }
}

#[test]
fn vmerge_vvm_vd_overlap_v0_illegal() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    let err = exec(
        &mut state,
        Zve64xPermInstruction::VmergeVvm {
            vd: VReg::V0,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::IllegalInstruction { .. }));
}

#[test]
fn vmv_v_v_vd_may_equal_v0() {
    // vmv.v.v (vm=true) has no restriction on vd, including vd=v0.
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V1, i, Vsew::E32, (i + 1) as u64);
    }
    exec(
        &mut state,
        Zve64xPermInstruction::VmergeVvm {
            vd: VReg::V0,
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
            read_elem(&state, VReg::V0, i, Vsew::E32),
            (i + 1) as u64,
            "elem {i}"
        );
    }
}

#[test]
fn vmerge_vvm_vstart_skips_early_elements() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, (i * 100) as u64);
        write_elem(&mut state, VReg::V1, i, Vsew::E32, (i * 10 + 1) as u64);
        write_elem(&mut state, VReg::V4, i, Vsew::E32, 0xBEEF);
    }
    state.ext_state.write_vregs().get_mut(VReg::V0).fill(0xFF);
    state.ext_state.set_vstart(2);
    exec(
        &mut state,
        Zve64xPermInstruction::VmergeVvm {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // Elements 0..2: undisturbed.
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0xBEEF);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E32), 0xBEEF);
    // Elements 2..4: mask=1 so from vs1.
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E32), 21);
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E32), 31);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vmerge_vvm_e8_full_register() {
    let mut state = setup(16, Vsew::E8, Vlmul::M1);
    // Even indices from vs2, odd indices from vs1.
    for i in 0..16usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, (i * 2) as u64);
        write_elem(&mut state, VReg::V1, i, Vsew::E8, (i * 2 + 1) as u64);
    }
    // Mask: odd bits set (0b10101010_10101010).
    state.ext_state.write_vregs().get_mut(VReg::V0)[0] = 0b1010_1010;
    state.ext_state.write_vregs().get_mut(VReg::V0)[1] = 0b1010_1010;
    exec(
        &mut state,
        Zve64xPermInstruction::VmergeVvm {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..16usize {
        let expected = if i % 2 == 1 { i * 2 + 1 } else { i * 2 } as u64;
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E8),
            expected,
            "elem {i}"
        );
    }
}

// vmerge.vxm / vmv.v.x

#[test]
fn vmv_v_x_broadcasts_scalar() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    state.regs.write(Reg::A0, 0x1234_5678u64);
    exec(
        &mut state,
        Zve64xPermInstruction::VmergeVxm {
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
            0x1234_5678,
            "elem {i}"
        );
    }
    assert_eq!(state.ext_state.vstart(), 0);
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
}

#[test]
fn vmv_v_x_e64_full_width() {
    let mut state = setup(2, Vsew::E64, Vlmul::M1);
    state.regs.write(Reg::A0, 0xDEAD_BEEF_CAFE_F00Du64);
    exec(
        &mut state,
        Zve64xPermInstruction::VmergeVxm {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(
        read_elem(&state, VReg::V4, 0, Vsew::E64),
        0xDEAD_BEEF_CAFE_F00D
    );
    assert_eq!(
        read_elem(&state, VReg::V4, 1, Vsew::E64),
        0xDEAD_BEEF_CAFE_F00D
    );
}

#[test]
fn vmv_v_x_truncates_to_sew() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    state.regs.write(Reg::A0, 0xABCD_EF01u64);
    exec(
        &mut state,
        Zve64xPermInstruction::VmergeVxm {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0x01, "elem {i}");
    }
}

#[test]
fn vmerge_vxm_blends_vs2_and_scalar() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, (i * 100) as u64);
    }
    state.regs.write(Reg::A0, 999u64);
    state.ext_state.write_vregs().get_mut(VReg::V0).fill(0);
    set_mask_bit(&mut state, VReg::V0, 0, true);
    set_mask_bit(&mut state, VReg::V0, 2, true);
    exec(
        &mut state,
        Zve64xPermInstruction::VmergeVxm {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: false,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // v0[0]=1 -> scalar 999
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 999);
    // v0[1]=0 -> vs2[1]=100
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E32), 100);
    // v0[2]=1 -> scalar 999
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E32), 999);
    // v0[3]=0 -> vs2[3]=300
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E32), 300);
}

#[test]
fn vmerge_vxm_vd_overlap_v0_illegal() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    let err = exec(
        &mut state,
        Zve64xPermInstruction::VmergeVxm {
            vd: VReg::V0,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: false,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::IllegalInstruction { .. }));
}

#[test]
fn vmv_v_x_vl_zero_leaves_vd_undisturbed() {
    let mut state = setup(0, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V4, i, Vsew::E32, 0xFACE);
    }
    state.regs.write(Reg::A0, 0x1234u64);
    exec(
        &mut state,
        Zve64xPermInstruction::VmergeVxm {
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
            0xFACE,
            "elem {i}"
        );
    }
}

// vmerge.vim / vmv.v.i

#[test]
fn vmv_v_i_broadcasts_positive_immediate() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    exec(
        &mut state,
        Zve64xPermInstruction::VmergeVim {
            vd: VReg::V4,
            vs2: VReg::V2,
            simm5: 15,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E32), 15, "elem {i}");
    }
    assert_eq!(state.ext_state.vstart(), 0);
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
}

#[test]
fn vmv_v_i_sign_extends_negative_immediate() {
    // simm5 = -1 (0b11111) sign-extended to 32 bits = 0xFFFF_FFFF.
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    exec(
        &mut state,
        Zve64xPermInstruction::VmergeVim {
            vd: VReg::V4,
            vs2: VReg::V2,
            simm5: -1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E32),
            0xFFFF_FFFF,
            "elem {i}"
        );
    }
}

#[test]
fn vmv_v_i_sign_extends_negative_e64() {
    // simm5 = -1 sign-extended to 64 bits = 0xFFFF_FFFF_FFFF_FFFF.
    let mut state = setup(2, Vsew::E64, Vlmul::M1);
    exec(
        &mut state,
        Zve64xPermInstruction::VmergeVim {
            vd: VReg::V4,
            vs2: VReg::V2,
            simm5: -1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(
        read_elem(&state, VReg::V4, 0, Vsew::E64),
        0xFFFF_FFFF_FFFF_FFFF
    );
    assert_eq!(
        read_elem(&state, VReg::V4, 1, Vsew::E64),
        0xFFFF_FFFF_FFFF_FFFF
    );
}

#[test]
fn vmv_v_i_negative_imm_truncated_to_sew_e8() {
    // simm5 = -1 = 0xFF_FF...FF; low 8 bits = 0xFF.
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    exec(
        &mut state,
        Zve64xPermInstruction::VmergeVim {
            vd: VReg::V4,
            vs2: VReg::V2,
            simm5: -1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0xFF, "elem {i}");
    }
}

#[test]
fn vmerge_vim_blends_vs2_and_immediate() {
    let mut state = setup(4, Vsew::E16, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E16, (i * 1000) as u64);
    }
    state.ext_state.write_vregs().get_mut(VReg::V0).fill(0);
    set_mask_bit(&mut state, VReg::V0, 1, true);
    set_mask_bit(&mut state, VReg::V0, 3, true);
    exec(
        &mut state,
        Zve64xPermInstruction::VmergeVim {
            vd: VReg::V4,
            vs2: VReg::V2,
            simm5: 7,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // v0[0]=0 -> vs2[0]=0
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E16), 0);
    // v0[1]=1 -> imm=7
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E16), 7);
    // v0[2]=0 -> vs2[2]=2000
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E16), 2000);
    // v0[3]=1 -> imm=7
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E16), 7);
}

#[test]
fn vmerge_vim_vd_overlap_v0_illegal() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    let err = exec(
        &mut state,
        Zve64xPermInstruction::VmergeVim {
            vd: VReg::V0,
            vs2: VReg::V2,
            simm5: 1,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::IllegalInstruction { .. }));
}

#[test]
fn vmv_v_i_vd_may_equal_v0_when_unmasked() {
    // vmv.v.i (vm=true): no restriction on vd.
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    exec(
        &mut state,
        Zve64xPermInstruction::VmergeVim {
            vd: VReg::V0,
            vs2: VReg::V2,
            simm5: 5,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        assert_eq!(read_elem(&state, VReg::V0, i, Vsew::E32), 5, "elem {i}");
    }
}

#[test]
fn vmerge_vim_vstart_skips_early_elements() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, (i * 100) as u64);
        write_elem(&mut state, VReg::V4, i, Vsew::E32, 0xABCD);
    }
    state.ext_state.write_vregs().get_mut(VReg::V0).fill(0xFF);
    state.ext_state.set_vstart(2);
    exec(
        &mut state,
        Zve64xPermInstruction::VmergeVim {
            vd: VReg::V4,
            vs2: VReg::V2,
            simm5: 42,
            vm: false,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0xABCD);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E32), 0xABCD);
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E32), 42);
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E32), 42);
    assert_eq!(state.ext_state.vstart(), 0);
}

// Vector-disabled / vtype-invalid

#[test]
fn vmerge_variants_illegal_when_vector_disabled() {
    let instrs: &[(Zve64xPermInstruction<Reg<u64>>, &str)] = &[
        (
            Zve64xPermInstruction::VmergeVvm {
                vd: VReg::V4,
                vs2: VReg::V2,
                vs1: VReg::V1,
                vm: true,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "VmergeVvm",
        ),
        (
            Zve64xPermInstruction::VmergeVxm {
                vd: VReg::V4,
                vs2: VReg::V2,
                rs1: Reg::A0,
                vm: true,
                rs2: Reg::Zero,
            },
            "VmergeVxm",
        ),
        (
            Zve64xPermInstruction::VmergeVim {
                vd: VReg::V4,
                vs2: VReg::V2,
                simm5: 1,
                vm: true,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "VmergeVim",
        ),
    ];
    for (instr, name) in instrs {
        let mut state = setup(4, Vsew::E32, Vlmul::M1);
        state.ext_state.set_vector_allowed(false);
        let err = exec(&mut state, *instr).unwrap_err();
        assert!(
            matches!(err, ExecutionError::IllegalInstruction { .. }),
            "expected IllegalInstruction for {name}"
        );
    }
}

#[test]
fn vmerge_variants_illegal_when_vtype_invalid() {
    let instrs: &[(Zve64xPermInstruction<Reg<u64>>, &str)] = &[
        (
            Zve64xPermInstruction::VmergeVvm {
                vd: VReg::V4,
                vs2: VReg::V2,
                vs1: VReg::V1,
                vm: true,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "VmergeVvm",
        ),
        (
            Zve64xPermInstruction::VmergeVxm {
                vd: VReg::V4,
                vs2: VReg::V2,
                rs1: Reg::A0,
                vm: true,
                rs2: Reg::Zero,
            },
            "VmergeVxm",
        ),
        (
            Zve64xPermInstruction::VmergeVim {
                vd: VReg::V4,
                vs2: VReg::V2,
                simm5: 1,
                vm: true,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "VmergeVim",
        ),
    ];
    for (instr, name) in instrs {
        let mut state = setup(4, Vsew::E32, Vlmul::M1);
        state.ext_state.set_vtype(None);
        let err = exec(&mut state, *instr).unwrap_err();
        assert!(
            matches!(err, ExecutionError::IllegalInstruction { .. }),
            "expected IllegalInstruction for {name}"
        );
    }
}

#[test]
fn vmerge_variants_reset_vstart_and_mark_dirty() {
    let instrs: &[(Zve64xPermInstruction<Reg<u64>>, &str)] = &[
        (
            Zve64xPermInstruction::VmergeVvm {
                vd: VReg::V4,
                vs2: VReg::V2,
                vs1: VReg::V1,
                vm: true,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "VmergeVvm",
        ),
        (
            Zve64xPermInstruction::VmergeVxm {
                vd: VReg::V4,
                vs2: VReg::V2,
                rs1: Reg::A0,
                vm: true,
                rs2: Reg::Zero,
            },
            "VmergeVxm",
        ),
        (
            Zve64xPermInstruction::VmergeVim {
                vd: VReg::V4,
                vs2: VReg::V2,
                simm5: 1,
                vm: true,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "VmergeVim",
        ),
    ];
    for (instr, name) in instrs {
        let mut state = setup(4, Vsew::E32, Vlmul::M1);
        for i in 0..4usize {
            write_elem(&mut state, VReg::V1, i, Vsew::E32, (i + 1) as u64);
        }
        state.regs.write(Reg::A0, 99u64);
        state.ext_state.set_vstart(2);
        let before = state.ext_state.vs_dirty_count();
        exec(&mut state, *instr).unwrap();
        assert_eq!(state.ext_state.vstart(), 0, "vstart not reset for {name}");
        assert_eq!(
            state.ext_state.vs_dirty_count(),
            before + 1,
            "vs_dirty not incremented for {name}"
        );
    }
}

// LMUL > 1

#[test]
fn vmv_v_x_m2_e32_broadcasts_across_group() {
    // E32/M2: VLMAX=16, vl=16 spans V4 and V5 (8 E32 elems per VLENB=32 register)
    let mut state = setup(16, Vsew::E32, Vlmul::M2);
    state.regs.write(Reg::A0, 0xCAFEu64);
    exec(
        &mut state,
        Zve64xPermInstruction::VmergeVxm {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..16usize {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E32),
            0xCAFE,
            "elem {i}"
        );
    }
}

#[test]
fn vmerge_vxm_m2_e32_blends_across_group() {
    // E32/M2: VLMAX=16, vl=16 spans V4 and V5 (8 E32 elems per VLENB=32 register)
    let mut state = setup(16, Vsew::E32, Vlmul::M2);
    for i in 0..16usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, (i * 100) as u64);
    }
    state.regs.write(Reg::A0, 777u64);
    // Set even mask bits across both bytes: elements 0,2,..,14 active.
    state.ext_state.write_vregs().get_mut(VReg::V0).fill(0);
    state.ext_state.write_vregs().get_mut(VReg::V0)[0] = 0b0101_0101;
    state.ext_state.write_vregs().get_mut(VReg::V0)[1] = 0b0101_0101;
    exec(
        &mut state,
        Zve64xPermInstruction::VmergeVxm {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: false,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..16usize {
        let expected = if i % 2 == 0 { 777 } else { (i * 100) as u64 };
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E32),
            expected,
            "elem {i}"
        );
    }
}

// vcompress.vm

#[test]
fn vcompress_vm_e32_basic() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, ((i + 1) * 10) as u64);
        write_elem(&mut state, VReg::V4, i, Vsew::E32, 0xBEEF);
    }
    state.ext_state.write_vregs().get_mut(VReg::V1).fill(0);
    set_mask_bit(&mut state, VReg::V1, 1, true);
    set_mask_bit(&mut state, VReg::V1, 3, true);
    exec(
        &mut state,
        Zve64xPermInstruction::VcompressVm {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 20);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E32), 40);
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E32), 0xBEEF);
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E32), 0xBEEF);
    assert_eq!(state.ext_state.vstart(), 0);
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
}

#[test]
fn vcompress_vm_all_active() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, ((i + 1) * 7) as u64);
    }
    state.ext_state.write_vregs().get_mut(VReg::V1).fill(0xFF);
    exec(
        &mut state,
        Zve64xPermInstruction::VcompressVm {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E32),
            ((i + 1) * 7) as u64,
            "elem {i}"
        );
    }
}

#[test]
fn vcompress_vm_none_active() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, (i + 1) as u64);
        write_elem(&mut state, VReg::V4, i, Vsew::E32, 0xABCD);
    }
    state.ext_state.write_vregs().get_mut(VReg::V1).fill(0x00);
    exec(
        &mut state,
        Zve64xPermInstruction::VcompressVm {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E32),
            0xABCD,
            "elem {i}"
        );
    }
}

#[test]
fn vcompress_vm_e8_all_elements() {
    let mut state = setup(16, Vsew::E8, Vlmul::M1);
    for i in 0..16usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, (15 - i) as u64);
    }
    state.ext_state.write_vregs().get_mut(VReg::V1).fill(0xFF);
    exec(
        &mut state,
        Zve64xPermInstruction::VcompressVm {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..16usize {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E8),
            (15 - i) as u64,
            "elem {i}"
        );
    }
}

#[test]
fn vcompress_vm_vd_overlap_vs2_illegal() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    let err = exec(
        &mut state,
        Zve64xPermInstruction::VcompressVm {
            vd: VReg::V2,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::IllegalInstruction { .. }));
}

#[test]
fn vcompress_vm_vd_overlap_vs1_illegal() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    let err = exec(
        &mut state,
        Zve64xPermInstruction::VcompressVm {
            vd: VReg::V1,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::IllegalInstruction { .. }));
}

#[test]
fn vcompress_vm_rejects_nonzero_vstart() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, ((i + 1) * 10) as u64);
    }
    state.ext_state.write_vregs().get_mut(VReg::V1).fill(0xFF);
    state.ext_state.set_vstart(1);
    let err = exec(
        &mut state,
        Zve64xPermInstruction::VcompressVm {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::IllegalInstruction { .. }));
}

#[test]
fn vcompress_vm_vstart_zero_normal_operation() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, ((i + 1) * 10) as u64);
        write_elem(&mut state, VReg::V4, i, Vsew::E32, 0xDEAD);
    }
    state.ext_state.write_vregs().get_mut(VReg::V1).fill(0);
    set_mask_bit(&mut state, VReg::V1, 1, true);
    set_mask_bit(&mut state, VReg::V1, 2, true);
    exec(
        &mut state,
        Zve64xPermInstruction::VcompressVm {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 20);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E32), 30);
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E32), 0xDEAD);
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E32), 0xDEAD);
}

// vmv1r.v / vmv2r.v / vmv4r.v / vmv8r.v

#[test]
fn vmv1r_v_copies_single_register() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    let src: [u8; 32] = [
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
        26, 27, 28, 29, 30, 31, 32,
    ];
    *state.ext_state.write_vregs().get_mut(VReg::V2) = src;
    set_vreg_bytes(&mut state, VReg::V4, 0xCC);
    exec(
        &mut state,
        Zve64xPermInstruction::Vmv1rV {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(get_vreg_bytes(&state, VReg::V4), src);
    assert_eq!(state.ext_state.vstart(), 0);
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
}

#[test]
fn vmv1r_v_src_eq_dst_nop() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    set_vreg_bytes(&mut state, VReg::V2, 0xAB);
    exec(
        &mut state,
        Zve64xPermInstruction::Vmv1rV {
            vd: VReg::V2,
            vs2: VReg::V2,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(get_vreg_bytes(&state, VReg::V2), [0xAB; 32]);
}

#[test]
fn vmv2r_v_copies_two_registers() {
    // V2/V3 -> V4/V5 (all aligned to 2)
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    set_vreg_bytes(&mut state, VReg::V2, 0x11);
    set_vreg_bytes(&mut state, VReg::V3, 0x22);
    set_vreg_bytes(&mut state, VReg::V4, 0xCC);
    set_vreg_bytes(&mut state, VReg::V5, 0xCC);
    exec(
        &mut state,
        Zve64xPermInstruction::Vmv2rV {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(get_vreg_bytes(&state, VReg::V4), [0x11; 32]);
    assert_eq!(get_vreg_bytes(&state, VReg::V5), [0x22; 32]);
}

#[test]
fn vmv2r_v_misaligned_vd_illegal() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    let err = exec(
        &mut state,
        Zve64xPermInstruction::Vmv2rV {
            vd: VReg::V3,
            vs2: VReg::V2,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::IllegalInstruction { .. }));
}

#[test]
fn vmv2r_v_misaligned_vs2_illegal() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    let err = exec(
        &mut state,
        Zve64xPermInstruction::Vmv2rV {
            vd: VReg::V4,
            vs2: VReg::V3,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::IllegalInstruction { .. }));
}

#[test]
fn vmv4r_v_copies_four_registers() {
    // V8,V9,V10,V11 -> V12,V13,V14,V15 (both aligned to 4)
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for k in 0u8..4 {
        set_vreg_bytes(
            &mut state,
            VReg::from_bits(VReg::V8.to_bits() + k).unwrap(),
            k + 1,
        );
        set_vreg_bytes(
            &mut state,
            VReg::from_bits(VReg::V12.to_bits() + k).unwrap(),
            0xCC,
        );
    }
    exec(
        &mut state,
        Zve64xPermInstruction::Vmv4rV {
            vd: VReg::V12,
            vs2: VReg::V8,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for k in 0u8..4 {
        assert_eq!(
            get_vreg_bytes(&state, VReg::from_bits(VReg::V12.to_bits() + k).unwrap()),
            [k + 1; 32],
            "reg offset {k}"
        );
    }
    assert_eq!(state.ext_state.vstart(), 0);
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
}

#[test]
fn vmv4r_v_misaligned_vd_illegal() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    // V6 is not aligned to 4.
    let err = exec(
        &mut state,
        Zve64xPermInstruction::Vmv4rV {
            vd: VReg::V6,
            vs2: VReg::V8,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::IllegalInstruction { .. }));
}

#[test]
fn vmv4r_v_misaligned_vs2_illegal() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    // V6 is not aligned to 4.
    let err = exec(
        &mut state,
        Zve64xPermInstruction::Vmv4rV {
            vd: VReg::V12,
            vs2: VReg::V6,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::IllegalInstruction { .. }));
}

#[test]
fn vmv8r_v_copies_eight_registers() {
    // V8..V15 -> V16..V23 (both aligned to 8)
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for k in 0u8..8 {
        set_vreg_bytes(
            &mut state,
            VReg::from_bits(VReg::V8.to_bits() + k).unwrap(),
            k + 10,
        );
        set_vreg_bytes(
            &mut state,
            VReg::from_bits(VReg::V16.to_bits() + k).unwrap(),
            0xCC,
        );
    }
    exec(
        &mut state,
        Zve64xPermInstruction::Vmv8rV {
            vd: VReg::V16,
            vs2: VReg::V8,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for k in 0u8..8 {
        assert_eq!(
            get_vreg_bytes(&state, VReg::from_bits(VReg::V16.to_bits() + k).unwrap()),
            [k + 10; 32],
            "reg offset {k}"
        );
    }
}

#[test]
fn vmv8r_v_misaligned_vd_illegal() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    // V16 is aligned to 8 but V4 is not.
    let err = exec(
        &mut state,
        Zve64xPermInstruction::Vmv8rV {
            vd: VReg::V4,
            vs2: VReg::V8,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::IllegalInstruction { .. }));
}

#[test]
fn vmv8r_v_misaligned_vs2_illegal() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    // V8 is aligned to 8, V4 is not.
    let err = exec(
        &mut state,
        Zve64xPermInstruction::Vmv8rV {
            vd: VReg::V0,
            vs2: VReg::V4,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::IllegalInstruction { .. }));
}

#[test]
fn vmvr_does_not_require_valid_vtype() {
    // Whole-register moves must work even with vtype invalid (vill=1).
    let mut state = setup(0, Vsew::E32, Vlmul::M1);
    state.ext_state.set_vtype(None);
    set_vreg_bytes(&mut state, VReg::V2, 0xAB);
    set_vreg_bytes(&mut state, VReg::V4, 0x00);
    exec(
        &mut state,
        Zve64xPermInstruction::Vmv1rV {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(get_vreg_bytes(&state, VReg::V4), [0xAB; 32]);
}

// Multi-register group (LMUL > 1) tests

#[test]
fn vslideup_vx_m2_e32() {
    let mut state = setup(8, Vsew::E32, Vlmul::M2);
    for i in 0..8usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, ((i + 1) * 10) as u64);
        write_elem(&mut state, VReg::V4, i, Vsew::E32, 0xDEAD);
    }
    state.regs.write(Reg::A0, 3u64);
    exec(
        &mut state,
        Zve64xPermInstruction::VslideupVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..3usize {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E32),
            0xDEAD,
            "elem {i}"
        );
    }
    for i in 3..8usize {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E32),
            ((i - 3 + 1) * 10) as u64,
            "elem {i}"
        );
    }
}

#[test]
fn vslidedown_vx_m2_e32_partial() {
    let mut state = setup(8, Vsew::E32, Vlmul::M2);
    for i in 0..8usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, ((i + 1) * 100) as u64);
    }
    state.regs.write(Reg::A0, 5u64);
    exec(
        &mut state,
        Zve64xPermInstruction::VslidedownVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 600);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E32), 700);
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E32), 800);
    for i in 3..8usize {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E32), 0, "elem {i}");
    }
}

#[test]
fn vrgather_vv_m2_e32() {
    let mut state = setup(8, Vsew::E32, Vlmul::M2);
    for i in 0..8usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, (i as u64 + 1) * 100);
        write_elem(&mut state, VReg::V6, i, Vsew::E32, (7 - i) as u64);
    }
    exec(
        &mut state,
        Zve64xPermInstruction::VrgatherVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V6,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..8usize {
        assert_eq!(
            read_elem(&state, VReg::V4, i, Vsew::E32),
            (8 - i) as u64 * 100,
            "elem {i}"
        );
    }
}

#[test]
fn vslideup_unaligned_group_vd_illegal() {
    // M2/E32 requires vd aligned to 2; V3 is not.
    let mut state = setup(4, Vsew::E32, Vlmul::M2);
    state.regs.write(Reg::A0, 1u64);
    let err = exec(
        &mut state,
        Zve64xPermInstruction::VslideupVx {
            vd: VReg::V3,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::IllegalInstruction { .. }));
}

// vstart interaction

#[test]
fn vslide1down_vstart_skips_early_elements() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, ((i + 1) * 10) as u64);
        write_elem(&mut state, VReg::V4, i, Vsew::E32, 0xAA);
    }
    state.ext_state.set_vstart(2);
    state.regs.write(Reg::A0, 999u64);
    exec(
        &mut state,
        Zve64xPermInstruction::Vslide1downVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0xAA);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E32), 0xAA);
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E32), 40);
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E32), 999);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vrgather_vstart_skips_early_elements() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, (i + 1) as u64);
        write_elem(&mut state, VReg::V1, i, Vsew::E32, (3 - i) as u64);
        write_elem(&mut state, VReg::V4, i, Vsew::E32, 0xCC);
    }
    state.ext_state.set_vstart(2);
    exec(
        &mut state,
        Zve64xPermInstruction::VrgatherVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0xCC);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E32), 0xCC);
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E32), 2);
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E32), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

// vstart reset and vs_dirty invariants (array-based, no macros)

#[test]
fn all_instructions_reset_vstart() {
    // Each tuple: (instruction, needs_vstart_set)
    // We use a valid aligned register combination for every instruction.
    let cases = &[
        (
            Zve64xPermInstruction::VmvXS {
                rd: Reg::A1,
                vs2: VReg::V2,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "VmvXS",
        ),
        (
            Zve64xPermInstruction::VmvSX {
                vd: VReg::V4,
                rs1: Reg::A0,
                rs2: Reg::Zero,
            },
            "VmvSX",
        ),
        (
            Zve64xPermInstruction::VslideupVx {
                vd: VReg::V4,
                vs2: VReg::V2,
                rs1: Reg::A0,
                vm: true,
                rs2: Reg::Zero,
            },
            "VslideupVx",
        ),
        (
            Zve64xPermInstruction::VslideupVi {
                vd: VReg::V4,
                vs2: VReg::V2,
                uimm: 0,
                vm: true,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "VslideupVi",
        ),
        (
            Zve64xPermInstruction::VslidedownVx {
                vd: VReg::V4,
                vs2: VReg::V2,
                rs1: Reg::A0,
                vm: true,
                rs2: Reg::Zero,
            },
            "VslidedownVx",
        ),
        (
            Zve64xPermInstruction::VslidedownVi {
                vd: VReg::V4,
                vs2: VReg::V2,
                uimm: 0,
                vm: true,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "VslidedownVi",
        ),
        (
            Zve64xPermInstruction::Vslide1upVx {
                vd: VReg::V4,
                vs2: VReg::V2,
                rs1: Reg::A0,
                vm: true,
                rs2: Reg::Zero,
            },
            "Vslide1upVx",
        ),
        (
            Zve64xPermInstruction::Vslide1downVx {
                vd: VReg::V4,
                vs2: VReg::V2,
                rs1: Reg::A0,
                vm: true,
                rs2: Reg::Zero,
            },
            "Vslide1downVx",
        ),
        (
            Zve64xPermInstruction::VrgatherVv {
                vd: VReg::V4,
                vs2: VReg::V2,
                vs1: VReg::V1,
                vm: true,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "VrgatherVv",
        ),
        (
            Zve64xPermInstruction::VrgatherVx {
                vd: VReg::V4,
                vs2: VReg::V2,
                rs1: Reg::A0,
                vm: true,
                rs2: Reg::Zero,
            },
            "VrgatherVx",
        ),
        (
            Zve64xPermInstruction::VrgatherVi {
                vd: VReg::V4,
                vs2: VReg::V2,
                uimm: 0,
                vm: true,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "VrgatherVi",
        ),
        (
            Zve64xPermInstruction::Vmv1rV {
                vd: VReg::V4,
                vs2: VReg::V2,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "Vmv1rV",
        ),
        (
            Zve64xPermInstruction::Vmv2rV {
                vd: VReg::V4,
                vs2: VReg::V2,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "Vmv2rV",
        ),
        // V8/V12 are both aligned to 4
        (
            Zve64xPermInstruction::Vmv4rV {
                vd: VReg::V12,
                vs2: VReg::V8,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "Vmv4rV",
        ),
        // V8/V16 are both aligned to 8
        (
            Zve64xPermInstruction::Vmv8rV {
                vd: VReg::V16,
                vs2: VReg::V8,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "Vmv8rV",
        ),
    ];
    for (instr, name) in cases {
        let mut state = setup(4, Vsew::E32, Vlmul::M1);
        for i in 0..4usize {
            write_elem(&mut state, VReg::V2, i, Vsew::E32, (i + 1) as u64);
            write_elem(&mut state, VReg::V1, i, Vsew::E32, i as u64);
        }
        state.ext_state.write_vregs().get_mut(VReg::V1).fill(0xFF);
        state.ext_state.set_vstart(2);
        state.regs.write(Reg::A0, 1u64);
        exec(&mut state, *instr).unwrap();
        assert_eq!(state.ext_state.vstart(), 0, "vstart not reset for {name}");
    }
}

#[test]
fn all_vector_instructions_mark_vs_dirty() {
    let cases = &[
        (
            Zve64xPermInstruction::VmvXS {
                rd: Reg::A1,
                vs2: VReg::V2,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "VmvXS",
        ),
        (
            Zve64xPermInstruction::VmvSX {
                vd: VReg::V4,
                rs1: Reg::A0,
                rs2: Reg::Zero,
            },
            "VmvSX",
        ),
        (
            Zve64xPermInstruction::VslideupVx {
                vd: VReg::V4,
                vs2: VReg::V2,
                rs1: Reg::A0,
                vm: true,
                rs2: Reg::Zero,
            },
            "VslideupVx",
        ),
        (
            Zve64xPermInstruction::VslideupVi {
                vd: VReg::V4,
                vs2: VReg::V2,
                uimm: 0,
                vm: true,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "VslideupVi",
        ),
        (
            Zve64xPermInstruction::VslidedownVx {
                vd: VReg::V4,
                vs2: VReg::V2,
                rs1: Reg::A0,
                vm: true,
                rs2: Reg::Zero,
            },
            "VslidedownVx",
        ),
        (
            Zve64xPermInstruction::VslidedownVi {
                vd: VReg::V4,
                vs2: VReg::V2,
                uimm: 0,
                vm: true,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "VslidedownVi",
        ),
        (
            Zve64xPermInstruction::Vslide1upVx {
                vd: VReg::V4,
                vs2: VReg::V2,
                rs1: Reg::A0,
                vm: true,
                rs2: Reg::Zero,
            },
            "Vslide1upVx",
        ),
        (
            Zve64xPermInstruction::Vslide1downVx {
                vd: VReg::V4,
                vs2: VReg::V2,
                rs1: Reg::A0,
                vm: true,
                rs2: Reg::Zero,
            },
            "Vslide1downVx",
        ),
        (
            Zve64xPermInstruction::VrgatherVv {
                vd: VReg::V4,
                vs2: VReg::V2,
                vs1: VReg::V1,
                vm: true,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "VrgatherVv",
        ),
        (
            Zve64xPermInstruction::VrgatherVx {
                vd: VReg::V4,
                vs2: VReg::V2,
                rs1: Reg::A0,
                vm: true,
                rs2: Reg::Zero,
            },
            "VrgatherVx",
        ),
        (
            Zve64xPermInstruction::VrgatherVi {
                vd: VReg::V4,
                vs2: VReg::V2,
                uimm: 0,
                vm: true,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "VrgatherVi",
        ),
        (
            Zve64xPermInstruction::VcompressVm {
                vd: VReg::V4,
                vs2: VReg::V2,
                vs1: VReg::V1,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "VcompressVm",
        ),
        (
            Zve64xPermInstruction::Vmv1rV {
                vd: VReg::V4,
                vs2: VReg::V2,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "Vmv1rV",
        ),
        (
            Zve64xPermInstruction::Vmv2rV {
                vd: VReg::V4,
                vs2: VReg::V2,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "Vmv2rV",
        ),
        (
            Zve64xPermInstruction::Vmv4rV {
                vd: VReg::V12,
                vs2: VReg::V8,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "Vmv4rV",
        ),
        (
            Zve64xPermInstruction::Vmv8rV {
                vd: VReg::V16,
                vs2: VReg::V8,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "Vmv8rV",
        ),
    ];
    for (instr, name) in cases {
        let mut state = setup(4, Vsew::E32, Vlmul::M1);
        for i in 0..4usize {
            write_elem(&mut state, VReg::V2, i, Vsew::E32, (i + 1) as u64);
            write_elem(&mut state, VReg::V1, i, Vsew::E32, i as u64);
        }
        state.regs.write(Reg::A0, 1u64);
        state.ext_state.write_vregs().get_mut(VReg::V1).fill(0xFF);
        let before = state.ext_state.vs_dirty_count();
        exec(&mut state, *instr).unwrap();
        assert_eq!(
            state.ext_state.vs_dirty_count(),
            before + 1,
            "vs_dirty not incremented for {name}"
        );
    }
}

#[test]
fn all_instructions_illegal_when_vector_disabled() {
    let cases = &[
        (
            Zve64xPermInstruction::VmvXS {
                rd: Reg::A1,
                vs2: VReg::V2,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "VmvXS",
        ),
        (
            Zve64xPermInstruction::VmvSX {
                vd: VReg::V4,
                rs1: Reg::A0,
                rs2: Reg::Zero,
            },
            "VmvSX",
        ),
        (
            Zve64xPermInstruction::VslideupVx {
                vd: VReg::V4,
                vs2: VReg::V2,
                rs1: Reg::A0,
                vm: true,
                rs2: Reg::Zero,
            },
            "VslideupVx",
        ),
        (
            Zve64xPermInstruction::VslideupVi {
                vd: VReg::V4,
                vs2: VReg::V2,
                uimm: 0,
                vm: true,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "VslideupVi",
        ),
        (
            Zve64xPermInstruction::VslidedownVx {
                vd: VReg::V4,
                vs2: VReg::V2,
                rs1: Reg::A0,
                vm: true,
                rs2: Reg::Zero,
            },
            "VslidedownVx",
        ),
        (
            Zve64xPermInstruction::VslidedownVi {
                vd: VReg::V4,
                vs2: VReg::V2,
                uimm: 0,
                vm: true,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "VslidedownVi",
        ),
        (
            Zve64xPermInstruction::Vslide1upVx {
                vd: VReg::V4,
                vs2: VReg::V2,
                rs1: Reg::A0,
                vm: true,
                rs2: Reg::Zero,
            },
            "Vslide1upVx",
        ),
        (
            Zve64xPermInstruction::Vslide1downVx {
                vd: VReg::V4,
                vs2: VReg::V2,
                rs1: Reg::A0,
                vm: true,
                rs2: Reg::Zero,
            },
            "Vslide1downVx",
        ),
        (
            Zve64xPermInstruction::VrgatherVv {
                vd: VReg::V4,
                vs2: VReg::V2,
                vs1: VReg::V1,
                vm: true,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "VrgatherVv",
        ),
        (
            Zve64xPermInstruction::VrgatherVx {
                vd: VReg::V4,
                vs2: VReg::V2,
                rs1: Reg::A0,
                vm: true,
                rs2: Reg::Zero,
            },
            "VrgatherVx",
        ),
        (
            Zve64xPermInstruction::VrgatherVi {
                vd: VReg::V4,
                vs2: VReg::V2,
                uimm: 0,
                vm: true,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "VrgatherVi",
        ),
        (
            Zve64xPermInstruction::VcompressVm {
                vd: VReg::V4,
                vs2: VReg::V2,
                vs1: VReg::V1,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "VcompressVm",
        ),
        (
            Zve64xPermInstruction::Vmv1rV {
                vd: VReg::V4,
                vs2: VReg::V2,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "Vmv1rV",
        ),
        (
            Zve64xPermInstruction::Vmv2rV {
                vd: VReg::V4,
                vs2: VReg::V2,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "Vmv2rV",
        ),
        (
            Zve64xPermInstruction::Vmv4rV {
                vd: VReg::V12,
                vs2: VReg::V8,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "Vmv4rV",
        ),
        (
            Zve64xPermInstruction::Vmv8rV {
                vd: VReg::V16,
                vs2: VReg::V8,
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            },
            "Vmv8rV",
        ),
    ];
    for (instr, name) in cases {
        let mut state = setup(4, Vsew::E32, Vlmul::M1);
        state.ext_state.set_vector_allowed(false);
        for i in 0..4usize {
            write_elem(&mut state, VReg::V2, i, Vsew::E32, (i + 1) as u64);
        }
        state.regs.write(Reg::A0, 1u64);
        let err = exec(&mut state, *instr).unwrap_err();
        assert!(
            matches!(err, ExecutionError::IllegalInstruction { .. }),
            "expected IllegalInstruction for {name}, got {err:?}"
        );
    }
}

// vl == 0 edge cases

#[test]
fn vl_zero_leaves_vd_undisturbed_slide() {
    let mut state = setup(0, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, (i + 1) as u64);
        write_elem(&mut state, VReg::V4, i, Vsew::E32, 0xFACE);
    }
    state.regs.write(Reg::A0, 1u64);
    exec(
        &mut state,
        Zve64xPermInstruction::VslideupVx {
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
            0xFACE,
            "elem {i}"
        );
    }
}

#[test]
fn vl_zero_leaves_vd_undisturbed_rgather() {
    let mut state = setup(0, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, (i + 1) as u64);
        write_elem(&mut state, VReg::V4, i, Vsew::E32, 0xFACE);
    }
    state.regs.write(Reg::A0, 0u64);
    exec(
        &mut state,
        Zve64xPermInstruction::VrgatherVx {
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
            0xFACE,
            "elem {i}"
        );
    }
}

// Fractional LMUL

#[test]
fn vslideup_mf2_e64_offset_ge_vlmax_no_write() {
    // Mf2/E64: VLMAX = VLEN/(64*2) = 256/128 = 2; this test pins vl=1.
    let mut state = setup(1, Vsew::E64, Vlmul::Mf2);
    write_elem(&mut state, VReg::V2, 0, Vsew::E64, 0xABCD);
    write_elem(&mut state, VReg::V4, 0, Vsew::E64, 0xDEAD);
    // Offset 1 == vl: no active destinations.
    state.regs.write(Reg::A0, 1u64);
    exec(
        &mut state,
        Zve64xPermInstruction::VslideupVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E64), 0xDEAD);
}

#[test]
fn vslidedown_mf2_e64_offset_zero_copies() {
    let mut state = setup(1, Vsew::E64, Vlmul::Mf2);
    write_elem(&mut state, VReg::V2, 0, Vsew::E64, 0x1234);
    state.regs.write(Reg::A0, 0u64);
    exec(
        &mut state,
        Zve64xPermInstruction::VslidedownVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E64), 0x1234);
}
