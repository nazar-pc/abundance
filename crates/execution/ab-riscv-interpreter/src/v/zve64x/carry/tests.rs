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
) -> TestInterpreterState<Zve64xCarryInstruction<Reg<u64>>> {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    let vtype = Vtype::from_raw::<Reg<u64>>(encode_vtype(vsew, vlmul)).unwrap();
    state.ext_state.set_vtype(Some(vtype));
    state.ext_state.set_vl(vl);
    state.ext_state.set_vstart(0);
    state
}

fn exec(
    state: &mut TestInterpreterState<Zve64xCarryInstruction<Reg<u64>>>,
    instr: Zve64xCarryInstruction<Reg<u64>>,
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
    state: &mut TestInterpreterState<Zve64xCarryInstruction<Reg<u64>>>,
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
    state: &TestInterpreterState<Zve64xCarryInstruction<Reg<u64>>>,
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

/// Set mask bit `i` in register `reg`
fn set_mask_bit(
    state: &mut TestInterpreterState<Zve64xCarryInstruction<Reg<u64>>>,
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

fn read_mask_bit(
    state: &TestInterpreterState<Zve64xCarryInstruction<Reg<u64>>>,
    reg: VReg,
    i: u32,
) -> bool {
    let byte = state.ext_state.read_vregs().get(reg)[(i / u8::BITS) as usize];
    (byte >> (i % u8::BITS)) & 1 != 0
}

// vadc.vvm

#[test]
fn vadc_vvm_no_carry() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    // v0 = all zeros (no carry-in)
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, 10);
        write_elem(&mut state, VReg::V1, i, Vsew::E32, 5);
    }
    exec(
        &mut state,
        Zve64xCarryInstruction::VadcVvm {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E32), 15, "elem {i}");
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vadc_vvm_with_carry_propagates() {
    // For each element: vs2=0xFF, vs1=0x00, carry-in=1 → result=0x100 → wraps to 0x00
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E8, 0xFF);
        write_elem(&mut state, VReg::V1, i, Vsew::E8, 0x00);
        // Set carry-in bit i in v0
        set_mask_bit(&mut state, VReg::V0, i as u32, true);
    }
    exec(
        &mut state,
        Zve64xCarryInstruction::VadcVvm {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        // 0xFF + 0x00 + 1 = 0x100 → truncated to E8 = 0x00
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E8), 0x00, "elem {i}");
    }
}

#[test]
fn vadc_vvm_mixed_carry_bits() {
    // Alternating carry: elements 0,2 have carry=1; elements 1,3 have carry=0
    let mut state = setup(4, Vsew::E16, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E16, 100);
        write_elem(&mut state, VReg::V1, i, Vsew::E16, 1);
    }
    set_mask_bit(&mut state, VReg::V0, 0, true);
    set_mask_bit(&mut state, VReg::V0, 1, false);
    set_mask_bit(&mut state, VReg::V0, 2, true);
    set_mask_bit(&mut state, VReg::V0, 3, false);
    exec(
        &mut state,
        Zve64xCarryInstruction::VadcVvm {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // carry=1: 100+1+1=102; carry=0: 100+1+0=101
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E16), 102);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E16), 101);
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E16), 102);
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E16), 101);
}

#[test]
fn vadc_vxm_with_scalar_and_carry() {
    let mut state = setup(2, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E64, u64::MAX);
    write_elem(&mut state, VReg::V2, 1, Vsew::E64, 5);
    state.regs.write(Reg::A0, 0u64);
    set_mask_bit(&mut state, VReg::V0, 0, true);
    set_mask_bit(&mut state, VReg::V0, 1, false);
    exec(
        &mut state,
        Zve64xCarryInstruction::VadcVxm {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // u64::MAX + 0 + 1 = wraps to 0
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E64), 0);
    // 5 + 0 + 0 = 5
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E64), 5);
}

#[test]
fn vadc_vim_sign_extended_imm() {
    let mut state = setup(2, Vsew::E16, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E16, 0x0001);
    write_elem(&mut state, VReg::V2, 1, Vsew::E16, 0x0001);
    // imm = -1 (sign-extended): 0xFFFF + 0x0001 + carry
    set_mask_bit(&mut state, VReg::V0, 0, false);
    set_mask_bit(&mut state, VReg::V0, 1, true);
    exec(
        &mut state,
        Zve64xCarryInstruction::VadcVim {
            vd: VReg::V4,
            vs2: VReg::V2,
            imm: -1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 0x0001 + 0xFFFF + 0 = 0x10000 → truncated to E16 = 0x0000
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E16), 0x0000);
    // 0x0001 + 0xFFFF + 1 = 0x10001 → truncated to E16 = 0x0001
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E16), 0x0001);
}

