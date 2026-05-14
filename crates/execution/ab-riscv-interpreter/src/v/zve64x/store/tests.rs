use crate::basic::BasicRegisters;
use crate::rv64::test_utils::{
    ExtState, TEST_BASE_ADDR, TestInstructionFetcher, TestInstructionHandler, TestInterpreterState,
    TestMemory, initialize_state,
};
use crate::v::vector_registers::{VectorRegisters, VectorRegistersExt};
use crate::{
    ExecutableInstruction, ExecutionError, RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands,
    VirtualMemory,
};
use ab_riscv_primitives::prelude::*;
use core::array;
// With TEST_VLEN=128 and TEST_VLENB=16:
//   E8/M1  VLMAX=16, E16/M1 VLMAX=8, E32/M1 VLMAX=4, E64/M1 VLMAX=2
//   E8/M2  VLMAX=32, E16/M2 VLMAX=16
//   E8/M4  VLMAX=64
//   E8/Mf2 VLMAX=8,  E16/Mf2 VLMAX=4

fn setup(
    vl: u32,
    vsew: Vsew,
    vlmul: Vlmul,
) -> TestInterpreterState<Zve64xStoreInstruction<Reg<u64>>> {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    let vtype = Vtype::from_raw::<Reg<u64>>(encode_vtype(vsew, vlmul)).unwrap();
    state.ext_state.set_vtype(Some(vtype));
    state.ext_state.set_vl(vl);
    state.ext_state.set_vstart(0);
    state
}

fn encode_vtype(vsew: Vsew, vlmul: Vlmul) -> u64 {
    (vlmul.to_bits() as u64) | ((vsew.to_bits() as u64) << 3)
}

fn set_vreg(
    state: &mut TestInterpreterState<Zve64xStoreInstruction<Reg<u64>>>,
    reg: VReg,
    data: &[u8],
) {
    let dst = &mut state.ext_state.write_vreg()[usize::from(reg.bits())];
    dst[..data.len()].copy_from_slice(data);
}

fn read_mem_bytes<const N: usize>(
    state: &TestInterpreterState<Zve64xStoreInstruction<Reg<u64>>>,
    addr: u64,
) -> &[u8; N] {
    state
        .memory
        .read_slice(addr, N as u32)
        .unwrap()
        .try_into()
        .unwrap()
}

