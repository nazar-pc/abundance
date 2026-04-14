use crate::rv64::test_utils::{TestInterpreterState, initialize_state};
use crate::v::vector_registers::{VectorRegisters, VectorRegistersExt};
use crate::{ExecutableInstruction, ExecutionError};
use ab_riscv_primitives::instructions::v::zve64x::reduction::Zve64xReductionInstruction;
use ab_riscv_primitives::instructions::v::{Vlmul, Vsew, Vtype};
use ab_riscv_primitives::registers::general_purpose::Reg;
use ab_riscv_primitives::registers::vector::VReg;

fn encode_vtype(vsew: Vsew, vlmul: Vlmul) -> u64 {
    (vlmul.to_bits() as u64) | ((vsew.to_bits() as u64) << 3)
}

fn setup(
    vl: u32,
    vsew: Vsew,
    vlmul: Vlmul,
) -> TestInterpreterState<Zve64xReductionInstruction<Reg<u64>>> {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    let vtype = Vtype::from_raw::<Reg<u64>>(encode_vtype(vsew, vlmul)).unwrap();
    state.ext_state.set_vtype(Some(vtype));
    state.ext_state.set_vl(vl);
    state.ext_state.set_vstart(0);
    state
}

fn exec(
    state: &mut TestInterpreterState<Zve64xReductionInstruction<Reg<u64>>>,
    instr: Zve64xReductionInstruction<Reg<u64>>,
) -> Result<(), ExecutionError<u64>> {
    instr.execute(state).map(|_| ())
}

fn read_elem(
    state: &TestInterpreterState<Zve64xReductionInstruction<Reg<u64>>>,
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
    state: &mut TestInterpreterState<Zve64xReductionInstruction<Reg<u64>>>,
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

fn set_mask_bit(
    state: &mut TestInterpreterState<Zve64xReductionInstruction<Reg<u64>>>,
    elem_i: u32,
    active: bool,
) {
    let byte = &mut state.ext_state.write_vreg()[usize::from(VReg::V0.bits())]
        [(elem_i / u8::BITS) as usize];
    if active {
        *byte |= 1 << (elem_i % u8::BITS);
    } else {
        *byte &= !(1 << (elem_i % u8::BITS));
    }
}

// vredsum

#[test]
fn vredsum_e8_m1_basic() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, (i + 1) as u64);
    }
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 10);
    exec(
        &mut state,
        Zve64xReductionInstruction::Vredsum {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 20);
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vredsum_e8_m1_wraps() {
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 200);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 100);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 0);
    exec(
        &mut state,
        Zve64xReductionInstruction::Vredsum {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 300u64 & 0xff);
}

#[test]
fn vredsum_e16_m1_basic() {
    let mut state = setup(8, Vsew::E16, Vlmul::M1);
    for i in 0..8usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E16, (i + 1) as u64);
    }
    write_elem(&mut state, VReg::V1, 0, Vsew::E16, 100);
    exec(
        &mut state,
        Zve64xReductionInstruction::Vredsum {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E16), 136);
}

#[test]
fn vredsum_e32_m1_basic() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, 1000);
    }
    write_elem(&mut state, VReg::V1, 0, Vsew::E32, 7);
    exec(
        &mut state,
        Zve64xReductionInstruction::Vredsum {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 4007);
}

#[test]
fn vredsum_e64_m1_basic() {
    let mut state = setup(2, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E64, 0xffff_ffff_ffff_fff0);
    write_elem(&mut state, VReg::V2, 1, Vsew::E64, 0x10);
    write_elem(&mut state, VReg::V1, 0, Vsew::E64, 1);
    exec(
        &mut state,
        Zve64xReductionInstruction::Vredsum {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E64), 1);
}