// vmadc: carry-out mask

#[test]
fn vmadc_vvm_carry_out_when_overflow() {
    // Element 0: 0xFF + 0x01 + carry=0 → 0x100, carry-out=1
    // Element 1: 0x01 + 0x01 + carry=0 → 0x02, carry-out=0
    // Element 2: 0xFE + 0x01 + carry=1 → 0x100, carry-out=1
    // Element 3: 0x01 + 0x00 + carry=1 → 0x02, carry-out=0
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xFF);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0x01);
    write_elem(&mut state, VReg::V2, 2, Vsew::E8, 0xFE);
    write_elem(&mut state, VReg::V2, 3, Vsew::E8, 0x01);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 0x01);
    write_elem(&mut state, VReg::V1, 1, Vsew::E8, 0x01);
    write_elem(&mut state, VReg::V1, 2, Vsew::E8, 0x01);
    write_elem(&mut state, VReg::V1, 3, Vsew::E8, 0x00);
    set_mask_bit(&mut state, VReg::V0, 0, false);
    set_mask_bit(&mut state, VReg::V0, 1, false);
    set_mask_bit(&mut state, VReg::V0, 2, true);
    set_mask_bit(&mut state, VReg::V0, 3, true);
    exec(
        &mut state,
        Zve64xCarryInstruction::VmadcVvm {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert!(read_mask_bit(&state, VReg::V4, 0));
    assert!(!read_mask_bit(&state, VReg::V4, 1));
    assert!(read_mask_bit(&state, VReg::V4, 2));
    assert!(!read_mask_bit(&state, VReg::V4, 3));
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vmadc_vv_no_carry_in_overflow_check() {
    // Without carry-in: 0xFF + 0x01 = 0x100 → carry-out=1
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xFF);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0x10);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 0x01);
    write_elem(&mut state, VReg::V1, 1, Vsew::E8, 0x10);
    exec(
        &mut state,
        Zve64xCarryInstruction::VmadcVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 0xFF + 0x01 = overflow → carry-out=1
    assert!(read_mask_bit(&state, VReg::V4, 0));
    // 0x10 + 0x10 = 0x20 → no overflow
    assert!(!read_mask_bit(&state, VReg::V4, 1));
}

#[test]
fn vmadc_vv_e64_carry_out() {
    let mut state = setup(1, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E64, u64::MAX);
    write_elem(&mut state, VReg::V1, 0, Vsew::E64, 1);
    exec(
        &mut state,
        Zve64xCarryInstruction::VmadcVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert!(read_mask_bit(&state, VReg::V4, 0));
}

#[test]
fn vmadc_vx_no_carry_scalar() {
    let mut state = setup(3, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 0xFFFF_FFFF);
    write_elem(&mut state, VReg::V2, 1, Vsew::E32, 0x7FFF_FFFF);
    write_elem(&mut state, VReg::V2, 2, Vsew::E32, 0);
    state.regs.write(Reg::A0, 1u64);
    exec(
        &mut state,
        Zve64xCarryInstruction::VmadcVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert!(read_mask_bit(&state, VReg::V4, 0));
    assert!(!read_mask_bit(&state, VReg::V4, 1));
    assert!(!read_mask_bit(&state, VReg::V4, 2));
}

#[test]
fn vmadc_vi_no_carry_imm() {
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    // imm=1: 0xFF+1=overflow, 0x00+1=no overflow
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 0xFF);
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 0x00);
    exec(
        &mut state,
        Zve64xCarryInstruction::VmadcVi {
            vd: VReg::V4,
            vs2: VReg::V2,
            imm: 1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert!(read_mask_bit(&state, VReg::V4, 0));
    assert!(!read_mask_bit(&state, VReg::V4, 1));
}

// vsbc

#[test]
fn vsbc_vvm_no_borrow() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, 100);
        write_elem(&mut state, VReg::V1, i, Vsew::E32, 10);
    }
    exec(
        &mut state,
        Zve64xCarryInstruction::VsbcVvm {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    for i in 0..4usize {
        assert_eq!(read_elem(&state, VReg::V4, i, Vsew::E32), 90, "elem {i}");
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vsbc_vvm_with_borrow_propagates() {
    // vs2=0, vs1=0, borrow-in=1 → 0 - 0 - 1 = -1 = 0xFFFFFFFF (E32)
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 0);
    write_elem(&mut state, VReg::V2, 1, Vsew::E32, 10);
    write_elem(&mut state, VReg::V1, 0, Vsew::E32, 0);
    write_elem(&mut state, VReg::V1, 1, Vsew::E32, 3);
    set_mask_bit(&mut state, VReg::V0, 0, true);
    set_mask_bit(&mut state, VReg::V0, 1, false);
    exec(
        &mut state,
        Zve64xCarryInstruction::VsbcVvm {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0xFFFF_FFFF);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E32), 7);
}

#[test]
fn vsbc_vxm_basic() {
    let mut state = setup(3, Vsew::E16, Vlmul::M1);
    for i in 0..3usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E16, 50);
    }
    state.regs.write(Reg::A0, 20u64);
    set_mask_bit(&mut state, VReg::V0, 0, false);
    set_mask_bit(&mut state, VReg::V0, 1, true);
    set_mask_bit(&mut state, VReg::V0, 2, false);
    exec(
        &mut state,
        Zve64xCarryInstruction::VsbcVxm {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E16), 30);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E16), 29);
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E16), 30);
}