fn exec_one(
    state: &mut TestInterpreterState<Zve64xStoreInstruction<Reg<u64>>>,
    instr: Zve64xStoreInstruction<Reg<u64>>,
) -> Result<(), ExecutionError<u64>> {
    let Rs1Rs2Operands { rs1, rs2 } = <_ as ExecutableInstruction<
        BasicRegisters<_>,
        ExtState,
        TestMemory,
        TestInstructionFetcher<Zve64xWidenNarrowInstruction<_>>,
        TestInstructionHandler,
    >>::get_rs1_rs2_operands(instr);
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

// Vsr (whole-register store)

#[test]
fn vsr_single_register_stores_vlenb_bytes() {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    let data = array::from_fn::<_, 16, _>(|i| i as u8 + 1);
    set_vreg(&mut state, VReg::V2, &data);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsr {
            vs3: VReg::V2,
            rs1: Reg::A0,
            nreg: 1,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    assert_eq!(read_mem_bytes::<16>(&state, TEST_BASE_ADDR), &data);
    // Stores must not mark vector state dirty
    assert_eq!(state.ext_state.vs_dirty_count(), 0);
}

#[test]
fn vsr_two_registers_stores_two_vlenb_blocks() {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    let data0 = array::from_fn::<_, 16, _>(|i| i as u8);
    let data1 = array::from_fn::<_, 16, _>(|i| i as u8 + 16);
    set_vreg(&mut state, VReg::V2, &data0);
    set_vreg(&mut state, VReg::V3, &data1);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsr {
            vs3: VReg::V2,
            rs1: Reg::A0,
            nreg: 2,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    assert_eq!(read_mem_bytes::<16>(&state, TEST_BASE_ADDR), &data0);
    assert_eq!(read_mem_bytes::<16>(&state, TEST_BASE_ADDR + 16), &data1);
}

#[test]
fn vsr_four_registers_stores_four_vlenb_blocks() {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    for i in 0u8..4 {
        let data = array::from_fn::<_, 16, _>(|j| i * 16 + j as u8);
        set_vreg(&mut state, VReg::from_bits(4 + i).unwrap(), &data);
    }
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsr {
            vs3: VReg::V4,
            rs1: Reg::A0,
            nreg: 4,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    for i in 0u8..4 {
        let expected = array::from_fn::<_, 16, _>(|j| i * 16 + j as u8);
        assert_eq!(
            read_mem_bytes::<16>(&state, TEST_BASE_ADDR + u64::from(i) * 16),
            &expected
        );
    }
}

#[test]
fn vsr_eight_registers_stores_eight_vlenb_blocks() {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    for i in 0u8..8 {
        let data = array::from_fn::<_, 16, _>(|j| i * 16 + j as u8);
        set_vreg(&mut state, VReg::from_bits(8 + i).unwrap(), &data);
    }
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsr {
            vs3: VReg::V8,
            rs1: Reg::A0,
            nreg: 8,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    for i in 0u8..8 {
        let expected = array::from_fn::<_, 16, _>(|j| i * 16 + j as u8);
        assert_eq!(
            read_mem_bytes::<16>(&state, TEST_BASE_ADDR + u64::from(i) * 16),
            &expected
        );
    }
}

#[test]
fn vsr_misaligned_register_returns_illegal_instruction() {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    // V3 is not aligned to nreg=2
    let result = exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsr {
            vs3: VReg::V3,
            rs1: Reg::A0,
            nreg: 2,
            rs2: Reg::Zero,
        },
    );

    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

#[test]
fn vsr_ignores_vtype_and_vl() {
    // vsr must work even when vtype=None (vill=1)
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    // Leave vtype as illegal (default)
    state.ext_state.set_vtype(None);
    state.ext_state.set_vl(0);
    let data = array::from_fn::<_, 16, _>(|i| i as u8 + 0xAA);
    set_vreg(&mut state, VReg::V0, &data);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsr {
            vs3: VReg::V0,
            rs1: Reg::A0,
            nreg: 1,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    assert_eq!(read_mem_bytes::<16>(&state, TEST_BASE_ADDR), &data);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vsr_vector_not_allowed_returns_illegal_instruction() {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    state.ext_state.set_vector_allowed(false);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    let result = exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsr {
            vs3: VReg::V0,
            rs1: Reg::A0,
            nreg: 1,
            rs2: Reg::Zero,
        },
    );

    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

#[test]
fn vsr_honors_nonzero_vstart() {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    let data = array::from_fn::<_, 16, _>(|i| i as u8 + 1);
    set_vreg(&mut state, VReg::V2, &data);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);
    for i in 0u64..16 {
        state.memory.write::<u8>(TEST_BASE_ADDR + i, 0xEE).unwrap();
    }
    state.ext_state.set_vstart(5);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsr {
            vs3: VReg::V2,
            rs1: Reg::A0,
            nreg: 1,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    for i in 0u64..5 {
        assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR + i).unwrap(), 0xEE);
    }
    for i in 5u64..16 {
        assert_eq!(
            state.memory.read::<u8>(TEST_BASE_ADDR + i).unwrap(),
            i as u8 + 1
        );
    }
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vsr_vstart_at_or_past_evl_writes_nothing() {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    set_vreg(&mut state, VReg::V2, &[0xAA; 16]);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);
    state.memory.write::<u8>(TEST_BASE_ADDR, 0x55).unwrap();
    // EVL = 1 * VLENB = 16; vstart = 16 => no-op
    state.ext_state.set_vstart(16);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsr {
            vs3: VReg::V2,
            rs1: Reg::A0,
            nreg: 1,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR).unwrap(), 0x55);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vsr_nreg2_vstart_spans_register_boundary() {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    let d0 = array::from_fn::<_, 16, _>(|i| i as u8);
    let d1 = array::from_fn::<_, 16, _>(|i| i as u8 + 16);
    set_vreg(&mut state, VReg::V2, &d0);
    set_vreg(&mut state, VReg::V3, &d1);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);
    for i in 0u64..32 {
        state.memory.write::<u8>(TEST_BASE_ADDR + i, 0xEE).unwrap();
    }
    // Start mid-second register (byte 20: v3, in-reg offset 4)
    state.ext_state.set_vstart(20);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsr {
            vs3: VReg::V2,
            rs1: Reg::A0,
            nreg: 2,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    for i in 0u64..20 {
        assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR + i).unwrap(), 0xEE);
    }
    // Bytes 20..32 correspond to v3 bytes 4..16, which equal (byte as u8) + 16
    for i in 20u64..32 {
        let in_reg = (i - 16) as u8;
        assert_eq!(
            state.memory.read::<u8>(TEST_BASE_ADDR + i).unwrap(),
            in_reg + 16
        );
    }
    assert_eq!(state.ext_state.vstart(), 0);
}

// Vsm (mask store)

#[test]
fn vsm_stores_ceil_vl_over_8_bytes() {
    // E8/M1: VLMAX=16. Set vl=9 -> ceil(9/8)=2 bytes written.
    let mut state = setup(9, Vsew::E8, Vlmul::M1);
    // v1 mask register: byte0=0xFF, byte1=0x01
    let mask = [
        0xFF, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ];
    set_vreg(&mut state, VReg::V1, &mask);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsm {
            vs3: VReg::V1,
            rs1: Reg::A0,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR).unwrap(), 0xFF);
    assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR + 1).unwrap(), 0x01);
    // Third byte must not have been written (still zero)
    assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR + 2).unwrap(), 0x00);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vsm_vl_zero_writes_nothing() {
    let mut state = setup(0, Vsew::E8, Vlmul::M1);
    let mask = [0xFF; 16];
    set_vreg(&mut state, VReg::V0, &mask);
    // Write sentinel to memory so we can detect any write
    state.memory.write::<u8>(TEST_BASE_ADDR, 0xAB).unwrap();
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsm {
            vs3: VReg::V0,
            rs1: Reg::A0,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    // vl=0: ceil(0/8)=0 bytes, sentinel intact
    assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR).unwrap(), 0xAB);
}

#[test]
fn vsm_vl_exactly_8_writes_one_byte() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    let mut mask = [0u8; 16];
    mask[0] = 0b10110101;
    set_vreg(&mut state, VReg::V3, &mask);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsm {
            vs3: VReg::V3,
            rs1: Reg::A0,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR).unwrap(), 0b10110101);
    assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR + 1).unwrap(), 0x00);
}

#[test]
fn vsm_vector_not_allowed_returns_illegal_instruction() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    state.ext_state.set_vector_allowed(false);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    let result = exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsm {
            vs3: VReg::V0,
            rs1: Reg::A0,
            rs2: Reg::Zero,
        },
    );

    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