// Spec §5.4: when vstart >= vl, no element of vd is updated, and vs is not marked dirty.
#[test]
fn vredsum_vl_zero_leaves_vd_undisturbed() {
    let mut state = setup(0, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V4, 0, Vsew::E32, 0xdead_beef);
    write_elem(&mut state, VReg::V4, 1, Vsew::E32, 0xcafe_babe);
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 999);
    write_elem(&mut state, VReg::V1, 0, Vsew::E32, 42);
    let dirty_before = state.ext_state.vs_dirty_count();
    exec(
        &mut state,
        Zve64xReductionInstruction::Vredsum {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0xdead_beef);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E32), 0xcafe_babe);
    assert_eq!(state.ext_state.vs_dirty_count(), dirty_before);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vredsum_masked_skips_inactive() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    // Active: elements 0, 2; inactive: 1, 3
    for i in 0..4 {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, 10);
        set_mask_bit(&mut state, i as u32, i % 2 == 0);
    }
    write_elem(&mut state, VReg::V1, 0, Vsew::E32, 5);
    exec(
        &mut state,
        Zve64xReductionInstruction::Vredsum {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: false,
        },
    )
    .unwrap();
    // 5 + 10 + 10 = 25
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 25);
}

// All elements masked out but vl > 0: spec §14.1 - vd[0] gets vs1[0] (the identity carry).
// This is distinct from vl == 0, where vd is not written at all.
#[test]
fn vredsum_all_masked_out_writes_vs1_zero() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V4, 0, Vsew::E32, 0xdead_beef);
    for i in 0..4 {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, 99);
        set_mask_bit(&mut state, i as u32, false);
    }
    write_elem(&mut state, VReg::V1, 0, Vsew::E32, 7);
    exec(
        &mut state,
        Zve64xReductionInstruction::Vredsum {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: false,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 7);
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
}

#[test]
fn vredsum_m2_uses_group() {
    // LMUL=2, E8: VLMAX=32; vs2 spans v2-v3
    let mut state = setup(32, Vsew::E8, Vlmul::M2);
    for i in 0..32usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 1);
    }
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 0);
    exec(
        &mut state,
        Zve64xReductionInstruction::Vredsum {
            vd: VReg::V8,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V8, 0, Vsew::E8), 32u64 & 0xff);
}

// vredand

#[test]
fn vredand_e8_m1_basic() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    // vs2 = [0b1111, 0b1010, 0b1100, 0b1110], vs1[0] = 0xff
    // AND reduction: 0xff & 0x0f & 0x0a & 0x0c & 0x0e = 0x08
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0x0f);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0x0a);
    write_elem(&mut state, VReg::V2, 2, Vsew::E8, 0x0c);
    write_elem(&mut state, VReg::V2, 3, Vsew::E8, 0x0e);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 0xff);
    exec(
        &mut state,
        Zve64xReductionInstruction::Vredand {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 0x08);
}

#[test]
fn vredand_e64_identity() {
    let mut state = setup(2, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E64, u64::MAX);
    write_elem(&mut state, VReg::V2, 1, Vsew::E64, u64::MAX);
    write_elem(&mut state, VReg::V1, 0, Vsew::E64, u64::MAX);
    exec(
        &mut state,
        Zve64xReductionInstruction::Vredand {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E64), u64::MAX);
}

#[test]
fn vredand_vl_zero_leaves_vd_undisturbed() {
    let mut state = setup(0, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V4, 0, Vsew::E32, 0x5555_aaaa);
    write_elem(&mut state, VReg::V1, 0, Vsew::E32, 0xdead_beef);
    let dirty_before = state.ext_state.vs_dirty_count();
    exec(
        &mut state,
        Zve64xReductionInstruction::Vredand {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0x5555_aaaa);
    assert_eq!(state.ext_state.vs_dirty_count(), dirty_before);
}

// vredor

#[test]
fn vredor_e8_m1_basic() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0x01);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0x02);
    write_elem(&mut state, VReg::V2, 2, Vsew::E8, 0x04);
    write_elem(&mut state, VReg::V2, 3, Vsew::E8, 0x08);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 0x00);
    exec(
        &mut state,
        Zve64xReductionInstruction::Vredor {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 0x0f);
}