// vmsbc

#[test]
fn vmsbc_vvm_borrow_out() {
    // vs2=5, vs1=10, borrow-in=0 → underflow → borrow-out=1
    // vs2=10, vs1=5, borrow-in=0 → no underflow → borrow-out=0
    // vs2=5, vs1=4, borrow-in=1 → 5 - 4 - 1 = 0 → no underflow → borrow-out=0
    // vs2=5, vs1=5, borrow-in=1 → 5 - 5 - 1 = -1 → underflow → borrow-out=1
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 5);
    write_elem(&mut state, VReg::V2, 1, Vsew::E32, 10);
    write_elem(&mut state, VReg::V2, 2, Vsew::E32, 5);
    write_elem(&mut state, VReg::V2, 3, Vsew::E32, 5);
    write_elem(&mut state, VReg::V1, 0, Vsew::E32, 10);
    write_elem(&mut state, VReg::V1, 1, Vsew::E32, 5);
    write_elem(&mut state, VReg::V1, 2, Vsew::E32, 4);
    write_elem(&mut state, VReg::V1, 3, Vsew::E32, 5);
    set_mask_bit(&mut state, VReg::V0, 0, false);
    set_mask_bit(&mut state, VReg::V0, 1, false);
    set_mask_bit(&mut state, VReg::V0, 2, true);
    set_mask_bit(&mut state, VReg::V0, 3, true);
    exec(
        &mut state,
        Zve64xCarryInstruction::VmsbcVvm {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert!(read_mask_bit(&state, VReg::V4, 0));
    assert!(!read_mask_bit(&state, VReg::V4, 1));
    assert!(!read_mask_bit(&state, VReg::V4, 2));
    assert!(read_mask_bit(&state, VReg::V4, 3));
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vmsbc_vv_no_borrow_in() {
    // No borrow-in: just check unsigned underflow
    let mut state = setup(2, Vsew::E8, Vlmul::M1);
    // 3 - 5: underflow
    write_elem(&mut state, VReg::V2, 0, Vsew::E8, 3);
    // 5 - 3: no underflow
    write_elem(&mut state, VReg::V2, 1, Vsew::E8, 5);
    write_elem(&mut state, VReg::V1, 0, Vsew::E8, 5);
    write_elem(&mut state, VReg::V1, 1, Vsew::E8, 3);
    exec(
        &mut state,
        Zve64xCarryInstruction::VmsbcVv {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert!(read_mask_bit(&state, VReg::V4, 0));
    assert!(!read_mask_bit(&state, VReg::V4, 1));
}

#[test]
fn vmsbc_vxm_e64_exact_boundary() {
    // 0 - 1 - borrow_in=0: underflow, borrow-out=1
    // MAX - MAX - borrow_in=0: no underflow, borrow-out=0
    let mut state = setup(2, Vsew::E64, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E64, 0);
    write_elem(&mut state, VReg::V2, 1, Vsew::E64, u64::MAX);
    state.regs.write(Reg::A0, u64::MAX);
    set_mask_bit(&mut state, VReg::V0, 0, false);
    set_mask_bit(&mut state, VReg::V0, 1, false);
    exec(
        &mut state,
        Zve64xCarryInstruction::VmsbcVxm {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 0 < u64::MAX → underflow
    assert!(read_mask_bit(&state, VReg::V4, 0));
    // u64::MAX == u64::MAX → no underflow
    assert!(!read_mask_bit(&state, VReg::V4, 1));
}

#[test]
fn vmsbc_vx_no_borrow() {
    let mut state = setup(3, Vsew::E32, Vlmul::M1);
    write_elem(&mut state, VReg::V2, 0, Vsew::E32, 0);
    write_elem(&mut state, VReg::V2, 1, Vsew::E32, 10);
    write_elem(&mut state, VReg::V2, 2, Vsew::E32, 0xFFFF_FFFF);
    state.regs.write(Reg::A0, 1u64);
    exec(
        &mut state,
        Zve64xCarryInstruction::VmsbcVx {
            vd: VReg::V4,
            vs2: VReg::V2,
            rs1: Reg::A0,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    // 0 < 1 → underflow
    assert!(read_mask_bit(&state, VReg::V4, 0));
    // 10 >= 1 → no underflow
    assert!(!read_mask_bit(&state, VReg::V4, 1));
    // 0xFFFF_FFFF >= 1 → no underflow
    assert!(!read_mask_bit(&state, VReg::V4, 2));
}

// Error paths

#[test]
fn error_vadc_vd_is_v0() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xCarryInstruction::VadcVvm {
            vd: VReg::V0,
            vs2: VReg::V2,
            vs1: VReg::V4,
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
fn error_vsbc_vd_is_v0() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    let result = exec(
        &mut state,
        Zve64xCarryInstruction::VsbcVvm {
            vd: VReg::V0,
            vs2: VReg::V2,
            vs1: VReg::V4,
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
fn error_vector_not_allowed() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    state.ext_state.set_vector_allowed(false);
    let result = exec(
        &mut state,
        Zve64xCarryInstruction::VadcVvm {
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

#[test]
fn error_vill_vtype() {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    state.ext_state.set_vtype(None);
    state.ext_state.set_vl(0);
    let result = exec(
        &mut state,
        Zve64xCarryInstruction::VadcVvm {
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

#[test]
fn error_vmadc_vd_overlaps_vs2_lmul_gt_1() {
    let mut state = setup(8, Vsew::E32, Vlmul::M2);
    let result = exec(
        &mut state,
        Zve64xCarryInstruction::VmadcVvm {
            vd: VReg::V3,
            vs2: VReg::V2,
            vs1: VReg::V6,
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
fn error_vadc_vs2_misaligned_m2() {
    let mut state = setup(4, Vsew::E32, Vlmul::M2);
    let result = exec(
        &mut state,
        Zve64xCarryInstruction::VadcVvm {
            vd: VReg::V4,
            vs2: VReg::V3,
            vs1: VReg::V6,
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
fn vadc_vstart_skips_elements() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V2, i, Vsew::E32, 10);
        write_elem(&mut state, VReg::V1, i, Vsew::E32, 1);
        write_elem(&mut state, VReg::V4, i, Vsew::E32, 0xDEAD);
    }
    state.ext_state.set_vstart(2);
    exec(
        &mut state,
        Zve64xCarryInstruction::VadcVvm {
            vd: VReg::V4,
            vs2: VReg::V2,
            vs1: VReg::V1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    assert_eq!(read_elem(&state, VReg::V4, 0, Vsew::E32), 0xDEAD);
    assert_eq!(read_elem(&state, VReg::V4, 1, Vsew::E32), 0xDEAD);
    assert_eq!(read_elem(&state, VReg::V4, 2, Vsew::E32), 11);
    assert_eq!(read_elem(&state, VReg::V4, 3, Vsew::E32), 11);
    assert_eq!(state.ext_state.vstart(), 0);
}

// vl=0

#[test]
fn vadc_vl_zero_no_writes() {
    let mut state = setup(0, Vsew::E32, Vlmul::M1);
    for i in 0..4usize {
        write_elem(&mut state, VReg::V4, i, Vsew::E32, 0xBEEF);
    }
    exec(
        &mut state,
        Zve64xCarryInstruction::VadcVvm {
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
            0xBEEF,
            "elem {i}"
        );
    }
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
}

// Cross-SEW correctness

#[test]
fn vadc_wraps_at_sew_boundary() {
    for (vsew, max) in [
        (Vsew::E8, 0xFFu64),
        (Vsew::E16, 0xFFFF),
        (Vsew::E32, 0xFFFF_FFFF),
        (Vsew::E64, u64::MAX),
    ] {
        let mut state = setup(1, vsew, Vlmul::M1);
        write_elem(&mut state, VReg::V2, 0, vsew, max);
        write_elem(&mut state, VReg::V1, 0, vsew, 0);
        set_mask_bit(&mut state, VReg::V0, 0, true);
        exec(
            &mut state,
            Zve64xCarryInstruction::VadcVvm {
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
            0,
            "SEW={vsew:?}: MAX+carry should wrap to 0"
        );
    }
}