#[test]
fn vsm_honors_vstart_in_byte_units_non_multiple_of_eight() {
    // vl=16 => ceil(vl/8) = 2 bytes to write.
    // vstart = 1 (byte units, NOT a multiple of 8) => skip the first byte of vs3,
    // write only the second byte of the mask register.
    // This test FAILS on the old buggy implementation (which did `vstart / 8`).
    // It PASSES after the fix (`start_byte = vstart` with no division).
    let mut state = setup(16, Vsew::E8, Vlmul::M1);

    let mut mask = [0u8; 16];
    mask[0] = 0xAA; // byte 0 – should be skipped
    mask[1] = 0xBB; // byte 1 – should be written
    set_vreg(&mut state, VReg::V1, &mask);

    // Sentinels so we can detect whether the first byte was wrongly overwritten
    state.memory.write::<u8>(TEST_BASE_ADDR, 0x11).unwrap();
    state.memory.write::<u8>(TEST_BASE_ADDR + 1, 0x22).unwrap();

    state.regs.write(Reg::A0, TEST_BASE_ADDR);
    state.ext_state.set_vstart(1); // ← the key edge-case value

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsm {
            vs3: VReg::V1,
            rs1: Reg::A0,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    // First output byte must be untouched (vstart=1 byte skipped)
    assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR).unwrap(), 0x11);
    // Only the second byte written
    assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR + 1).unwrap(), 0xBB);
    // No further bytes written
    assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR + 2).unwrap(), 0x00);

    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vsm_vstart_past_evl_writes_nothing() {
    // vl=8 => EVL = 1 byte; vstart=8 => start_byte = 1, equals EVL => no-op
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    set_vreg(&mut state, VReg::V1, &[0xFF; 16]);
    state.memory.write::<u8>(TEST_BASE_ADDR, 0x77).unwrap();
    state.regs.write(Reg::A0, TEST_BASE_ADDR);
    state.ext_state.set_vstart(8);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsm {
            vs3: VReg::V1,
            rs1: Reg::A0,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR).unwrap(), 0x77);
    assert_eq!(state.ext_state.vstart(), 0);
}

// Vse (unit-stride store)

#[test]
fn vse_e8_m1_stores_all_elements() {
    // VLMAX=16, store all 16 elements
    let mut state = setup(16, Vsew::E8, Vlmul::M1);
    let data = array::from_fn::<_, 16, _>(|i| i as u8 + 1);
    set_vreg(&mut state, VReg::V4, &data);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vse {
            vs3: VReg::V4,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E8,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    assert_eq!(read_mem_bytes::<16>(&state, TEST_BASE_ADDR), &data);
    assert_eq!(state.ext_state.vs_dirty_count(), 0);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vse_e32_m1_stores_partial_vl() {
    // VLMAX=4, use vl=3
    let mut state = setup(3, Vsew::E32, Vlmul::M1);
    // Pack three u32 values into v0: [1, 2, 3, 0]
    let mut vreg = [0u8; 16];
    vreg[0..4].copy_from_slice(&1u32.to_le_bytes());
    vreg[4..8].copy_from_slice(&2u32.to_le_bytes());
    vreg[8..12].copy_from_slice(&3u32.to_le_bytes());
    set_vreg(&mut state, VReg::V0, &vreg);
    // Sentinel at element 3 position (byte 12)
    state
        .memory
        .write::<u32>(TEST_BASE_ADDR + 12, 0xDEAD_BEEF)
        .unwrap();
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vse {
            vs3: VReg::V0,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E32,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    assert_eq!(state.memory.read::<u32>(TEST_BASE_ADDR).unwrap(), 1u32);
    assert_eq!(state.memory.read::<u32>(TEST_BASE_ADDR + 4).unwrap(), 2u32);
    assert_eq!(state.memory.read::<u32>(TEST_BASE_ADDR + 8).unwrap(), 3u32);
    // Element 3 must not have been overwritten
    assert_eq!(
        state.memory.read::<u32>(TEST_BASE_ADDR + 12).unwrap(),
        0xDEAD_BEEF
    );
}

#[test]
fn vse_e64_m1_stores_two_elements() {
    // VLMAX=2
    let mut state = setup(2, Vsew::E64, Vlmul::M1);
    let mut vreg = [0u8; 16];
    vreg[0..8].copy_from_slice(&0x0102030405060708u64.to_le_bytes());
    vreg[8..16].copy_from_slice(&0xAABBCCDDEEFF0011u64.to_le_bytes());
    set_vreg(&mut state, VReg::V8, &vreg);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vse {
            vs3: VReg::V8,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E64,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    assert_eq!(
        state.memory.read::<u64>(TEST_BASE_ADDR).unwrap(),
        0x0102030405060708u64
    );
    assert_eq!(
        state.memory.read::<u64>(TEST_BASE_ADDR + 8).unwrap(),
        0xAABBCCDDEEFF0011u64
    );
}

#[test]
fn vse_masked_skips_inactive_elements() {
    // E8/M1 VLMAX=16, vl=8, use first byte of v0 as mask
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    // mask: bits 0,2,4,6 set -> elements 0,2,4,6 active
    let mut mask = [0u8; 16];
    mask[0] = 0b01010101;
    set_vreg(&mut state, VReg::V0, &mask);
    let data = array::from_fn::<_, 16, _>(|i| (i as u8 + 1) * 10);
    set_vreg(&mut state, VReg::V2, &data);
    // Fill memory with sentinel values so inactive positions are distinguishable
    for i in 0u64..8 {
        state.memory.write::<u8>(TEST_BASE_ADDR + i, 0xFF).unwrap();
    }
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vse {
            vs3: VReg::V2,
            rs1: Reg::A0,
            vm: false,
            eew: Eew::E8,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    // Active elements written
    assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR).unwrap(), 10);
    assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR + 2).unwrap(), 30);
    assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR + 4).unwrap(), 50);
    assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR + 6).unwrap(), 70);
    // Inactive elements untouched
    assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR + 1).unwrap(), 0xFF);
    assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR + 3).unwrap(), 0xFF);
    assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR + 5).unwrap(), 0xFF);
    assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR + 7).unwrap(), 0xFF);
}