#[test]
fn vredor_vl_zero_leaves_vd_undisturbed() {
    let mut state = setup(0, Vsew::E16, Vlmul::M1);
    write_elem(&mut state, VReg::V4, 0, Vsew::E16, 0xbeef);
    write_elem(&mut state, VReg::V1, 0, Vsew::E16, 0x1234);
    exec(
        &mut state,
        Zve64xReductionInstruction::Vredor {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E16), 0xbeef);
}

// vredxor

#[test]
fn vredxor_e32_m1_basic() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    // XOR of all same value cancels out pairs
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 0xaaaa_aaaa);
    write_elem(&mut state, VReg::V2, 1, Vsew::E32, 0xaaaa_aaaa);
    write_elem(&mut state, VReg::V2, 2, Vsew::E32, 0x5555_5555);
    write_elem(&mut state, VReg::V2, 3, Vsew::E32, 0x5555_5555);
    write_elem(&mut state, VReg::V1, 0, Vsew::E32, 0);
    exec(
        &mut state,
        Zve64xReductionInstruction::Vredxor {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0);
}

#[test]
fn vredxor_e8_parity() {
    let mut state = setup(3, Vsew::E8, Vlmul::M1);
    // 0x01 ^ 0x03 ^ 0x07 = 0x05; initial 0 -> 0x05
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0x01);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0x03);
    write_elem(&mut state, VReg::V2, 2, Vsew::E8, 0x07);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 0);
    exec(
        &mut state,
        Zve64xReductionInstruction::Vredxor {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 0x05);
}

// vredminu

#[test]
fn vredminu_e8_m1_basic() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 200);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 5);
    write_elem(&mut state, VReg::V2, 2, Vsew::E8, 150);
    write_elem(&mut state, VReg::V2, 3, Vsew::E8, 80);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 255);
    exec(
        &mut state,
        Zve64xReductionInstruction::Vredminu {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 5);
}

#[test]
fn vredminu_e32_initial_wins() {
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    // vs1[0] smaller than all vs2 elements (unsigned)
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 1000);
    write_elem(&mut state, VReg::V2, 1, Vsew::E32, 2000);
    write_elem(&mut state, VReg::V1, 0, Vsew::E32, 1);
    exec(
        &mut state,
        Zve64xReductionInstruction::Vredminu {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 1);
}

#[test]
fn vredminu_treats_as_unsigned() {
    // 0x80 (128 as u8, but -128 as i8) must be treated unsigned
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0x80);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0x01);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 0xff);
    exec(
        &mut state,
        Zve64xReductionInstruction::Vredminu {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    // Unsigned min of {0xff, 0x80, 0x01} = 0x01
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 0x01);
}

// vredmin

#[test]
fn vredmin_e8_m1_signed() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    // -1 (0xff), -128 (0x80), 0, 127 (0x7f); signed min = -128
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xff);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0x80);
    write_elem(&mut state, VReg::V2, 2, Vsew::E8, 0);
    write_elem(&mut state, VReg::V2, 3, Vsew::E8, 0x7f);
    // initial = 0 (signed)
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 0);
    exec(
        &mut state,
        Zve64xReductionInstruction::Vredmin {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 0x80);
}

#[test]
fn vredmin_e32_initial_is_most_negative() {
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 0xffff_ff00);
    write_elem(&mut state, VReg::V2, 1, Vsew::E32, 5);
    // vs1[0] = 0x8000_0000 = i32::MIN, which is the most negative
    write_elem(&mut state, VReg::V1, 0, Vsew::E32, 0x8000_0000);
    exec(
        &mut state,
        Zve64xReductionInstruction::Vredmin {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0x8000_0000);
}

// vredmaxu

#[test]
fn vredmaxu_e8_m1_basic() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 10);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 200);
    write_elem(&mut state, VReg::V2, 2, Vsew::E8, 50);
    write_elem(&mut state, VReg::V2, 3, Vsew::E8, 170);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 0);
    exec(
        &mut state,
        Zve64xReductionInstruction::Vredmaxu {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 200);
}

