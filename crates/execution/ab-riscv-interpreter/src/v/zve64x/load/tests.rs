use crate::rv64::test_utils::{TEST_BASE_ADDR, TestInterpreterState, initialize_state};
use crate::v::vector_registers::{VectorRegisters, VectorRegistersExt};
use crate::{
    ExecutableInstruction, ExecutableInstructionOperands, ExecutionError, RegisterFile,
    Rs1Rs2OperandValues, Rs1Rs2Operands, VirtualMemory,
};
use ab_riscv_primitives::prelude::*;
use core::array;

// With TEST_VLEN=128 and TEST_VLENB=16, the VLMAX values are:
//   E8/M1=16, E16/M1=8, E32/M1=4, E64/M1=2
//   E8/M2=32, E16/M2=16, E32/M2=8, E64/M2=4
//   E8/M4=64, E32/M4=16
//   E8/Mf2=8, E16/Mf2=4

/// Initialize the state with vector CSRs and a given vtype configuration
fn setup(
    vl: u32,
    vsew: Vsew,
    vlmul: Vlmul,
) -> TestInterpreterState<Zve64xLoadInstruction<Reg<u64>>> {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    let vtype = Vtype::from_raw::<Reg<u64>>(encode_vtype(vsew, vlmul)).unwrap();
    state.ext_state.set_vtype(Some(vtype));
    state.ext_state.set_vl(vl);
    state.ext_state.set_vstart(0);
    state
}

/// Encode a raw vtype value from SEW and LMUL (vta=false, vma=false)
fn encode_vtype(vsew: Vsew, vlmul: Vlmul) -> u64 {
    u64::from(vlmul.to_bits()) | (u64::from(vsew.to_bits()) << 3)
}

/// Write a sequence of bytes into test memory starting at `addr`
fn write_mem(
    state: &mut TestInterpreterState<Zve64xLoadInstruction<Reg<u64>>>,
    addr: u64,
    data: &[u8],
) {
    for (i, &b) in data.iter().enumerate() {
        state.memory.write::<u8>(addr + i as u64, b).unwrap();
    }
}

/// Read a byte from a vector register
fn vreg_byte(
    state: &TestInterpreterState<Zve64xLoadInstruction<Reg<u64>>>,
    reg: VReg,
    offset: usize,
) -> u8 {
    state.ext_state.read_vreg()[usize::from(reg.bits())][offset]
}

/// Read a full vector register as a byte slice copy
fn vreg_bytes(
    state: &TestInterpreterState<Zve64xLoadInstruction<Reg<u64>>>,
    reg: VReg,
) -> [u8; 16] {
    state.ext_state.read_vreg()[usize::from(reg.bits())]
}

/// Set a vector register's bytes directly
fn set_vreg(
    state: &mut TestInterpreterState<Zve64xLoadInstruction<Reg<u64>>>,
    reg: VReg,
    data: &[u8],
) {
    let dst = &mut state.ext_state.write_vreg()[usize::from(reg.bits())];
    dst[..data.len()].copy_from_slice(data);
}