#[test]
fn vse_vstart_nonzero_skips_earlier_elements() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    let mut vreg = [0u8; 16];
    for i in 0u32..4 {
        vreg[i as usize * 4..(i as usize + 1) * 4].copy_from_slice(&(i + 1).to_le_bytes());
    }
    set_vreg(&mut state, VReg::V0, &vreg);
    // Mark elements 0 and 1 positions with sentinels
    state.memory.write::<u32>(TEST_BASE_ADDR, 0xDEAD).unwrap();
    state
        .memory
        .write::<u32>(TEST_BASE_ADDR + 4, 0xBEEF)
        .unwrap();
    state.regs.write(Reg::A0, TEST_BASE_ADDR);
    // Start from element 2
    state.ext_state.set_vstart(2);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vse {
            vs3: VReg::V0,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E32,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    // Elements 0 and 1 skipped: sentinels intact
    assert_eq!(state.memory.read::<u32>(TEST_BASE_ADDR).unwrap(), 0xDEAD);
    assert_eq!(
        state.memory.read::<u32>(TEST_BASE_ADDR + 4).unwrap(),
        0xBEEF
    );
    // Elements 2 and 3 written
    assert_eq!(state.memory.read::<u32>(TEST_BASE_ADDR + 8).unwrap(), 3u32);
    assert_eq!(state.memory.read::<u32>(TEST_BASE_ADDR + 12).unwrap(), 4u32);
    // vstart reset
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vse_masked_vs3_equals_v0_is_legal() {
    // Per RVV 1.0, vs3 is a source operand; source/v0 overlap is permitted for stores.
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    let mut mask_and_data = [0u8; 16];
    mask_and_data[0] = 0b11111111;
    set_vreg(&mut state, VReg::V0, &mask_and_data);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vse {
            vs3: VReg::V0,
            rs1: Reg::A0,
            vm: false,
            eew: Eew::E8,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR).unwrap(), 0b11111111);
}

#[test]
fn vse_vtype_illegal_returns_illegal_instruction() {
    let mut state = initialize_state([]);
    state.ext_state.init_vector_csrs();
    state.ext_state.set_vtype(None);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    let result = exec_one(
        &mut state,
        Zve64xStoreInstruction::Vse {
            vs3: VReg::V4,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E8,
            rs2: Reg::Zero,
        },
    );

    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

#[test]
fn vse_vector_not_allowed_returns_illegal_instruction() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    state.ext_state.set_vector_allowed(false);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    let result = exec_one(
        &mut state,
        Zve64xStoreInstruction::Vse {
            vs3: VReg::V4,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E32,
            rs2: Reg::Zero,
        },
    );

    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

// Vsse (strided store)

#[test]
fn vsse_positive_stride_stores_with_gap() {
    // E32/M1 VLMAX=4, vl=3, stride=8 (two u32 gaps)
    let mut state = setup(3, Vsew::E32, Vlmul::M1);
    let mut vreg = [0u8; 16];
    vreg[0..4].copy_from_slice(&10u32.to_le_bytes());
    vreg[4..8].copy_from_slice(&20u32.to_le_bytes());
    vreg[8..12].copy_from_slice(&30u32.to_le_bytes());
    set_vreg(&mut state, VReg::V0, &vreg);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);
    // stride = 8 bytes
    state.regs.write(Reg::A1, 8);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsse {
            vs3: VReg::V0,
            rs1: Reg::A0,
            rs2: Reg::A1,
            vm: true,
            eew: Eew::E32,
        },
    )
    .unwrap();

    assert_eq!(state.memory.read::<u32>(TEST_BASE_ADDR).unwrap(), 10u32);
    assert_eq!(state.memory.read::<u32>(TEST_BASE_ADDR + 8).unwrap(), 20u32);
    assert_eq!(
        state.memory.read::<u32>(TEST_BASE_ADDR + 16).unwrap(),
        30u32
    );
    // Gaps untouched
    assert_eq!(state.memory.read::<u32>(TEST_BASE_ADDR + 4).unwrap(), 0u32);
    assert_eq!(state.memory.read::<u32>(TEST_BASE_ADDR + 12).unwrap(), 0u32);
}

#[test]
fn vsse_zero_stride_writes_same_address_repeatedly() {
    // With stride=0 every active element overwrites the same location.
    // Last active element wins.
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    let data = array::from_fn::<_, 16, _>(|i| i as u8 + 1);
    set_vreg(&mut state, VReg::V1, &data);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);
    state.regs.write(Reg::A1, 0);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsse {
            vs3: VReg::V1,
            rs1: Reg::A0,
            rs2: Reg::A1,
            vm: true,
            eew: Eew::E8,
        },
    )
    .unwrap();

    // Element 3 (value=4) is last written to TEST_BASE_ADDR
    assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR).unwrap(), 4u8);
}

#[test]
fn vsse_negative_stride_stores_in_reverse() {
    // E32/M1, vl=3, stride=-4 (0xFFFF_FFFF_FFFF_FFFC as u64).
    // base points at the *last* slot; elements go backwards.
    let mut state = setup(3, Vsew::E32, Vlmul::M1);
    let mut vreg = [0u8; 16];
    vreg[0..4].copy_from_slice(&1u32.to_le_bytes());
    vreg[4..8].copy_from_slice(&2u32.to_le_bytes());
    vreg[8..12].copy_from_slice(&3u32.to_le_bytes());
    set_vreg(&mut state, VReg::V0, &vreg);
    // base = TEST_BASE_ADDR + 8 (points at element-0 slot when stride=-4)
    state.regs.write(Reg::A0, TEST_BASE_ADDR + 8);
    // stride = -4 as two's-complement u64
    state.regs.write(Reg::A1, (-4i64).cast_unsigned());

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsse {
            vs3: VReg::V0,
            rs1: Reg::A0,
            rs2: Reg::A1,
            vm: true,
            eew: Eew::E32,
        },
    )
    .unwrap();

    // Element 0 -> base+0*(-4) = TEST_BASE_ADDR+8
    assert_eq!(state.memory.read::<u32>(TEST_BASE_ADDR + 8).unwrap(), 1u32);
    // Element 1 -> base+1*(-4) = TEST_BASE_ADDR+4
    assert_eq!(state.memory.read::<u32>(TEST_BASE_ADDR + 4).unwrap(), 2u32);
    // Element 2 -> base+2*(-4) = TEST_BASE_ADDR+0
    assert_eq!(state.memory.read::<u32>(TEST_BASE_ADDR).unwrap(), 3u32);
}