#[test]
fn vredmaxu_treats_as_unsigned() {
    // 0x80 (-128 signed but 128 unsigned) should win over 0x01
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0x01);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0x80);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 0);
    exec(
        &mut state,
        Zve64xReductionInstruction::Vredmaxu {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 0x80);
}

// vredmax

#[test]
fn vredmax_e8_m1_signed() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    // 0xff = -1, 0x80 = -128, 0 = 0, 0x7f = 127; signed max = 127
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xff);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0x80);
    write_elem(&mut state, VReg::V2, 2, Vsew::E8, 0);
    write_elem(&mut state, VReg::V2, 3, Vsew::E8, 0x7f);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 0);
    exec(
        &mut state,
        Zve64xReductionInstruction::Vredmax {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 0x7f);
}

#[test]
fn vredmax_e32_initial_is_largest() {
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 0x7fff_fff0);
    write_elem(&mut state, VReg::V2, 1, Vsew::E32, 100);
    // vs1[0] = 0x7fff_ffff = i32::MAX
    write_elem(&mut state, VReg::V1, 0, Vsew::E32, 0x7fff_ffff);
    exec(
        &mut state,
        Zve64xReductionInstruction::Vredmax {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0x7fff_ffff);
}

// vwredsumu

#[test]
fn vwredsumu_e8_to_e16_basic() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    // vs2 = [1, 2, 3, 4] at E8; vs1[0] = 10 at E16; result = 10+1+2+3+4 = 20 at E16
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, (i + 1) as u64);
    }
    // Write vs1[0] at E16 (wide SEW)
    write_elem(&mut state, VReg::V1, 0, Vsew::E16, 10);
    exec(
        &mut state,
        Zve64xReductionInstruction::Vwredsumu {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E16), 20);
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vwredsumu_e8_to_e16_zero_extends() {
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    // 0xff should be zero-extended to 0x00ff, not sign-extended to 0xffff
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xff);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0xff);
    write_elem(&mut state, VReg::V1, 0, Vsew::E16, 0);
    exec(
        &mut state,
        Zve64xReductionInstruction::Vwredsumu {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E16), 510);
}

#[test]
fn vwredsumu_e16_to_e32_basic() {
    let mut state = setup(4, Vsew::E16, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E16, 0x8000);
    }
    write_elem(&mut state, VReg::V1, 0, Vsew::E32, 0);
    exec(
        &mut state,
        Zve64xReductionInstruction::Vwredsumu {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 4 * 0x8000u64);
}

#[test]
fn vwredsumu_e32_to_e64_basic() {
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 0xffff_ffff);
    write_elem(&mut state, VReg::V2, 1, Vsew::E32, 1);
    write_elem(&mut state, VReg::V1, 0, Vsew::E64, 0);
    exec(
        &mut state,
        Zve64xReductionInstruction::Vwredsumu {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E64), 0x1_0000_0000u64);
}

#[test]
fn vwredsumu_vl_zero_leaves_vd_undisturbed() {
    let mut state = setup(0, Vsew::E16, Vlmul::M1);
    write_elem(&mut state, VReg::V4, 0, Vsew::E32, 0x5a5a_5a5a);
    write_elem(&mut state, VReg::V1, 0, Vsew::E32, 0xabcd);
    let dirty_before = state.ext_state.vs_dirty_count();
    exec(
        &mut state,
        Zve64xReductionInstruction::Vwredsumu {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0x5a5a_5a5a);
    assert_eq!(state.ext_state.vs_dirty_count(), dirty_before);
}

// vwredsum

#[test]
fn vwredsum_e8_to_e16_sign_extends() {
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xff);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0xff);
    write_elem(&mut state, VReg::V1, 0, Vsew::E16, 0);
    exec(
        &mut state,
        Zve64xReductionInstruction::Vwredsum {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E16), 0xfffe);
}