/// Execute a single instruction directly (not via the instruction fetcher)
fn exec_one(
    state: &mut TestInterpreterState<Zve64xLoadInstruction<Reg<u64>>>,
    instr: Zve64xLoadInstruction<Reg<u64>>,
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

// `Vlr` tests

#[test]
fn vlr_single_register_loads_vlenb_bytes() {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    let data = array::from_fn::<_, 16, _>(|i| i as u8);
    write_mem(&mut state, TEST_BASE_ADDR, &data);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vlr {
            vd: VReg::V2,
            rs1: Reg::A0,
            nreg: 1,
            eew: Eew::E8,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    assert_eq!(vreg_bytes(&state, VReg::V2), data);
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
}

#[test]
fn vlr_two_registers_loads_two_vlenb_blocks() {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    let data = array::from_fn::<_, 32, _>(|i| i as u8);
    write_mem(&mut state, TEST_BASE_ADDR, &data);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vlr {
            vd: VReg::V2,
            rs1: Reg::A0,
            nreg: 2,
            eew: Eew::E8,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    assert_eq!(&vreg_bytes(&state, VReg::V2), &data[..16]);
    assert_eq!(&vreg_bytes(&state, VReg::V3), &data[16..]);
}

#[test]
fn vlr_four_registers() {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    let data = array::from_fn::<_, 64, _>(|i| i as u8);
    write_mem(&mut state, TEST_BASE_ADDR, &data);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vlr {
            vd: VReg::V4,
            rs1: Reg::A0,
            nreg: 4,
            eew: Eew::E8,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    for i in 0u8..4 {
        let expected: [u8; 16] = data[i as usize * 16..(i as usize + 1) * 16]
            .try_into()
            .unwrap();
        let reg = VReg::from_bits(4 + i).unwrap();
        assert_eq!(vreg_bytes(&state, reg), expected);
    }
}

#[test]
fn vlr_ignores_vtype_and_vl() {
    // vtype is vill and vl=0; Vlr should still load regardless
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    // vtype remains vill (default after init_vector_csrs)
    let data = [0xABu8; 16];
    write_mem(&mut state, TEST_BASE_ADDR, &data);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vlr {
            vd: VReg::V0,
            rs1: Reg::A0,
            nreg: 1,
            eew: Eew::E8,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    assert_eq!(vreg_bytes(&state, VReg::V0), data);
}

#[test]
fn vlr_resets_vstart_on_success() {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    state.ext_state.set_vstart(7);
    let data = [0u8; 16];
    write_mem(&mut state, TEST_BASE_ADDR, &data);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vlr {
            vd: VReg::V1,
            rs1: Reg::A0,
            nreg: 1,
            eew: Eew::E8,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    assert_eq!(
        state.ext_state.vstart(),
        0,
        "Vlr must reset vstart on completion"
    );
}

#[test]
fn vlr_misaligned_vd_is_illegal() {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    // nreg=2 requires vd % 2 == 0; V3 is misaligned
    let err = exec_one(
        &mut state,
        Zve64xLoadInstruction::Vlr {
            vd: VReg::V3,
            rs1: Reg::A0,
            nreg: 2,
            eew: Eew::E8,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::IllegalInstruction { .. }));
}

#[test]
fn vlr_out_of_bounds_memory_returns_error() {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    // Address 0 is below the memory base
    state.regs.write(Reg::A0, 0);

    let err = exec_one(
        &mut state,
        Zve64xLoadInstruction::Vlr {
            vd: VReg::V0,
            rs1: Reg::A0,
            nreg: 1,
            eew: Eew::E8,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::MemoryAccess(_)));
}

// `Vlm` tests

#[test]
fn vlm_loads_ceil_vl_over_8_bytes() {
    // vl=10 -> ceil(10/8)=2 bytes loaded
    let mut state = setup(10, Vsew::E8, Vlmul::M1);
    let data = [0b1011_0101, 0b0000_0011];
    write_mem(&mut state, TEST_BASE_ADDR, &data);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vlm {
            vd: VReg::V3,
            rs1: Reg::A0,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    assert_eq!(vreg_byte(&state, VReg::V3, 0), 0b1011_0101);
    assert_eq!(vreg_byte(&state, VReg::V3, 1), 0b0000_0011);
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0, "vstart must be reset");
}

#[test]
fn vlm_vl_8_loads_exactly_1_byte() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    write_mem(&mut state, TEST_BASE_ADDR, &[0xFFu8]);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vlm {
            vd: VReg::V1,
            rs1: Reg::A0,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    assert_eq!(vreg_byte(&state, VReg::V1, 0), 0xFF);
}

#[test]
fn vlm_vl_0_loads_no_bytes_and_leaves_dst_unchanged() {
    let mut state = setup(0, Vsew::E8, Vlmul::M1);
    set_vreg(&mut state, VReg::V5, &[0xABu8; 16]);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vlm {
            vd: VReg::V5,
            rs1: Reg::A0,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    // Nothing written; destination unchanged
    assert_eq!(vreg_bytes(&state, VReg::V5), [0xABu8; 16]);
}

#[test]
fn vlm_does_not_require_valid_vtype() {
    // vtype is vill; Vlm only needs vector_instructions_allowed, not valid vtype
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    // vtype stays vill; vl is 3
    state.ext_state.set_vl(3);
    write_mem(&mut state, TEST_BASE_ADDR, &[0x07u8]);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vlm {
            vd: VReg::V0,
            rs1: Reg::A0,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    assert_eq!(vreg_byte(&state, VReg::V0, 0), 0x07);
}

#[test]
fn vlm_vector_not_allowed_is_illegal() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    state.ext_state.set_vector_allowed(false);

    let err = exec_one(
        &mut state,
        Zve64xLoadInstruction::Vlm {
            vd: VReg::V1,
            rs1: Reg::A0,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::IllegalInstruction { .. }));
}

// `Vle` tests

#[test]
fn vle_e8_loads_vl_bytes_sequentially() {
    // E8/M1: VLMAX=16, vl=4
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    let data = [10u8, 20, 30, 40];
    write_mem(&mut state, TEST_BASE_ADDR, &data);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vle {
            vd: VReg::V1,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E8,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    assert_eq!(vreg_byte(&state, VReg::V1, 0), 10);
    assert_eq!(vreg_byte(&state, VReg::V1, 1), 20);
    assert_eq!(vreg_byte(&state, VReg::V1, 2), 30);
    assert_eq!(vreg_byte(&state, VReg::V1, 3), 40);
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vle_e32_loads_vl_words_sequentially() {
    // E32/M1: VLMAX=4, vl=3
    let mut state = setup(3, Vsew::E32, Vlmul::M1);
    let data = array::from_fn::<_, 12, _>(|i| i as u8);
    write_mem(&mut state, TEST_BASE_ADDR, &data);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vle {
            vd: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E32,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    // Element 0: bytes [0,1,2,3] at offset 0
    assert_eq!(vreg_bytes(&state, VReg::V2)[0..4], [0, 1, 2, 3]);
    // Element 1: bytes [4,5,6,7] at offset 4
    assert_eq!(vreg_bytes(&state, VReg::V2)[4..8], [4, 5, 6, 7]);
    // Element 2: bytes [8,9,10,11] at offset 8
    assert_eq!(vreg_bytes(&state, VReg::V2)[8..12], [8, 9, 10, 11]);
}

#[test]
fn vle_e64_loads_vl_doublewords() {
    // E64/M1: VLMAX=2, vl=2
    let mut state = setup(2, Vsew::E64, Vlmul::M1);
    let val0 = 0x0102_0304_0506_0708_u64;
    let val1 = 0xDEAD_BEEF_CAFE_BABE_u64;
    write_mem(&mut state, TEST_BASE_ADDR, &val0.to_le_bytes());
    write_mem(&mut state, TEST_BASE_ADDR + 8, &val1.to_le_bytes());
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vle {
            vd: VReg::V4,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E64,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    assert_eq!(vreg_bytes(&state, VReg::V4)[0..8], val0.to_le_bytes());
    assert_eq!(vreg_bytes(&state, VReg::V4)[8..16], val1.to_le_bytes());
}

#[test]
fn vle_vl_0_does_not_write_any_elements() {
    let mut state = setup(0, Vsew::E32, Vlmul::M1);
    set_vreg(&mut state, VReg::V7, &[0xFFu8; 16]);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vle {
            vd: VReg::V7,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E32,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    // Tail is undisturbed when vl=0
    assert_eq!(vreg_bytes(&state, VReg::V7), [0xFFu8; 16]);
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
}

#[test]
fn vle_masked_skips_inactive_elements_undisturbed() {
    // E8/M1: vl=8, mask=0b0011_0101 -> elements 0,2,4,5 active
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    // Pre-fill destination with sentinel
    set_vreg(&mut state, VReg::V2, &[0xEEu8; 16]);
    // Set mask in v0: byte 0 = 0b0011_0101
    set_vreg(&mut state, VReg::V0, &{
        let mut m = [0u8; 16];
        m[0] = 0b0011_0101;
        m
    });
    // Write 8 distinct bytes to memory
    write_mem(&mut state, TEST_BASE_ADDR, &[1, 2, 3, 4, 5, 6, 7, 8]);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vle {
            vd: VReg::V2,
            rs1: Reg::A0,
            vm: false,
            eew: Eew::E8,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    let reg = vreg_bytes(&state, VReg::V2);
    // Active elements (mask bit set): 0, 2, 4, 5
    assert_eq!(reg[0], 1, "element 0 active");
    assert_eq!(reg[1], 0xEE, "element 1 inactive, undisturbed");
    assert_eq!(reg[2], 3, "element 2 active");
    assert_eq!(reg[3], 0xEE, "element 3 inactive, undisturbed");
    assert_eq!(reg[4], 5, "element 4 active");
    assert_eq!(reg[5], 6, "element 5 active");
    assert_eq!(reg[6], 0xEE, "element 6 inactive, undisturbed");
    assert_eq!(reg[7], 0xEE, "element 7 inactive, undisturbed");
}

#[test]
fn vle_respects_vstart_skips_earlier_elements() {
    // E8/M1: vl=4, vstart=2 -> only elements 2,3 loaded
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    set_vreg(&mut state, VReg::V1, &[0xCCu8; 16]);
    write_mem(&mut state, TEST_BASE_ADDR, &[10, 20, 30, 40]);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);
    state.ext_state.set_vstart(2);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vle {
            vd: VReg::V1,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E8,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    let reg = vreg_bytes(&state, VReg::V1);
    // Elements 0,1 untouched (below vstart)
    assert_eq!(reg[0], 0xCC, "element 0 below vstart, undisturbed");
    assert_eq!(reg[1], 0xCC, "element 1 below vstart, undisturbed");
    // Elements 2,3 loaded. Address offsets: element 2 is at base + 2*1 = base+2
    assert_eq!(reg[2], 30, "element 2 loaded");
    assert_eq!(reg[3], 40, "element 3 loaded");
    assert_eq!(state.ext_state.vstart(), 0, "vstart reset after completion");
}

#[test]
fn vle_vtype_vill_is_illegal() {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    // vtype stays vill
    let err = exec_one(
        &mut state,
        Zve64xLoadInstruction::Vle {
            vd: VReg::V1,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E32,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::IllegalInstruction { .. }));
}

#[test]
fn vle_vector_not_allowed_is_illegal() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    state.ext_state.set_vector_allowed(false);

    let err = exec_one(
        &mut state,
        Zve64xLoadInstruction::Vle {
            vd: VReg::V1,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E32,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::IllegalInstruction { .. }));
}

#[test]
fn vle_masked_vd_overlapping_v0_is_illegal() {
    // vm=false with vd=V0 -> overlap with mask register
    let mut state = setup(4, Vsew::E8, Vlmul::M1);

    let err = exec_one(
        &mut state,
        Zve64xLoadInstruction::Vle {
            vd: VReg::V0,
            rs1: Reg::A0,
            vm: false,
            eew: Eew::E8,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::IllegalInstruction { .. }));
}

#[test]
fn vle_eew_wider_than_sew_uses_multiple_registers() {
    // SEW=E32/M1 but EEW=E64 -> EMUL=2, vd needs 2 registers
    // VLMAX (for EEW=E64, EMUL=2) = 2*16/8 = 4 elements
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    let data = array::from_fn::<_, 16, _>(|i| i as u8);
    write_mem(&mut state, TEST_BASE_ADDR, &data);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    // vd=V2 (aligned to 2), group uses V2 and V3
    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vle {
            vd: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E64,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    // Element 0 (8 bytes) in V2[0..8]
    assert_eq!(vreg_bytes(&state, VReg::V2)[0..8], data[0..8]);
    // Element 1 (8 bytes) in V2[8..16]
    assert_eq!(vreg_bytes(&state, VReg::V2)[8..16], data[8..16]);
}

#[test]
fn vle_misaligned_vd_for_emul2_is_illegal() {
    // SEW=E32/M1, EEW=E64 -> EMUL=2, vd must be even; V3 is misaligned
    let mut state = setup(2, Vsew::E32, Vlmul::M1);

    let err = exec_one(
        &mut state,
        Zve64xLoadInstruction::Vle {
            vd: VReg::V3,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E64,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::IllegalInstruction { .. }));
}

#[test]
fn vle_memory_fault_propagates() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    // Address 0 is out of bounds
    state.regs.write(Reg::A0, 0);

    let err = exec_one(
        &mut state,
        Zve64xLoadInstruction::Vle {
            vd: VReg::V1,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E32,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::MemoryAccess(_)));
}

// `Vleff` tests

#[test]
fn vleff_no_fault_behaves_like_vle() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    write_mem(&mut state, TEST_BASE_ADDR, &[1, 2, 3, 4]);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vleff {
            vd: VReg::V1,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E8,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    let reg = vreg_bytes(&state, VReg::V1);
    assert_eq!(reg[0], 1);
    assert_eq!(reg[1], 2);
    assert_eq!(reg[2], 3);
    assert_eq!(reg[3], 4);
    // vl unchanged
    assert_eq!(state.ext_state.vl(), 4);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vleff_fault_at_i0_traps() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    // Out-of-bounds address for element 0
    state.regs.write(Reg::A0, 0);

    let err = exec_one(
        &mut state,
        Zve64xLoadInstruction::Vleff {
            vd: VReg::V1,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E32,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::MemoryAccess(_)));
    // vl must not be modified on a trapped fault
    assert_eq!(state.ext_state.vl(), 4);
}

#[test]
fn vleff_fault_at_i1_truncates_vl_to_1() {
    // Place only 4 valid bytes (one E32 element) in memory; the second element address is valid
    // but out of the allocated region - use an address near the end of memory
    let mem_top = TEST_BASE_ADDR + 8191;
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    // Write one valid E32 element at mem_top-3 (4 bytes fit), second element would be at mem_top+1
    let aligned_addr = mem_top - 3;
    write_mem(&mut state, aligned_addr, &[0xAAu8, 0xBBu8, 0xCCu8, 0xDDu8]);
    state.regs.write(Reg::A0, aligned_addr);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vleff {
            vd: VReg::V1,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E32,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    // Element 0 was loaded
    assert_eq!(vreg_bytes(&state, VReg::V1)[0..4], [0xAA, 0xBB, 0xCC, 0xDD]);
    // vl truncated to 1 (fault at element 1)
    assert_eq!(state.ext_state.vl(), 1);
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vleff_fault_at_i2_truncates_vl_to_2() {
    // E8/M1: vl=4; write 2 valid bytes, then cause fault at element 2
    // Put the base address so that only 2 bytes are accessible
    let mem_end = TEST_BASE_ADDR + 8192;
    let base = mem_end - 2; // only 2 bytes remain in bounds
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    write_mem(&mut state, base, &[0x11u8, 0x22u8]);
    state.regs.write(Reg::A0, base);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vleff {
            vd: VReg::V3,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E8,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    assert_eq!(vreg_byte(&state, VReg::V3, 0), 0x11);
    assert_eq!(vreg_byte(&state, VReg::V3, 1), 0x22);
    assert_eq!(state.ext_state.vl(), 2);
}

// `Vlse` tests

#[test]
fn vlse_positive_stride_loads_at_stride_intervals() {
    // E32/M1: vl=3, stride=8 -> addr[i] = base + i*8
    let mut state = setup(3, Vsew::E32, Vlmul::M1);
    // Write 3 words at offsets 0, 8, 16 from base
    state
        .memory
        .write::<u32>(TEST_BASE_ADDR, 0xAAAA_AAAA)
        .unwrap();
    state
        .memory
        .write::<u32>(TEST_BASE_ADDR + 8, 0xBBBB_BBBB)
        .unwrap();
    state
        .memory
        .write::<u32>(TEST_BASE_ADDR + 16, 0xCCCC_CCCC)
        .unwrap();
    state.regs.write(Reg::A0, TEST_BASE_ADDR);
    state.regs.write(Reg::A1, 8);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vlse {
            vd: VReg::V1,
            rs1: Reg::A0,
            rs2: Reg::A1,
            vm: true,
            eew: Eew::E32,
        },
    )
    .unwrap();

    let reg = vreg_bytes(&state, VReg::V1);
    assert_eq!(
        u32::from_le_bytes(reg[0..4].try_into().unwrap()),
        0xAAAA_AAAA
    );
    assert_eq!(
        u32::from_le_bytes(reg[4..8].try_into().unwrap()),
        0xBBBB_BBBB
    );
    assert_eq!(
        u32::from_le_bytes(reg[8..12].try_into().unwrap()),
        0xCCCC_CCCC
    );
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vlse_negative_stride_loads_in_reverse() {
    // E8/M1: vl=3, stride=-1 -> elements loaded at base, base-1, base-2
    let mut state = setup(3, Vsew::E8, Vlmul::M1);
    // base = TEST_BASE_ADDR + 2 so that base-2 = TEST_BASE_ADDR is still valid
    let base = TEST_BASE_ADDR + 2;
    write_mem(&mut state, TEST_BASE_ADDR, &[0x30u8, 0x20, 0x10]);
    state.regs.write(Reg::A0, base);
    // stride = -1 as i64 -> 0xFFFFFFFFFFFFFFFF as u64
    state.regs.write(Reg::A1, (-1i64).cast_unsigned());

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vlse {
            vd: VReg::V2,
            rs1: Reg::A0,
            rs2: Reg::A1,
            vm: true,
            eew: Eew::E8,
        },
    )
    .unwrap();

    // element 0: base+0 = TEST_BASE_ADDR+2 -> value 0x10
    // element 1: base-1 = TEST_BASE_ADDR+1 -> value 0x20
    // element 2: base-2 = TEST_BASE_ADDR+0 -> value 0x30
    assert_eq!(vreg_byte(&state, VReg::V2, 0), 0x10);
    assert_eq!(vreg_byte(&state, VReg::V2, 1), 0x20);
    assert_eq!(vreg_byte(&state, VReg::V2, 2), 0x30);
}

#[test]
fn vlse_zero_stride_loads_same_address_repeatedly() {
    // E32/M1: vl=4, stride=0 -> all elements from base
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    state
        .memory
        .write::<u32>(TEST_BASE_ADDR, 0xDEAD_BEEF)
        .unwrap();
    state.regs.write(Reg::A0, TEST_BASE_ADDR);
    state.regs.write(Reg::A1, 0u64);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vlse {
            vd: VReg::V1,
            rs1: Reg::A0,
            rs2: Reg::A1,
            vm: true,
            eew: Eew::E32,
        },
    )
    .unwrap();

    let reg = vreg_bytes(&state, VReg::V1);
    for i in 0..4 {
        assert_eq!(
            u32::from_le_bytes(reg[i * 4..(i + 1) * 4].try_into().unwrap()),
            0xDEAD_BEEF,
            "element {i}"
        );
    }
}

#[test]
fn vlse_masked_skips_inactive_elements() {
    // E8/M1: vl=4, stride=1, mask=0b0101 -> elements 0,2 active
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    set_vreg(&mut state, VReg::V0, &{
        let mut m = [0u8; 16];
        m[0] = 0b0101;
        m
    });
    set_vreg(&mut state, VReg::V2, &[0xDDu8; 16]);
    write_mem(&mut state, TEST_BASE_ADDR, &[10u8, 20, 30, 40]);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);
    state.regs.write(Reg::A1, 1u64);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vlse {
            vd: VReg::V2,
            rs1: Reg::A0,
            rs2: Reg::A1,
            vm: false,
            eew: Eew::E8,
        },
    )
    .unwrap();

    let reg = vreg_bytes(&state, VReg::V2);
    assert_eq!(reg[0], 10, "element 0 active");
    assert_eq!(reg[1], 0xDD, "element 1 inactive");
    assert_eq!(reg[2], 30, "element 2 active");
    assert_eq!(reg[3], 0xDD, "element 3 inactive");
}

// `Vluxei` tests

#[test]
fn vluxei_e32_data_e32_index_basic() {
    // SEW=E32/M1: data EEW=E32, index EEW=E32, vl=3
    // Indices select data values scattered in memory
    let mut state = setup(3, Vsew::E32, Vlmul::M1);
    // Write indices (u32 LE) at base: [12, 0, 8] -> offsets into data region
    let index_base = TEST_BASE_ADDR;
    let data_base = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u32>(index_base, 12u32).unwrap();
    state.memory.write::<u32>(index_base + 4, 0u32).unwrap();
    state.memory.write::<u32>(index_base + 8, 8u32).unwrap();
    // Write data values at data_base + offsets
    state.memory.write::<u32>(data_base, 0x1111_1111).unwrap();
    state
        .memory
        .write::<u32>(data_base + 8, 0x2222_2222)
        .unwrap();
    state
        .memory
        .write::<u32>(data_base + 12, 0x3333_3333)
        .unwrap();

    // Load indices into vs2=V4
    state.regs.write(Reg::A0, index_base);
    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vle {
            vd: VReg::V4,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E32,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    // Perform indexed load with base=data_base
    state.regs.write(Reg::A0, data_base);
    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vluxei {
            vd: VReg::V1,
            rs1: Reg::A0,
            vs2: VReg::V4,
            vm: true,
            eew: Eew::E32,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    let reg = vreg_bytes(&state, VReg::V1);
    assert_eq!(
        u32::from_le_bytes(reg[0..4].try_into().unwrap()),
        0x3333_3333,
        "elem0 at offset 12"
    );
    assert_eq!(
        u32::from_le_bytes(reg[4..8].try_into().unwrap()),
        0x1111_1111,
        "elem1 at offset 0"
    );
    assert_eq!(
        u32::from_le_bytes(reg[8..12].try_into().unwrap()),
        0x2222_2222,
        "elem2 at offset 8"
    );
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vluxei_index_eew_smaller_than_data_eew() {
    // SEW=E32/M1, index EEW=E8, data EEW=E32 (data EEW=SEW for indexed)
    // EMUL_index = 8/32 * 1 = 1/4 -> 1 register
    // vl=2
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    // Write two u8 indices [4, 0] into V6 bytes [0,1]
    let mut idx_reg = [0u8; 16];
    // offset 4 bytes into data
    idx_reg[0] = 4;
    idx_reg[1] = 0;
    set_vreg(&mut state, VReg::V6, &idx_reg);

    let data_base = TEST_BASE_ADDR;
    state.memory.write::<u32>(data_base, 0xAABB_CCDD).unwrap();
    state
        .memory
        .write::<u32>(data_base + 4, 0x1122_3344)
        .unwrap();
    state.regs.write(Reg::A0, data_base);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vluxei {
            vd: VReg::V1,
            rs1: Reg::A0,
            vs2: VReg::V6,
            vm: true,
            eew: Eew::E8,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    let reg = vreg_bytes(&state, VReg::V1);
    assert_eq!(
        u32::from_le_bytes(reg[0..4].try_into().unwrap()),
        0x1122_3344,
        "elem0 at offset 4"
    );
    assert_eq!(
        u32::from_le_bytes(reg[4..8].try_into().unwrap()),
        0xAABB_CCDD,
        "elem1 at offset 0"
    );
}

#[test]
fn vluxei_vd_vs2_overlap_is_illegal() {
    // data_group_regs=1 (LMUL=M1), index_group_regs=1 (EMUL_index=1 for E32/E32/M1)
    let mut state = setup(2, Vsew::E32, Vlmul::M1);

    let err = exec_one(
        &mut state,
        Zve64xLoadInstruction::Vluxei {
            vd: VReg::V3,
            rs1: Reg::A0,
            vs2: VReg::V3,
            vm: true,
            eew: Eew::E32,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::IllegalInstruction { .. }));
}

#[test]
fn vluxei_masked_vd_overlapping_v0_is_illegal() {
    let mut state = setup(2, Vsew::E32, Vlmul::M1);

    let err = exec_one(
        &mut state,
        Zve64xLoadInstruction::Vluxei {
            vd: VReg::V0,
            rs1: Reg::A0,
            vs2: VReg::V4,
            vm: false,
            eew: Eew::E32,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::IllegalInstruction { .. }));
}

// `Vloxei` tests

#[test]
fn vloxei_functionally_identical_to_vluxei() {
    // Ordered and unordered indexed loads produce the same result in an interpreter
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    let mut idx = [0u8; 16];
    idx[0] = 4;
    // index 0: offset 4
    idx[1..4].copy_from_slice(&[0u8, 0, 0]);
    idx[4] = 0; // index 1: offset 0
    set_vreg(&mut state, VReg::V5, &idx);

    state
        .memory
        .write::<u32>(TEST_BASE_ADDR, 0x1234_5678)
        .unwrap();
    state
        .memory
        .write::<u32>(TEST_BASE_ADDR + 4, 0x8765_4321)
        .unwrap();
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vloxei {
            vd: VReg::V1,
            rs1: Reg::A0,
            vs2: VReg::V5,
            vm: true,
            eew: Eew::E32,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    let reg = vreg_bytes(&state, VReg::V1);
    assert_eq!(
        u32::from_le_bytes(reg[0..4].try_into().unwrap()),
        0x8765_4321
    );
    assert_eq!(
        u32::from_le_bytes(reg[4..8].try_into().unwrap()),
        0x1234_5678
    );
}

// `Vlseg` tests

#[test]
fn vlseg_nf2_e8_interleaved_fields() {
    // Segment load nf=2, E8/M1: each segment has 2 consecutive bytes; vl=4
    // Memory layout: [f0e0, f1e0, f0e1, f1e1, f0e2, f1e2, f0e3, f1e3]
    // Field 0 -> V2, Field 1 -> V3
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    write_mem(
        &mut state,
        TEST_BASE_ADDR,
        &[10, 20, 11, 21, 12, 22, 13, 23],
    );
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vlseg {
            vd: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E8,
            nf: 2,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    // Field 0 (V2): elements 0-3 = [10, 11, 12, 13]
    let v2 = vreg_bytes(&state, VReg::V2);
    assert_eq!(v2[0], 10);
    assert_eq!(v2[1], 11);
    assert_eq!(v2[2], 12);
    assert_eq!(v2[3], 13);
    // Field 1 (V3): elements 0-3 = [20, 21, 22, 23]
    let v3 = vreg_bytes(&state, VReg::V3);
    assert_eq!(v3[0], 20);
    assert_eq!(v3[1], 21);
    assert_eq!(v3[2], 22);
    assert_eq!(v3[3], 23);
    assert_eq!(state.ext_state.vstart(), 0);
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
}

#[test]
fn vlseg_nf3_e32() {
    // nf=3, E32/M1: each segment has 3 x 4 bytes; vl=2
    // Memory: [f0e0(4B), f1e0(4B), f2e0(4B), f0e1(4B), f1e1(4B), f2e1(4B)]
    // Fields: V1, V2, V3
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    let data = array::from_fn::<_, 24, _>(|i| i as u8 + 1);
    write_mem(&mut state, TEST_BASE_ADDR, &data);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vlseg {
            vd: VReg::V1,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E32,
            nf: 3,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    // Segment 0: bytes 0-11 = fields 0,1,2 of element 0
    // Segment 1: bytes 12-23 = fields 0,1,2 of element 1
    // f0e0
    assert_eq!(vreg_bytes(&state, VReg::V1)[0..4], data[0..4]);
    // f1e0
    assert_eq!(vreg_bytes(&state, VReg::V2)[0..4], data[4..8]);
    // f2e0
    assert_eq!(vreg_bytes(&state, VReg::V3)[0..4], data[8..12]);
    // f0e1
    assert_eq!(vreg_bytes(&state, VReg::V1)[4..8], data[12..16]);
    // f1e1
    assert_eq!(vreg_bytes(&state, VReg::V2)[4..8], data[16..20]);
    // f2e1
    assert_eq!(vreg_bytes(&state, VReg::V3)[4..8], data[20..24]);
}

#[test]
fn vlseg_register_group_overflow_is_illegal() {
    // E8/M1: group_regs=1, nf=8, vd=V30: V30..V37 -> 38 >= 32, overflow
    let mut state = setup(2, Vsew::E8, Vlmul::M1);

    let err = exec_one(
        &mut state,
        Zve64xLoadInstruction::Vlseg {
            vd: VReg::V30,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E8,
            nf: 8,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::IllegalInstruction { .. }));
}

#[test]
fn vlseg_masked_vd_at_v0_is_illegal() {
    let mut state = setup(2, Vsew::E8, Vlmul::M1);

    let err = exec_one(
        &mut state,
        Zve64xLoadInstruction::Vlseg {
            vd: VReg::V0,
            rs1: Reg::A0,
            vm: false,
            eew: Eew::E8,
            nf: 2,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::IllegalInstruction { .. }));
}

// `Vlsegff` tests

#[test]
fn vlsegff_no_fault_loads_all_segments() {
    // Same as vlseg with no faults
    let mut state = setup(3, Vsew::E8, Vlmul::M1);
    write_mem(&mut state, TEST_BASE_ADDR, &[1, 2, 3, 4, 5, 6]);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vlsegff {
            vd: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E8,
            nf: 2,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    assert_eq!(vreg_byte(&state, VReg::V2, 0), 1);
    assert_eq!(vreg_byte(&state, VReg::V3, 0), 2);
    assert_eq!(vreg_byte(&state, VReg::V2, 1), 3);
    assert_eq!(vreg_byte(&state, VReg::V3, 1), 4);
    assert_eq!(vreg_byte(&state, VReg::V2, 2), 5);
    assert_eq!(vreg_byte(&state, VReg::V3, 2), 6);
    assert_eq!(state.ext_state.vl(), 3);
}

#[test]
fn vlsegff_fault_at_segment_1_truncates_vl() {
    // nf=2, E8/M1: vl=3; only 2 bytes at end of memory (enough for segment 0 of element 0 only)
    // Place base address so that element 0's first field is valid but anything further faults
    let mem_end = TEST_BASE_ADDR + 8192;
    // Place base so element 0 fully loaded (2 bytes), element 1 faults
    let base = mem_end - 2;
    let mut state = setup(3, Vsew::E8, Vlmul::M1);
    write_mem(&mut state, base, &[0xAAu8, 0xBBu8]);
    state.regs.write(Reg::A0, base);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vlsegff {
            vd: VReg::V4,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E8,
            nf: 2,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    // Element 0 loaded (both fields)
    assert_eq!(vreg_byte(&state, VReg::V4, 0), 0xAA);
    assert_eq!(vreg_byte(&state, VReg::V5, 0), 0xBB);
    // vl truncated to 1 (fault at element index 1)
    assert_eq!(state.ext_state.vl(), 1);
}

// `Vlsseg` tests

#[test]
fn vlsseg_nf2_e32_with_stride() {
    // nf=2, E32/M1: stride=16, vl=2
    // addr[i] = base + i*16; field f at addr[i] + f*4
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    // Element 0 at base+0: fields [0xAAAA, 0xBBBB]
    state
        .memory
        .write::<u32>(TEST_BASE_ADDR, 0xAAAA_AAAA)
        .unwrap();
    state
        .memory
        .write::<u32>(TEST_BASE_ADDR + 4, 0xBBBB_BBBB)
        .unwrap();
    // Element 1 at base+16: fields [0xCCCC, 0xDDDD]
    state
        .memory
        .write::<u32>(TEST_BASE_ADDR + 16, 0xCCCC_CCCC)
        .unwrap();
    state
        .memory
        .write::<u32>(TEST_BASE_ADDR + 20, 0xDDDD_DDDD)
        .unwrap();
    state.regs.write(Reg::A0, TEST_BASE_ADDR);
    state.regs.write(Reg::A1, 16u64);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vlsseg {
            vd: VReg::V2,
            rs1: Reg::A0,
            rs2: Reg::A1,
            vm: true,
            eew: Eew::E32,
            nf: 2,
        },
    )
    .unwrap();

    let v2 = vreg_bytes(&state, VReg::V2);
    let v3 = vreg_bytes(&state, VReg::V3);
    assert_eq!(
        u32::from_le_bytes(v2[0..4].try_into().unwrap()),
        0xAAAA_AAAA
    );
    assert_eq!(
        u32::from_le_bytes(v2[4..8].try_into().unwrap()),
        0xCCCC_CCCC
    );
    assert_eq!(
        u32::from_le_bytes(v3[0..4].try_into().unwrap()),
        0xBBBB_BBBB
    );
    assert_eq!(
        u32::from_le_bytes(v3[4..8].try_into().unwrap()),
        0xDDDD_DDDD
    );
    assert_eq!(state.ext_state.vstart(), 0);
}

// Fault cases

#[test]
fn vlsseg_fault_at_f1_of_i0_marks_vs_dirty_and_sets_vstart() {
    // nf=2, E32/M1: base points to memory with exactly 4 valid bytes (one field).
    // Element 0, field 0 loads successfully; field 1 faults.
    // Since f>0 at fault time, VS must be marked dirty and vstart set to 0.
    let mem_end = TEST_BASE_ADDR + 8192;
    let base = mem_end - 4; // exactly 4 bytes (one E32 element) before end of memory
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    state.memory.write::<u32>(base, 0xDEAD_BEEF).unwrap();
    state.regs.write(Reg::A0, base);
    state.regs.write(Reg::A1, 8u64); // stride

    let err = exec_one(
        &mut state,
        Zve64xLoadInstruction::Vlsseg {
            vd: VReg::V2,
            rs1: Reg::A0,
            rs2: Reg::A1,
            vm: true,
            eew: Eew::E32,
            nf: 2,
        },
    )
    .unwrap_err();

    assert!(matches!(err, ExecutionError::MemoryAccess(_)));
    // f>0 at the fault point: field 0 of element 0 was written
    assert_eq!(
        state.ext_state.vs_dirty_count(),
        1,
        "VS must be marked dirty"
    );
    assert_eq!(
        state.ext_state.vstart(),
        0u16,
        "vstart must record the faulting element"
    );
    // Field 0 of element 0 was written
    let v2 = vreg_bytes(&state, VReg::V2);
    assert_eq!(
        u32::from_le_bytes(v2[0..4].try_into().unwrap()),
        0xDEAD_BEEF
    );
}

#[test]
fn vlsseg_fault_at_i1_f0_marks_vs_dirty_and_sets_vstart() {
    // nf=2, E8/M1: two full segments (4 bytes each) at base; third segment faults.
    // vl=3, stride=2 (each segment is 2 bytes wide).
    // Elements 0 and 1 (i=0,1) load cleanly; element 2 (i=2) faults on field 0.
    // Since i>vstart at fault: dirty + vstart=2.
    let mem_end = TEST_BASE_ADDR + 8192;
    let base = mem_end - 4;
    let mut state = setup(3, Vsew::E8, Vlmul::M1);
    write_mem(&mut state, base, &[0xAAu8, 0xBBu8, 0xCCu8, 0xDDu8]);
    state.regs.write(Reg::A0, base);
    state.regs.write(Reg::A1, 2u64); // stride=2

    let err = exec_one(
        &mut state,
        Zve64xLoadInstruction::Vlsseg {
            vd: VReg::V2,
            rs1: Reg::A0,
            rs2: Reg::A1,
            vm: true,
            eew: Eew::E8,
            nf: 2,
        },
    )
    .unwrap_err();

    assert!(matches!(err, ExecutionError::MemoryAccess(_)));
    assert_eq!(state.ext_state.vs_dirty_count(), 1);
    assert_eq!(state.ext_state.vstart(), 2u16);
}

// `Vluxseg` tests

#[test]
fn vluxseg_nf2_e32_indexed() {
    // nf=2, SEW=E32/M1, index EEW=E32, vl=2
    // Indices in vs2: [8, 0] -> data at base+8 and base+0
    // For each element: 2 fields at data_addr+0 and data_addr+4
    let mut state = setup(2, Vsew::E32, Vlmul::M1);

    let data_base = TEST_BASE_ADDR;
    state.memory.write::<u32>(data_base, 0x1111_1111).unwrap();
    state
        .memory
        .write::<u32>(data_base + 4, 0x2222_2222)
        .unwrap();
    state
        .memory
        .write::<u32>(data_base + 8, 0x3333_3333)
        .unwrap();
    state
        .memory
        .write::<u32>(data_base + 12, 0x4444_4444)
        .unwrap();

    // Set indices [8, 0] as u32 LE in V8
    let mut idx_bytes = [0u8; 16];
    idx_bytes[0..4].copy_from_slice(&8u32.to_le_bytes());
    idx_bytes[4..8].copy_from_slice(&0u32.to_le_bytes());
    set_vreg(&mut state, VReg::V8, &idx_bytes);

    state.regs.write(Reg::A0, data_base);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vluxseg {
            vd: VReg::V2,
            rs1: Reg::A0,
            vs2: VReg::V8,
            vm: true,
            eew: Eew::E32,
            nf: 2,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    let v2 = vreg_bytes(&state, VReg::V2);
    let v3 = vreg_bytes(&state, VReg::V3);
    // Element 0: offset=8 -> fields at base+8 (0x3333_3333) and base+12 (0x4444_4444)
    assert_eq!(
        u32::from_le_bytes(v2[0..4].try_into().unwrap()),
        0x3333_3333
    );
    assert_eq!(
        u32::from_le_bytes(v3[0..4].try_into().unwrap()),
        0x4444_4444
    );
    // Element 1: offset=0 -> fields at base+0 (0x1111_1111) and base+4 (0x2222_2222)
    assert_eq!(
        u32::from_le_bytes(v2[4..8].try_into().unwrap()),
        0x1111_1111
    );
    assert_eq!(
        u32::from_le_bytes(v3[4..8].try_into().unwrap()),
        0x2222_2222
    );
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vluxseg_field_vs2_overlap_is_illegal() {
    // nf=2, data_group_regs=1, index_group_regs=1; vs2=V3 overlaps field 1 (V2+1=V3)
    let mut state = setup(2, Vsew::E32, Vlmul::M1);

    let err = exec_one(
        &mut state,
        Zve64xLoadInstruction::Vluxseg {
            vd: VReg::V2,
            rs1: Reg::A0,
            vs2: VReg::V3,
            vm: true,
            eew: Eew::E32,
            nf: 2,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExecutionError::IllegalInstruction { .. }));
}

// `Vloxseg` tests

#[test]
fn vloxseg_same_result_as_vluxseg() {
    // Ordered segment indexed is functionally identical to unordered in an interpreter
    let mut state = setup(2, Vsew::E32, Vlmul::M1);

    let data_base = TEST_BASE_ADDR;
    state.memory.write::<u32>(data_base, 0xABCD_EF01).unwrap();
    state
        .memory
        .write::<u32>(data_base + 4, 0x10FE_DCBA)
        .unwrap();

    let mut idx_bytes = [0u8; 16];
    // elem 0 -> offset 4
    idx_bytes[0..4].copy_from_slice(&4u32.to_le_bytes());
    // elem 1 -> offset 0
    idx_bytes[4..8].copy_from_slice(&0u32.to_le_bytes());
    set_vreg(&mut state, VReg::V6, &idx_bytes);

    state.regs.write(Reg::A0, data_base);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vloxseg {
            vd: VReg::V2,
            rs1: Reg::A0,
            vs2: VReg::V6,
            vm: true,
            eew: Eew::E32,
            nf: 1,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    let v2 = vreg_bytes(&state, VReg::V2);
    assert_eq!(
        u32::from_le_bytes(v2[0..4].try_into().unwrap()),
        0x10FE_DCBA
    );
    assert_eq!(
        u32::from_le_bytes(v2[4..8].try_into().unwrap()),
        0xABCD_EF01
    );
}

// vstart invariants

#[test]
fn all_non_vlr_loads_reset_vstart_on_success() {
    // Test each non-Vlr instruction resets vstart=0 after a clean execution.
    // We use a simple Vle as the representative, but the helper is called for each variant.
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    write_mem(&mut state, TEST_BASE_ADDR, &[0u8; 32]);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);
    // stride
    state.regs.write(Reg::A1, 4u64);

    let indices_bytes = {
        let mut b = [0u8; 16];
        b[4..8].copy_from_slice(&0u32.to_le_bytes());
        b
    };
    set_vreg(&mut state, VReg::V8, &indices_bytes);

    for instr in [
        Zve64xLoadInstruction::Vle {
            vd: VReg::V1,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E32,
            rs2: Reg::Zero,
        },
        Zve64xLoadInstruction::Vleff {
            vd: VReg::V1,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E32,
            rs2: Reg::Zero,
        },
        Zve64xLoadInstruction::Vlse {
            vd: VReg::V1,
            rs1: Reg::A0,
            rs2: Reg::A1,
            vm: true,
            eew: Eew::E32,
        },
        Zve64xLoadInstruction::Vlm {
            vd: VReg::V1,
            rs1: Reg::A0,
            rs2: Reg::Zero,
        },
        Zve64xLoadInstruction::Vluxei {
            vd: VReg::V1,
            rs1: Reg::A0,
            vs2: VReg::V8,
            vm: true,
            eew: Eew::E32,
            rs2: Reg::Zero,
        },
        Zve64xLoadInstruction::Vloxei {
            vd: VReg::V1,
            rs1: Reg::A0,
            vs2: VReg::V8,
            vm: true,
            eew: Eew::E32,
            rs2: Reg::Zero,
        },
    ] {
        state.ext_state.set_vstart(5);
        exec_one(&mut state, instr).unwrap();
        assert_eq!(
            state.ext_state.vstart(),
            0,
            "vstart not reset for {instr:?}"
        );
    }
}

// mark_vs_dirty invariants

#[test]
fn mark_vs_dirty_called_exactly_once_on_success() {
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    write_mem(&mut state, TEST_BASE_ADDR, &[0u8; 32]);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vle {
            vd: VReg::V1,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E8,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    assert_eq!(state.ext_state.vs_dirty_count(), 1);
}

#[test]
fn mark_vs_dirty_not_called_on_illegal_instruction_error() {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    // vtype is vill -> IllegalInstruction before any register writes
    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vle {
            vd: VReg::V1,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E32,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();
    assert_eq!(state.ext_state.vs_dirty_count(), 0);
}

// Element width boundary tests

#[test]
fn vle_e8_all_elements_across_register_boundary_m2() {
    // E8/M2: VLMAX=32, vl=20, group spans V2 and V3
    let mut state = setup(20, Vsew::E8, Vlmul::M2);
    let data = array::from_fn::<_, 20, _>(|i| i as u8 + 1);
    write_mem(&mut state, TEST_BASE_ADDR, &data);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vle {
            vd: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E8,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    // Elements 0-15 in V2, elements 16-19 in V3
    for i in 0..16usize {
        assert_eq!(vreg_byte(&state, VReg::V2, i), (i + 1) as u8, "V2[{i}]");
    }
    for i in 0..4usize {
        assert_eq!(vreg_byte(&state, VReg::V3, i), (17 + i) as u8, "V3[{i}]");
    }
}

#[test]
fn vle_e16_loads_half_words() {
    // E16/M1: VLMAX=8, vl=3
    let mut state = setup(3, Vsew::E16, Vlmul::M1);
    let vals = [0x0102_u16, 0x0304, 0x0506];
    let data = vals.map(u16::to_le_bytes);
    write_mem(&mut state, TEST_BASE_ADDR, data.as_flattened());
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vle {
            vd: VReg::V1,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E16,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    let reg = vreg_bytes(&state, VReg::V1);
    assert_eq!(u16::from_le_bytes(reg[0..2].try_into().unwrap()), 0x0102);
    assert_eq!(u16::from_le_bytes(reg[2..4].try_into().unwrap()), 0x0304);
    assert_eq!(u16::from_le_bytes(reg[4..6].try_into().unwrap()), 0x0506);
}

// vl=VLMAX edge case

#[test]
fn vle_vl_equals_vlmax_loads_all_elements() {
    // E8/M1: VLMAX=16, vl=16 (maximum)
    let mut state = setup(16, Vsew::E8, Vlmul::M1);
    let data = array::from_fn::<_, 16, _>(|i| i as u8);
    write_mem(&mut state, TEST_BASE_ADDR, &data);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vle {
            vd: VReg::V1,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E8,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    assert_eq!(vreg_bytes(&state, VReg::V1), data);
}

// Fractional LMUL

#[test]
fn vle_fractional_lmul_mf2_e8() {
    // E8/Mf2: VLMAX = VLEN/(SEW*2) = 128/(8*2) = 8; group_regs=1
    let mut state = setup(4, Vsew::E8, Vlmul::Mf2);
    write_mem(&mut state, TEST_BASE_ADDR, &[5u8, 6, 7, 8]);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vle {
            vd: VReg::V1,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E8,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    assert_eq!(vreg_byte(&state, VReg::V1, 0), 5);
    assert_eq!(vreg_byte(&state, VReg::V1, 1), 6);
    assert_eq!(vreg_byte(&state, VReg::V1, 2), 7);
    assert_eq!(vreg_byte(&state, VReg::V1, 3), 8);
}

// Mask spanning multiple bytes

#[test]
fn vle_mask_spanning_two_bytes() {
    // E8/M1: vl=12, mask uses bytes 0 and 1
    // mask_byte0=0b1100_1010, mask_byte1=0b0000_1101 -> active: 1,3,6,7,8,10,11
    let mut state = setup(12, Vsew::E8, Vlmul::M1);
    set_vreg(&mut state, VReg::V2, &[0xEEu8; 16]);
    set_vreg(&mut state, VReg::V0, &{
        let mut m = [0u8; 16];
        m[0] = 0b1100_1010;
        m[1] = 0b0000_1101;
        m
    });
    let data = array::from_fn::<_, 12, _>(|i| i as u8 + 1);
    write_mem(&mut state, TEST_BASE_ADDR, &data);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vle {
            vd: VReg::V2,
            rs1: Reg::A0,
            vm: false,
            eew: Eew::E8,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    let reg = vreg_bytes(&state, VReg::V2);
    // Active bits in mask_byte0 (0b1100_1010): bits 1,3,6,7
    // Active bits in mask_byte1 (0b0000_1101): bits 0,2,3 -> elements 8,10,11
    let active: &[usize] = &[1, 3, 6, 7, 8, 10, 11];
    for (i, &byte) in reg.iter().enumerate().take(12usize) {
        if active.contains(&i) {
            assert_eq!(byte, (i + 1) as u8, "element {i} should be loaded");
        } else {
            assert_eq!(byte, 0xEE, "element {i} should be undisturbed");
        }
    }
}

// Fault cases

#[test]
fn vle_fault_after_first_element_marks_vs_dirty() {
    // E8/M1: vl=4; only 2 bytes accessible, so element 0 succeeds and element 2 faults.
    // vs_dirty must be marked even though the instruction returns an error.
    let mem_end = TEST_BASE_ADDR + 8192;
    let base = mem_end - 2;
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    write_mem(&mut state, base, &[0xAAu8, 0xBBu8]);
    state.regs.write(Reg::A0, base);

    let err = exec_one(
        &mut state,
        Zve64xLoadInstruction::Vle {
            vd: VReg::V1,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E8,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();

    assert!(matches!(err, ExecutionError::MemoryAccess(_)));
    // Elements 0 and 1 were committed before the fault at element 2.
    assert_eq!(vreg_byte(&state, VReg::V1, 0), 0xAA, "element 0 committed");
    assert_eq!(vreg_byte(&state, VReg::V1, 1), 0xBB, "element 1 committed");
    assert_eq!(
        state.ext_state.vs_dirty_count(),
        1,
        "vs_dirty must be marked after partial write"
    );
}

#[test]
fn vle_fault_after_first_element_sets_vstart_to_faulting_index() {
    // E8/M1: vl=4; only 2 bytes accessible, fault at element 2.
    // vstart must be set to 2 (the faulting element index) for restartability.
    let mem_end = TEST_BASE_ADDR + 8192;
    let base = mem_end - 2;
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    write_mem(&mut state, base, &[0x11u8, 0x22u8]);
    state.regs.write(Reg::A0, base);

    let err = exec_one(
        &mut state,
        Zve64xLoadInstruction::Vle {
            vd: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E8,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();

    assert!(matches!(err, ExecutionError::MemoryAccess(_)));
    assert_eq!(
        state.ext_state.vstart(),
        2,
        "vstart must record the faulting element index"
    );
}

#[test]
fn vle_fault_at_first_element_does_not_mark_vs_dirty() {
    // Element 0 itself faults (address 0 is out of bounds); nothing was written, so
    // vs_dirty must not be marked and vstart must remain unchanged.
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    state.regs.write(Reg::A0, 0);

    let err = exec_one(
        &mut state,
        Zve64xLoadInstruction::Vle {
            vd: VReg::V1,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E8,
            rs2: Reg::Zero,
        },
    )
    .unwrap_err();

    assert!(matches!(err, ExecutionError::MemoryAccess(_)));
    assert_eq!(
        state.ext_state.vs_dirty_count(),
        0,
        "vs_dirty must not be marked when no element was written"
    );
    assert_eq!(
        state.ext_state.vstart(),
        0,
        "vstart must not be modified when fault is at the first element"
    );
}

// Vlr with nreg=8

#[test]
fn vlr_eight_registers() {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    let data = array::from_fn::<_, 128, _>(|i| i as u8);
    write_mem(&mut state, TEST_BASE_ADDR, &data);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xLoadInstruction::Vlr {
            vd: VReg::V0,
            rs1: Reg::A0,
            nreg: 8,
            eew: Eew::E8,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    for r in 0u8..8 {
        let reg = VReg::from_bits(r).unwrap();
        let expected: [u8; 16] = data[r as usize * 16..(r as usize + 1) * 16]
            .try_into()
            .unwrap();
        assert_eq!(vreg_bytes(&state, reg), expected, "register v{r}");
    }
}