#[test]
fn vsse_masked_skips_inactive_elements() {
    // E64/M1 VLMAX=2, vl=2, stride=16; mask bit 0 set, bit 1 clear
    let mut state = setup(2, Vsew::E64, Vlmul::M1);
    let mut mask = [0u8; 16];
    mask[0] = 0b00000001;
    set_vreg(&mut state, VReg::V0, &mask);
    let mut vreg = [0u8; 16];
    vreg[0..8].copy_from_slice(&0xAAAAAAAAAAAAAAAAu64.to_le_bytes());
    vreg[8..16].copy_from_slice(&0xBBBBBBBBBBBBBBBBu64.to_le_bytes());
    set_vreg(&mut state, VReg::V2, &vreg);
    // Sentinel for element 1 slot
    state
        .memory
        .write::<u64>(TEST_BASE_ADDR + 16, 0x1234567890ABCDEFu64)
        .unwrap();
    state.regs.write(Reg::A0, TEST_BASE_ADDR);
    state.regs.write(Reg::A1, 16);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsse {
            vs3: VReg::V2,
            rs1: Reg::A0,
            rs2: Reg::A1,
            vm: false,
            eew: Eew::E64,
        },
    )
    .unwrap();

    assert_eq!(
        state.memory.read::<u64>(TEST_BASE_ADDR).unwrap(),
        0xAAAAAAAAAAAAAAAAu64
    );
    // Element 1 inactive: sentinel untouched
    assert_eq!(
        state.memory.read::<u64>(TEST_BASE_ADDR + 16).unwrap(),
        0x1234567890ABCDEFu64
    );
}

// Vsuxei / Vsoxei (indexed stores)

#[test]
fn vsuxei_e32_data_e32_index_stores_at_indexed_addresses() {
    // SEW=E32/M1: VLMAX=4, vl=3
    // index EEW=E32; EMUL_index = (32/32)*1 = 1 -> also M1
    let mut state = setup(3, Vsew::E32, Vlmul::M1);
    // Data register v2: [100, 200, 300]
    let mut data_reg = [0u8; 16];
    data_reg[0..4].copy_from_slice(&100u32.to_le_bytes());
    data_reg[4..8].copy_from_slice(&200u32.to_le_bytes());
    data_reg[8..12].copy_from_slice(&300u32.to_le_bytes());
    set_vreg(&mut state, VReg::V2, &data_reg);
    // Index register v4: offsets [0, 8, 16] bytes from base
    let mut idx_reg = [0u8; 16];
    idx_reg[0..4].copy_from_slice(&0u32.to_le_bytes());
    idx_reg[4..8].copy_from_slice(&8u32.to_le_bytes());
    idx_reg[8..12].copy_from_slice(&16u32.to_le_bytes());
    set_vreg(&mut state, VReg::V4, &idx_reg);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsuxei {
            vs3: VReg::V2,
            rs1: Reg::A0,
            vs2: VReg::V4,
            vm: true,
            eew: Eew::E32,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    assert_eq!(state.memory.read::<u32>(TEST_BASE_ADDR).unwrap(), 100u32);
    assert_eq!(
        state.memory.read::<u32>(TEST_BASE_ADDR + 8).unwrap(),
        200u32
    );
    assert_eq!(
        state.memory.read::<u32>(TEST_BASE_ADDR + 16).unwrap(),
        300u32
    );
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vsoxei_e64_data_e64_index_stores_at_indexed_addresses() {
    // SEW=E64/M1: VLMAX=2, vl=2; index EEW=E64 -> EMUL=1
    let mut state = setup(2, Vsew::E64, Vlmul::M1);
    let mut data_reg = [0u8; 16];
    data_reg[0..8].copy_from_slice(&0xDEADBEEFDEADBEEFu64.to_le_bytes());
    data_reg[8..16].copy_from_slice(&0xCAFEBABECAFEBABEu64.to_le_bytes());
    set_vreg(&mut state, VReg::V2, &data_reg);
    // Offsets: 0 and 32
    let mut idx_reg = [0u8; 16];
    idx_reg[0..8].copy_from_slice(&0u64.to_le_bytes());
    idx_reg[8..16].copy_from_slice(&32u64.to_le_bytes());
    set_vreg(&mut state, VReg::V4, &idx_reg);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsoxei {
            vs3: VReg::V2,
            rs1: Reg::A0,
            vs2: VReg::V4,
            vm: true,
            eew: Eew::E64,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    assert_eq!(
        state.memory.read::<u64>(TEST_BASE_ADDR).unwrap(),
        0xDEADBEEFDEADBEEFu64
    );
    assert_eq!(
        state.memory.read::<u64>(TEST_BASE_ADDR + 32).unwrap(),
        0xCAFEBABECAFEBABEu64
    );
}

#[test]
fn vsuxei_e8_index_scatter_e8_data() {
    // SEW=E8/M1: VLMAX=16, vl=4; index EEW=E8 -> EMUL=1
    // Scatter four bytes to arbitrary offsets
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    let mut data_reg = [0u8; 16];
    data_reg[0] = 0xAA;
    data_reg[1] = 0xBB;
    data_reg[2] = 0xCC;
    data_reg[3] = 0xDD;
    set_vreg(&mut state, VReg::V8, &data_reg);
    // Scatter to offsets 5, 2, 0, 7
    let mut idx_reg = [0u8; 16];
    idx_reg[0] = 5;
    idx_reg[1] = 2;
    idx_reg[2] = 0;
    idx_reg[3] = 7;
    set_vreg(&mut state, VReg::V10, &idx_reg);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsuxei {
            vs3: VReg::V8,
            rs1: Reg::A0,
            vs2: VReg::V10,
            vm: true,
            eew: Eew::E8,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR + 5).unwrap(), 0xAA);
    assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR + 2).unwrap(), 0xBB);
    assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR).unwrap(), 0xCC);
    assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR + 7).unwrap(), 0xDD);
}