#[test]
fn vwredsum_e8_to_e16_mixed_signs() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xff);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0xff);
    write_elem(&mut state, VReg::V2, 2, Vsew::E8, 0x01);
    write_elem(&mut state, VReg::V2, 3, Vsew::E8, 0x01);
    write_elem(&mut state, VReg::V1, 0, Vsew::E16, 0);
    exec(
        &mut state,
        Zve64xReductionInstruction::Vwredsum {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E16), 0);
}

#[test]
fn vwredsum_e16_to_e32_sign_extends() {
    let mut state = setup(2, Vsew::E16, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E16, 0x8000);
    write_elem(&mut state, VReg::V2, 1, Vsew::E16, 0x8000);
    write_elem(&mut state, VReg::V1, 0, Vsew::E32, 0);
    exec(
        &mut state,
        Zve64xReductionInstruction::Vwredsum {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0xffff_0000u64);
}

#[test]
fn vwredsum_e32_to_e64_sign_extends() {
    let mut state = setup(1, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 0x8000_0000);
    write_elem(&mut state, VReg::V1, 0, Vsew::E64, 0);
    exec(
        &mut state,
        Zve64xReductionInstruction::Vwredsum {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(
        read_elem(&state, VReg::V4, 0, Vsew::E64),
        0xffff_ffff_8000_0000u64
    );
}

#[test]
fn vwredsum_vl_zero_leaves_vd_undisturbed() {
    let mut state = setup(0, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V4, 0, Vsew::E16, 0xbeef);
    write_elem(&mut state, VReg::V1, 0, Vsew::E16, 0x1234);
    let dirty_before = state.ext_state.vs_dirty_count();
    exec(
        &mut state,
        Zve64xReductionInstruction::Vwredsum {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E16), 0xbeef);
    assert_eq!(state.ext_state.vs_dirty_count(), dirty_before);
}

// widening illegal with E64

#[test]
fn vwredsumu_e64_is_illegal() {
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xReductionInstruction::Vwredsumu {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    );
    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

#[test]
fn vwredsum_e64_is_illegal() {
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xReductionInstruction::Vwredsum {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    );
    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

// guard rails

#[test]
fn reduction_vector_not_allowed() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    state.ext_state.set_vector_allowed(false);
    let result = exec(
        &mut state,
        Zve64xReductionInstruction::Vredsum {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    );
    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

#[test]
fn reduction_invalid_vtype_is_illegal() {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    let result = exec(
        &mut state,
        Zve64xReductionInstruction::Vredsum {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    );
    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

#[test]
fn reduction_misaligned_vs2_m2_is_illegal() {
    let mut state = setup(4, Vsew::E32, Vlmul::M2);
    let result = exec(
        &mut state,
        Zve64xReductionInstruction::Vredsum {
            vd: VReg::V8,
            vs2: VReg::V3,
            vs1: VReg::V1,
            vm: true,
        },
    );
    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

// Spec §14: reductions with non-zero vstart are reserved. Raise illegal instruction
// rather than proceeding with a partial reduction.
#[test]
fn reduction_nonzero_vstart_is_illegal() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    state.ext_state.set_vstart(1);
    let result = exec(
        &mut state,
        Zve64xReductionInstruction::Vredsum {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    );
    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

#[test]
fn widening_reduction_nonzero_vstart_is_illegal() {
    let mut state = setup(4, Vsew::E16, Vlmul::M1);
    state.ext_state.set_vstart(2);
    let result = exec(
        &mut state,
        Zve64xReductionInstruction::Vwredsum {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    );
    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

#[test]
fn reduction_vstart_reset_after_execution() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, 1);
    }
    write_elem(&mut state, VReg::V1, 0, Vsew::E32, 0);
    exec(
        &mut state,
        Zve64xReductionInstruction::Vredsum {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn reduction_marks_vs_dirty() {
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V1, 0, Vsew::E32, 0);
    let before = state.ext_state.vs_dirty_count();
    exec(
        &mut state,
        Zve64xReductionInstruction::Vredsum {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert!(state.ext_state.vs_dirty_count() > before);
}

#[test]
fn reduction_vd_element_zero_only_written() {
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V4, 0, Vsew::E32, 0xdead_beef);
    write_elem(&mut state, VReg::V4, 1, Vsew::E32, 0xcafe_babe);
    write_elem(&mut state, VReg::V4, 2, Vsew::E32, 0x1234_5678);
    write_elem(&mut state, VReg::V4, 3, Vsew::E32, 0xaaaa_aaaa);
    for i in 0..2usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, 1);
    }
    write_elem(&mut state, VReg::V1, 0, Vsew::E32, 0);
    exec(
        &mut state,
        Zve64xReductionInstruction::Vredsum {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 2);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E32), 0xcafe_babe);
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E32), 0x1234_5678);
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E32), 0xaaaa_aaaa);
}

// signed vs unsigned distinction

#[test]
fn vredmin_vs_vredminu_differ_on_high_bit() {
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0x80);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0x01);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 0x7f);
    let mut state_u = setup(2, Vsew::E8, Vlmul::M1);
    write_elem(&mut state_u, VReg::V2, 0, Vsew::E8, 0x80);
    write_elem(&mut state_u, VReg::V2, 1, Vsew::E8, 0x01);
    write_elem(&mut state_u, VReg::V1, 0, Vsew::E8, 0x7f);
    exec(
        &mut state,
        Zve64xReductionInstruction::Vredmin {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    exec(
        &mut state_u,
        Zve64xReductionInstruction::Vredminu {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E8), 0x80);
    assert_eq!(read_elem(&state_u, VReg::V4, 0, Vsew::E8), 0x01);
}

#[test]
fn vredmax_vs_vredmaxu_differ_on_high_bit() {
    let mut state_s = setup(2, Vsew::E8, Vlmul::M1);
    write_elem(&mut state_s, VReg::V2, 0, Vsew::E8, 0x80);
    write_elem(&mut state_s, VReg::V2, 1, Vsew::E8, 0x01);
    write_elem(&mut state_s, VReg::V1, 0, Vsew::E8, 0);
    let mut state_u = setup(2, Vsew::E8, Vlmul::M1);
    write_elem(&mut state_u, VReg::V2, 0, Vsew::E8, 0x80);
    write_elem(&mut state_u, VReg::V2, 1, Vsew::E8, 0x01);
    write_elem(&mut state_u, VReg::V1, 0, Vsew::E8, 0);
    exec(
        &mut state_s,
        Zve64xReductionInstruction::Vredmax {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    exec(
        &mut state_u,
        Zve64xReductionInstruction::Vredmaxu {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state_s, VReg::V4, 0, Vsew::E8), 0x01);
    assert_eq!(read_elem(&state_u, VReg::V4, 0, Vsew::E8), 0x80);
}

#[test]
fn vwredsumu_vs_vwredsum_differ_on_high_bit() {
    let mut state_u = setup(1, Vsew::E8, Vlmul::M1);
    write_elem(&mut state_u, VReg::V2, 0, Vsew::E8, 0x80);
    write_elem(&mut state_u, VReg::V1, 0, Vsew::E16, 0);
    let mut state_s = setup(1, Vsew::E8, Vlmul::M1);
    write_elem(&mut state_s, VReg::V2, 0, Vsew::E8, 0x80);
    write_elem(&mut state_s, VReg::V1, 0, Vsew::E16, 0);
    exec(
        &mut state_u,
        Zve64xReductionInstruction::Vwredsumu {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    exec(
        &mut state_s,
        Zve64xReductionInstruction::Vwredsum {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            vm: true,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state_u, VReg::V4, 0, Vsew::E16), 0x0080);
    assert_eq!(read_elem(&state_s, VReg::V4, 0, Vsew::E16), 0xff80);
}