#[test]
fn vsuxei_masked_skips_inactive_elements() {
    // E32/M1, vl=4; mask has only bits 0 and 3 set
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    let mut mask = [0u8; 16];
    mask[0] = 0b00001001;
    set_vreg(&mut state, VReg::V0, &mask);
    let mut data_reg = [0u8; 16];
    for i in 0u32..4 {
        data_reg[i as usize * 4..(i as usize + 1) * 4]
            .copy_from_slice(&((i + 1) * 100).to_le_bytes());
    }
    set_vreg(&mut state, VReg::V2, &data_reg);
    let mut idx_reg = [0u8; 16];
    for i in 0u32..4 {
        idx_reg[i as usize * 4..(i as usize + 1) * 4].copy_from_slice(&(i * 8).to_le_bytes());
    }
    set_vreg(&mut state, VReg::V4, &idx_reg);
    // Sentinels at element 1 and 2 target addresses
    state
        .memory
        .write::<u32>(TEST_BASE_ADDR + 8, 0xDEAD)
        .unwrap();
    state
        .memory
        .write::<u32>(TEST_BASE_ADDR + 16, 0xBEEF)
        .unwrap();
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsuxei {
            vs3: VReg::V2,
            rs1: Reg::A0,
            vs2: VReg::V4,
            vm: false,
            eew: Eew::E32,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    assert_eq!(state.memory.read::<u32>(TEST_BASE_ADDR).unwrap(), 100u32);
    assert_eq!(
        state.memory.read::<u32>(TEST_BASE_ADDR + 24).unwrap(),
        400u32
    );
    // Inactive elements: sentinels intact
    assert_eq!(
        state.memory.read::<u32>(TEST_BASE_ADDR + 8).unwrap(),
        0xDEAD
    );
    assert_eq!(
        state.memory.read::<u32>(TEST_BASE_ADDR + 16).unwrap(),
        0xBEEF
    );
}

#[test]
fn vsuxei_misaligned_data_register_returns_illegal() {
    // M2 requires vs3 to be even; V3 is odd -> illegal
    let mut state = setup(4, Vsew::E32, Vlmul::M2);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);
    let idx_reg = [0u8; 16];
    set_vreg(&mut state, VReg::V4, &idx_reg);

    let result = exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsuxei {
            vs3: VReg::V3,
            rs1: Reg::A0,
            vs2: VReg::V4,
            vm: true,
            eew: Eew::E32,
            rs2: Reg::Zero,
        },
    );

    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

// Vsseg (unit-stride segment store)

#[test]
fn vsseg_nf2_e8_m1_stores_two_fields_interleaved() {
    // nf=2, SEW=E8/M1 VLMAX=16, vl=4
    // Field 0 in v2, field 1 in v3
    // Memory layout per element: [f0, f1], stride=nf*eew_bytes=2
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    let f0 = array::from_fn::<_, 16, _>(|i| i as u8 + 1);
    let f1 = array::from_fn::<_, 16, _>(|i| i as u8 + 17);
    set_vreg(&mut state, VReg::V2, &f0);
    set_vreg(&mut state, VReg::V3, &f1);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsseg {
            vs3: VReg::V2,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E8,
            nf: 2,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    // Element 0: [f0[0]=1, f1[0]=17]
    assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR).unwrap(), 1);
    assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR + 1).unwrap(), 17);
    // Element 1: [f0[1]=2, f1[1]=18]
    assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR + 2).unwrap(), 2);
    assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR + 3).unwrap(), 18);
    // Element 2: [f0[2]=3, f1[2]=19]
    assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR + 4).unwrap(), 3);
    assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR + 5).unwrap(), 19);
    // Element 3: [f0[3]=4, f1[3]=20]
    assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR + 6).unwrap(), 4);
    assert_eq!(state.memory.read::<u8>(TEST_BASE_ADDR + 7).unwrap(), 20);
    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vsseg_nf3_e32_m1_stores_three_fields_per_element() {
    // nf=3, SEW=E32/M1 VLMAX=4, vl=2
    // segment stride = 3 * 4 = 12 bytes per element
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    for (f, base_val) in [(VReg::V0, 1u32), (VReg::V1, 2u32), (VReg::V2, 3u32)] {
        let mut reg = [0u8; 16];
        reg[0..4].copy_from_slice(&base_val.to_le_bytes());
        reg[4..8].copy_from_slice(&(base_val + 10).to_le_bytes());
        set_vreg(&mut state, f, &reg);
    }
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsseg {
            vs3: VReg::V0,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E32,
            nf: 3,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    // Element 0 at offset 0: f0=1, f1=2, f2=3
    assert_eq!(state.memory.read::<u32>(TEST_BASE_ADDR).unwrap(), 1);
    assert_eq!(state.memory.read::<u32>(TEST_BASE_ADDR + 4).unwrap(), 2);
    assert_eq!(state.memory.read::<u32>(TEST_BASE_ADDR + 8).unwrap(), 3);
    // Element 1 at offset 12: f0=11, f1=12, f2=13
    assert_eq!(state.memory.read::<u32>(TEST_BASE_ADDR + 12).unwrap(), 11);
    assert_eq!(state.memory.read::<u32>(TEST_BASE_ADDR + 16).unwrap(), 12);
    assert_eq!(state.memory.read::<u32>(TEST_BASE_ADDR + 20).unwrap(), 13);
}

#[test]
fn vsseg_register_group_out_of_bounds_returns_illegal() {
    // nf=4, M1: need registers [V30, V31, V32, V33] -> V32/V33 out of range
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    let result = exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsseg {
            vs3: VReg::V30,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E32,
            nf: 4,
            rs2: Reg::Zero,
        },
    );

    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

#[test]
fn vsseg_masked_vs3_equals_v0_is_legal() {
    // Per RVV 1.0, vs3 is a source register group; source/v0 overlap is permitted for stores.
    let mut state = setup(4, Vsew::E8, Vlmul::M1);
    let mut mask_and_f0 = [0u8; 16];
    mask_and_f0[0] = 0b00001111;
    set_vreg(&mut state, VReg::V0, &mask_and_f0);
    let f1 = array::from_fn::<_, 16, _>(|i| i as u8 + 100);
    set_vreg(&mut state, VReg::V1, &f1);
    for i in 0u64..8 {
        state.memory.write::<u8>(TEST_BASE_ADDR + i, 0xEE).unwrap();
    }
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsseg {
            vs3: VReg::V0,
            rs1: Reg::A0,
            vm: false,
            eew: Eew::E8,
            nf: 2,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    // All 4 elements active (mask = 0b00001111). Per-element: [v0[i], v1[i]].
    for i in 0u64..4 {
        assert_eq!(
            state.memory.read::<u8>(TEST_BASE_ADDR + i * 2).unwrap(),
            if i == 0 { 0b00001111 } else { 0 }
        );
        assert_eq!(
            state.memory.read::<u8>(TEST_BASE_ADDR + i * 2 + 1).unwrap(),
            i as u8 + 100
        );
    }
}

// Vssseg (strided segment store)

#[test]
fn vssseg_nf2_e32_m1_stride_16_stores_correctly() {
    // nf=2, SEW=E32/M1, vl=2, stride=16
    // Element i at base + i*16; within element: [f0, f1] at +0, +4
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    let mut f0 = [0u8; 16];
    f0[0..4].copy_from_slice(&10u32.to_le_bytes());
    f0[4..8].copy_from_slice(&30u32.to_le_bytes());
    let mut f1 = [0u8; 16];
    f1[0..4].copy_from_slice(&20u32.to_le_bytes());
    f1[4..8].copy_from_slice(&40u32.to_le_bytes());
    set_vreg(&mut state, VReg::V2, &f0);
    set_vreg(&mut state, VReg::V3, &f1);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);
    // stride = 16 bytes between element starts
    state.regs.write(Reg::A1, 16);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vssseg {
            vs3: VReg::V2,
            rs1: Reg::A0,
            rs2: Reg::A1,
            vm: true,
            eew: Eew::E32,
            nf: 2,
        },
    )
    .unwrap();

    // Element 0 at TEST_BASE_ADDR + 0: f0=10 at +0, f1=20 at +4
    assert_eq!(state.memory.read::<u32>(TEST_BASE_ADDR).unwrap(), 10);
    assert_eq!(state.memory.read::<u32>(TEST_BASE_ADDR + 4).unwrap(), 20);
    // Element 1 at TEST_BASE_ADDR + 16: f0=30 at +0, f1=40 at +4
    assert_eq!(state.memory.read::<u32>(TEST_BASE_ADDR + 16).unwrap(), 30);
    assert_eq!(state.memory.read::<u32>(TEST_BASE_ADDR + 20).unwrap(), 40);
    // Gaps between segments untouched
    assert_eq!(state.memory.read::<u32>(TEST_BASE_ADDR + 8).unwrap(), 0);
    assert_eq!(state.memory.read::<u32>(TEST_BASE_ADDR + 12).unwrap(), 0);
}

// Vsuxseg / Vsoxseg (indexed segment stores)

#[test]
fn vsuxseg_nf2_e32_index_e32_data_stores_segments_at_indexed_addresses() {
    // nf=2, SEW=E32/M1 VLMAX=4, vl=2; index EEW=E32
    // vs3=V2 (f0), vs3+1=V3 (f1); vs2=V6 (indices)
    let mut state = setup(2, Vsew::E32, Vlmul::M1);
    let mut f0 = [0u8; 16];
    f0[0..4].copy_from_slice(&100u32.to_le_bytes());
    f0[4..8].copy_from_slice(&200u32.to_le_bytes());
    let mut f1 = [0u8; 16];
    f1[0..4].copy_from_slice(&101u32.to_le_bytes());
    f1[4..8].copy_from_slice(&201u32.to_le_bytes());
    set_vreg(&mut state, VReg::V2, &f0);
    set_vreg(&mut state, VReg::V3, &f1);
    // Scatter indices: element 0 -> offset 0, element 1 -> offset 32
    let mut idx = [0u8; 16];
    idx[0..4].copy_from_slice(&0u32.to_le_bytes());
    idx[4..8].copy_from_slice(&32u32.to_le_bytes());
    set_vreg(&mut state, VReg::V6, &idx);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsuxseg {
            vs3: VReg::V2,
            rs1: Reg::A0,
            vs2: VReg::V6,
            vm: true,
            eew: Eew::E32,
            nf: 2,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    // Element 0 segment at base+0: f0=100, f1=101
    assert_eq!(state.memory.read::<u32>(TEST_BASE_ADDR).unwrap(), 100);
    assert_eq!(state.memory.read::<u32>(TEST_BASE_ADDR + 4).unwrap(), 101);
    // Element 1 segment at base+32: f0=200, f1=201
    assert_eq!(state.memory.read::<u32>(TEST_BASE_ADDR + 32).unwrap(), 200);
    assert_eq!(state.memory.read::<u32>(TEST_BASE_ADDR + 36).unwrap(), 201);
}

#[test]
fn vsoxseg_nf2_e64_index_e64_data_stores_in_element_order() {
    // nf=2, SEW=E64/M1 VLMAX=2, vl=2; index EEW=E64
    let mut state = setup(2, Vsew::E64, Vlmul::M1);
    let mut f0 = [0u8; 16];
    f0[0..8].copy_from_slice(&0xAAAAAAAAAAAAAAAAu64.to_le_bytes());
    f0[8..16].copy_from_slice(&0xBBBBBBBBBBBBBBBBu64.to_le_bytes());
    let mut f1 = [0u8; 16];
    f1[0..8].copy_from_slice(&0xCCCCCCCCCCCCCCCCu64.to_le_bytes());
    f1[8..16].copy_from_slice(&0xDDDDDDDDDDDDDDDDu64.to_le_bytes());
    set_vreg(&mut state, VReg::V2, &f0);
    set_vreg(&mut state, VReg::V3, &f1);
    let mut idx = [0u8; 16];
    // element 0 -> offset 64, element 1 -> offset 0
    idx[0..8].copy_from_slice(&64u64.to_le_bytes());
    idx[8..16].copy_from_slice(&0u64.to_le_bytes());
    set_vreg(&mut state, VReg::V6, &idx);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsoxseg {
            vs3: VReg::V2,
            rs1: Reg::A0,
            vs2: VReg::V6,
            vm: true,
            eew: Eew::E64,
            nf: 2,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    // Element 0 at base+64: f0=0xAAAA..., f1=0xCCCC...
    assert_eq!(
        state.memory.read::<u64>(TEST_BASE_ADDR + 64).unwrap(),
        0xAAAAAAAAAAAAAAAAu64
    );
    assert_eq!(
        state.memory.read::<u64>(TEST_BASE_ADDR + 72).unwrap(),
        0xCCCCCCCCCCCCCCCCu64
    );
    // Element 1 at base+0: f0=0xBBBB..., f1=0xDDDD...
    assert_eq!(
        state.memory.read::<u64>(TEST_BASE_ADDR).unwrap(),
        0xBBBBBBBBBBBBBBBBu64
    );
    assert_eq!(
        state.memory.read::<u64>(TEST_BASE_ADDR + 8).unwrap(),
        0xDDDDDDDDDDDDDDDDu64
    );
}

// vstart invariant

#[test]
fn vse_resets_vstart_to_zero() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    let vreg = [0u8; 16];
    set_vreg(&mut state, VReg::V4, &vreg);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);
    state.ext_state.set_vstart(2);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vse {
            vs3: VReg::V4,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E32,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vsse_resets_vstart_to_zero() {
    let mut state = setup(2, Vsew::E64, Vlmul::M1);
    let vreg = [0u8; 16];
    set_vreg(&mut state, VReg::V4, &vreg);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);
    state.regs.write(Reg::A1, 8);
    state.ext_state.set_vstart(1);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsse {
            vs3: VReg::V4,
            rs1: Reg::A0,
            rs2: Reg::A1,
            vm: true,
            eew: Eew::E64,
        },
    )
    .unwrap();

    assert_eq!(state.ext_state.vstart(), 0);
}

#[test]
fn vsm_resets_vstart_to_zero() {
    let mut state = setup(8, Vsew::E8, Vlmul::M1);
    set_vreg(&mut state, VReg::V1, &[0u8; 16]);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);
    state.ext_state.set_vstart(3);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsm {
            vs3: VReg::V1,
            rs1: Reg::A0,
            rs2: Reg::Zero,
        },
    )
    .unwrap();

    assert_eq!(state.ext_state.vstart(), 0);
}

// vs_dirty invariant

#[test]
fn stores_never_mark_vs_dirty() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    let vreg = [0u8; 16];
    set_vreg(&mut state, VReg::V4, &vreg);
    state.regs.write(Reg::A0, TEST_BASE_ADDR);
    state.regs.write(Reg::A1, 4);

    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vse {
            vs3: VReg::V4,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E32,
            rs2: Reg::Zero,
        },
    )
    .unwrap();
    exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsse {
            vs3: VReg::V4,
            rs1: Reg::A0,
            rs2: Reg::A1,
            vm: true,
            eew: Eew::E32,
        },
    )
    .unwrap();

    assert_eq!(state.ext_state.vs_dirty_count(), 0);
}

// Memory error propagation

#[test]
fn vse_out_of_bounds_write_returns_memory_access_error() {
    let mut state = setup(4, Vsew::E32, Vlmul::M1);
    let vreg = [0u8; 16];
    set_vreg(&mut state, VReg::V0, &vreg);
    // Write past the end of the 8192-byte TestMemory window
    state.regs.write(Reg::A0, TEST_BASE_ADDR + 8192 - 4);

    let result = exec_one(
        &mut state,
        Zve64xStoreInstruction::Vse {
            vs3: VReg::V0,
            rs1: Reg::A0,
            vm: true,
            eew: Eew::E32,
            rs2: Reg::Zero,
        },
    );

    assert!(matches!(result, Err(ExecutionError::MemoryAccess(_))));
}

#[test]
fn vsse_out_of_bounds_write_returns_memory_access_error() {
    let mut state = setup(2, Vsew::E64, Vlmul::M1);
    let vreg = [0u8; 16];
    set_vreg(&mut state, VReg::V0, &vreg);
    state.regs.write(Reg::A0, TEST_BASE_ADDR + 8192 - 8);
    state.regs.write(Reg::A1, 8);

    let result = exec_one(
        &mut state,
        Zve64xStoreInstruction::Vsse {
            vs3: VReg::V0,
            rs1: Reg::A0,
            rs2: Reg::A1,
            vm: true,
            eew: Eew::E64,
        },
    );

    assert!(matches!(result, Err(ExecutionError::MemoryAccess(_))));
}
