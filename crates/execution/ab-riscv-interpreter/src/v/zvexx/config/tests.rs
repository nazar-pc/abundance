use crate::rv64::test_utils::{ExtState, execute, initialize_state};
use crate::v::vector_registers::{VectorRegisters, VectorRegistersBase, VectorRegistersExt};
use crate::{Csrs, ExecutableInstructionCsr, RegisterFile};
use ab_riscv_primitives::prelude::*;

/// Encode a vtype immediate from SEW, LMUL, vta, vma fields
fn encode_vtype(vsew: Vsew, vlmul: Vlmul, vta: bool, vma: bool) -> u16 {
    let mut val = u16::from(vlmul.to_bits());
    val |= u16::from(vsew.to_bits()) << 3u8;
    if vta {
        val |= 1 << 6u8;
    }
    if vma {
        val |= 1 << 7u8;
    }
    val
}

// VLMAX for TEST_VLEN=256:
//   e8,m1  -> 256/8    = 32
//   e16,m1 -> 256/16   = 16
//   e32,m1 -> 256/32   = 8
//   e64,m1 -> 256/64   = 4
//   e8,m2  -> 512/8    = 64
//   e8,m8  -> 2048/8   = 256
//   e32,mf2-> 128/32   = 4
//   e8,mf8 -> 16/8    = 2

// vsetvli basic tests

#[test]
fn vsetvli_sets_vl_and_rd_from_avl() {
    let vtypei = encode_vtype(Vsew::E32, Vlmul::M1, false, false);
    // VLMAX = 256/32 = 8, AVL = 3 < VLMAX -> vl = 3
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetvli {
        rd: Reg::A0,
        rs1: Reg::A1,
        vtypei,
        rs2: Reg::Zero,
    }]);
    state.ext_state.init_vector_csrs();
    state.regs.write(Reg::A1, 3);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A0), 3);
    assert_eq!(state.ext_state.vl(), 3);
    assert!(state.ext_state.vtype().is_some());
    let vtype = state.ext_state.vtype().unwrap();
    assert_eq!(vtype.vsew(), Vsew::E32);
    assert_eq!(vtype.vlmul(), Vlmul::M1);
}

#[test]
fn vsetvli_avl_exceeds_vlmax_caps_to_vlmax() {
    let vtypei = encode_vtype(Vsew::E32, Vlmul::M1, false, false);
    // VLMAX = 8, AVL = 100 -> vl = 8
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetvli {
        rd: Reg::A0,
        rs1: Reg::A1,
        vtypei,
        rs2: Reg::Zero,
    }]);
    state.ext_state.init_vector_csrs();
    state.regs.write(Reg::A1, 100);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A0), 8);
    assert_eq!(state.ext_state.vl(), 8);
}

#[test]
fn vsetvli_avl_zero_gives_vl_zero() {
    let vtypei = encode_vtype(Vsew::E32, Vlmul::M1, false, false);
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetvli {
        rd: Reg::A0,
        rs1: Reg::A1,
        vtypei,
        rs2: Reg::Zero,
    }]);
    state.ext_state.init_vector_csrs();
    state.regs.write(Reg::A1, 0);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A0), 0);
    assert_eq!(state.ext_state.vl(), 0);
    assert!(state.ext_state.vtype().is_some());
}

#[test]
fn vsetvli_avl_equals_vlmax() {
    let vtypei = encode_vtype(Vsew::E8, Vlmul::M1, false, false);
    // VLMAX = 256/8 = 32
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetvli {
        rd: Reg::A0,
        rs1: Reg::A1,
        vtypei,
        rs2: Reg::Zero,
    }]);
    state.ext_state.init_vector_csrs();
    state.regs.write(Reg::A1, 32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A0), 32);
    assert_eq!(state.ext_state.vl(), 32);
}

#[test]
fn vsetvli_rd_x0_discards_result() {
    let vtypei = encode_vtype(Vsew::E32, Vlmul::M1, false, false);
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetvli {
        rd: Reg::Zero,
        rs1: Reg::A1,
        vtypei,
        rs2: Reg::Zero,
    }]);
    state.ext_state.init_vector_csrs();
    state.regs.write(Reg::A1, 3);

    execute(&mut state).unwrap();

    // x0 always reads as 0
    assert_eq!(state.regs.read(Reg::Zero), 0);
    // vl still set correctly
    assert_eq!(state.ext_state.vl(), 3);
}

// vsetvli SEW/LMUL combination tests

#[test]
fn vsetvli_e8_m8_gives_max_vlmax() {
    let vtypei = encode_vtype(Vsew::E8, Vlmul::M8, false, false);
    // VLMAX = (256*8)/8 = 256
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetvli {
        rd: Reg::A0,
        rs1: Reg::A1,
        vtypei,
        rs2: Reg::Zero,
    }]);
    state.ext_state.init_vector_csrs();
    state.regs.write(Reg::A1, 300);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A0), 256);
    assert_eq!(state.ext_state.vl(), 256);
}

#[test]
fn vsetvli_e64_m1() {
    let vtypei = encode_vtype(Vsew::E64, Vlmul::M1, false, false);
    // VLMAX = 256/64 = 4
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetvli {
        rd: Reg::A0,
        rs1: Reg::A1,
        vtypei,
        rs2: Reg::Zero,
    }]);
    state.ext_state.init_vector_csrs();
    state.regs.write(Reg::A1, 1);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A0), 1);
    assert_eq!(state.ext_state.vl(), 1);
    let vtype = state.ext_state.vtype().unwrap();
    assert_eq!(vtype.vsew(), Vsew::E64);
}

#[test]
fn vsetvli_e32_mf2() {
    let vtypei = encode_vtype(Vsew::E32, Vlmul::Mf2, false, false);
    // VLMAX = 256 / (32*2) = 4
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetvli {
        rd: Reg::A0,
        rs1: Reg::A1,
        vtypei,
        rs2: Reg::Zero,
    }]);
    state.ext_state.init_vector_csrs();
    state.regs.write(Reg::A1, 10);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A0), 4);
    assert_eq!(state.ext_state.vl(), 4);
}

#[test]
fn vsetvli_e8_mf8() {
    let vtypei = encode_vtype(Vsew::E8, Vlmul::Mf8, false, false);
    // VLMAX = 256 / (8*8) = 4
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetvli {
        rd: Reg::A0,
        rs1: Reg::A1,
        vtypei,
        rs2: Reg::Zero,
    }]);
    state.ext_state.init_vector_csrs();
    state.regs.write(Reg::A1, 1);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A0), 1);
    assert_eq!(state.ext_state.vl(), 1);
}

// vsetvli with vta/vma flags

#[test]
fn vsetvli_ta_ma_flags_preserved() {
    let vtypei = encode_vtype(Vsew::E16, Vlmul::M2, true, true);
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetvli {
        rd: Reg::A0,
        rs1: Reg::A1,
        vtypei,
        rs2: Reg::Zero,
    }]);
    state.ext_state.init_vector_csrs();
    state.regs.write(Reg::A1, 1);

    execute(&mut state).unwrap();

    let vtype = state.ext_state.vtype().unwrap();
    assert!(vtype.vta());
    assert!(vtype.vma());
    assert_eq!(vtype.vsew(), Vsew::E16);
    assert_eq!(vtype.vlmul(), Vlmul::M2);
}

#[test]
fn vsetvli_tu_mu_flags_preserved() {
    let vtypei = encode_vtype(Vsew::E16, Vlmul::M1, false, false);
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetvli {
        rd: Reg::A0,
        rs1: Reg::A1,
        vtypei,
        rs2: Reg::Zero,
    }]);
    state.ext_state.init_vector_csrs();
    state.regs.write(Reg::A1, 1);

    execute(&mut state).unwrap();

    let vtype = state.ext_state.vtype().unwrap();
    assert!(!vtype.vta());
    assert!(!vtype.vma());
}

// vsetvli unsupported configurations

#[test]
fn vsetvli_unsupported_sew_sets_vill() {
    // vsew = 0b100 is reserved, encode manually
    let vtypei = 0b100 << 3u8;
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetvli {
        rd: Reg::A0,
        rs1: Reg::A1,
        vtypei,
        rs2: Reg::Zero,
    }]);
    state.ext_state.init_vector_csrs();
    state.regs.write(Reg::A1, 10);

    execute(&mut state).unwrap();

    assert!(state.ext_state.vtype().is_none());
    assert_eq!(state.ext_state.vl(), 0);
    assert_eq!(state.regs.read(Reg::A0), 0);
}

#[test]
fn vsetvli_reserved_vlmul_sets_vill() {
    // vlmul = 0b100 is reserved
    let vtypei = 0b100;
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetvli {
        rd: Reg::A0,
        rs1: Reg::A1,
        vtypei,
        rs2: Reg::Zero,
    }]);
    state.ext_state.init_vector_csrs();
    state.regs.write(Reg::A1, 10);

    execute(&mut state).unwrap();

    assert!(state.ext_state.vtype().is_none());
    assert_eq!(state.ext_state.vl(), 0);
    assert_eq!(state.regs.read(Reg::A0), 0);
}

#[test]
fn vsetvli_vlmax_zero_sets_vill() {
    // e64 with mf8: VLMAX = 256/(64*8) = 0 -> unsupported
    let vtypei = encode_vtype(Vsew::E64, Vlmul::Mf8, false, false);
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetvli {
        rd: Reg::A0,
        rs1: Reg::A1,
        vtypei,
        rs2: Reg::Zero,
    }]);
    state.ext_state.init_vector_csrs();
    state.regs.write(Reg::A1, 1);

    execute(&mut state).unwrap();

    assert!(state.ext_state.vtype().is_none());
    assert_eq!(state.ext_state.vl(), 0);
    assert_eq!(state.regs.read(Reg::A0), 0);
}

#[test]
fn vsetvli_reserved_upper_bits_set_vill() {
    // Bit 8 set in vtypei -> reserved, must set vill
    let vtypei = encode_vtype(Vsew::E32, Vlmul::M1, false, false) | (1 << 8u8);
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetvli {
        rd: Reg::A0,
        rs1: Reg::A1,
        vtypei,
        rs2: Reg::Zero,
    }]);
    state.ext_state.init_vector_csrs();
    state.regs.write(Reg::A1, 1);

    execute(&mut state).unwrap();

    assert!(state.ext_state.vtype().is_none());
    assert_eq!(state.ext_state.vl(), 0);
}

// vsetvli rs1=x0 special cases

#[test]
fn vsetvli_rs1_x0_rd_nonzero_sets_vlmax() {
    let vtypei = encode_vtype(Vsew::E32, Vlmul::M1, false, false);
    // VLMAX = 256/32 = 8
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetvli {
        rd: Reg::A0,
        rs1: Reg::Zero,
        vtypei,
        rs2: Reg::Zero,
    }]);
    state.ext_state.init_vector_csrs();

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A0), 8);
    assert_eq!(state.ext_state.vl(), 8);
}

#[test]
fn vsetvli_rs1_x0_rd_nonzero_e8_m8_gives_full_vlmax() {
    let vtypei = encode_vtype(Vsew::E8, Vlmul::M8, false, false);
    // VLMAX = (256*8)/8 = 256
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetvli {
        rd: Reg::A0,
        rs1: Reg::Zero,
        vtypei,
        rs2: Reg::Zero,
    }]);
    state.ext_state.init_vector_csrs();

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A0), 256);
    assert_eq!(state.ext_state.vl(), 256);
}

#[test]
fn vsetvli_rs1_x0_rd_x0_keeps_vl_when_vlmax_unchanged() {
    // First: set e32,m1 with AVL=3 -> vl=3, VLMAX=4
    let vtypei_1 = encode_vtype(Vsew::E32, Vlmul::M1, false, false);
    let mut state = initialize_state([
        ZveXxConfigInstruction::Vsetvli {
            rd: Reg::A0,
            rs1: Reg::A1,
            vtypei: vtypei_1,
            rs2: Reg::Zero,
        },
        // Then: vsetvli x0, x0, e32,m1,ta,ma -> same VLMAX, keep vl=3
        ZveXxConfigInstruction::Vsetvli {
            rd: Reg::Zero,
            rs1: Reg::Zero,
            vtypei: encode_vtype(Vsew::E32, Vlmul::M1, true, true),
            rs2: Reg::Zero,
        },
    ]);
    state.ext_state.init_vector_csrs();
    state.regs.write(Reg::A1, 3);

    execute(&mut state).unwrap();

    assert_eq!(state.ext_state.vl(), 3);
    let vtype = state.ext_state.vtype().unwrap();
    assert!(vtype.vta());
    assert!(vtype.vma());
    assert_eq!(vtype.vsew(), Vsew::E32);
    assert_eq!(vtype.vlmul(), Vlmul::M1);
}

#[test]
fn vsetvli_rs1_x0_rd_x0_vill_when_vlmax_changes() {
    // First: set e32,m1 -> VLMAX = 4
    let mut state = initialize_state([
        ZveXxConfigInstruction::Vsetvli {
            rd: Reg::A0,
            rs1: Reg::A1,
            vtypei: encode_vtype(Vsew::E32, Vlmul::M1, false, false),
            rs2: Reg::Zero,
        },
        // Then: vsetvli x0, x0, e8,m1 -> VLMAX would be 16 != 4 -> vill
        ZveXxConfigInstruction::Vsetvli {
            rd: Reg::Zero,
            rs1: Reg::Zero,
            vtypei: encode_vtype(Vsew::E8, Vlmul::M1, false, false),
            rs2: Reg::Zero,
        },
    ]);
    state.ext_state.init_vector_csrs();
    state.regs.write(Reg::A1, 3);

    execute(&mut state).unwrap();

    assert!(state.ext_state.vtype().is_none());
    assert_eq!(state.ext_state.vl(), 0);
}

// vsetivli tests

#[test]
fn vsetivli_basic() {
    let vtypei = encode_vtype(Vsew::E32, Vlmul::M1, false, false);
    // VLMAX = 4, AVL = 3 (from immediate)
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetivli {
        rd: Reg::A0,
        uimm: 3,
        vtypei,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.ext_state.init_vector_csrs();

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A0), 3);
    assert_eq!(state.ext_state.vl(), 3);
    assert!(state.ext_state.vtype().is_some());
}

#[test]
fn vsetivli_avl_zero() {
    let vtypei = encode_vtype(Vsew::E32, Vlmul::M1, false, false);
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetivli {
        rd: Reg::A0,
        uimm: 0,
        vtypei,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.ext_state.init_vector_csrs();

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A0), 0);
    assert_eq!(state.ext_state.vl(), 0);
    assert!(state.ext_state.vtype().is_some());
}

#[test]
fn vsetivli_max_immediate() {
    let vtypei = encode_vtype(Vsew::E32, Vlmul::M1, false, false);
    // VLMAX = 8, uimm = 31 > VLMAX -> vl = 8
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetivli {
        rd: Reg::A0,
        uimm: 31,
        vtypei,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.ext_state.init_vector_csrs();

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A0), 8);
    assert_eq!(state.ext_state.vl(), 8);
}

#[test]
fn vsetivli_avl_within_vlmax() {
    let vtypei = encode_vtype(Vsew::E8, Vlmul::M8, false, false);
    // VLMAX = 256, uimm = 20 -> vl = 20
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetivli {
        rd: Reg::A0,
        uimm: 20,
        vtypei,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.ext_state.init_vector_csrs();

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A0), 20);
    assert_eq!(state.ext_state.vl(), 20);
}

#[test]
fn vsetivli_unsupported_sets_vill() {
    // Reserved vlmul encoding
    let vtypei = 0b100;
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetivli {
        rd: Reg::A0,
        uimm: 5,
        vtypei,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.ext_state.init_vector_csrs();

    execute(&mut state).unwrap();

    assert!(state.ext_state.vtype().is_none());
    assert_eq!(state.ext_state.vl(), 0);
    assert_eq!(state.regs.read(Reg::A0), 0);
}

#[test]
fn vsetivli_with_ta_ma() {
    let vtypei = encode_vtype(Vsew::E16, Vlmul::M4, true, true);
    // VLMAX = (256*4)/16 = 64, uimm = 10
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetivli {
        rd: Reg::A0,
        uimm: 10,
        vtypei,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.ext_state.init_vector_csrs();

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A0), 10);
    let vtype = state.ext_state.vtype().unwrap();
    assert!(vtype.vta());
    assert!(vtype.vma());
}

// vsetvl tests

#[test]
fn vsetvl_basic() {
    let vtype_raw = u64::from(encode_vtype(Vsew::E32, Vlmul::M1, false, false));
    // VLMAX = 4, AVL = 3
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetvl {
        rd: Reg::A0,
        rs1: Reg::A1,
        rs2: Reg::A2,
    }]);
    state.ext_state.init_vector_csrs();
    state.regs.write(Reg::A1, 3);
    state.regs.write(Reg::A2, vtype_raw);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A0), 3);
    assert_eq!(state.ext_state.vl(), 3);
    assert!(state.ext_state.vtype().is_some());
    let vtype = state.ext_state.vtype().unwrap();
    assert_eq!(vtype.vsew(), Vsew::E32);
    assert_eq!(vtype.vlmul(), Vlmul::M1);
}

#[test]
fn vsetvl_rs1_x0_rd_nonzero() {
    let vtype_raw = u64::from(encode_vtype(Vsew::E64, Vlmul::M1, false, false));
    // VLMAX = 256/64 = 4
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetvl {
        rd: Reg::A0,
        rs1: Reg::Zero,
        rs2: Reg::A2,
    }]);
    state.ext_state.init_vector_csrs();
    state.regs.write(Reg::A2, vtype_raw);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A0), 4);
    assert_eq!(state.ext_state.vl(), 4);
}

#[test]
fn vsetvl_unsupported_raw_sets_vill() {
    // Set bit `XLEN-1` (vill) in the register value
    let vtype_raw = 1u64 << (u64::BITS - 1);
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetvl {
        rd: Reg::A0,
        rs1: Reg::A1,
        rs2: Reg::A2,
    }]);
    state.ext_state.init_vector_csrs();
    state.regs.write(Reg::A1, 10);
    state.regs.write(Reg::A2, vtype_raw);

    execute(&mut state).unwrap();

    assert!(state.ext_state.vtype().is_none());
    assert_eq!(state.ext_state.vl(), 0);
    assert_eq!(state.regs.read(Reg::A0), 0);
}

#[test]
fn vsetvl_high_bits_in_rs2_sets_vill() {
    // Upper bits [62:8] non-zero -> must set vill per spec
    let vtype_raw = (1u64 << 10u8) | u64::from(encode_vtype(Vsew::E32, Vlmul::M1, false, false));
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetvl {
        rd: Reg::A0,
        rs1: Reg::A1,
        rs2: Reg::A2,
    }]);
    state.ext_state.init_vector_csrs();
    state.regs.write(Reg::A1, 1);
    state.regs.write(Reg::A2, vtype_raw);

    execute(&mut state).unwrap();

    assert!(state.ext_state.vtype().is_none());
    assert_eq!(state.ext_state.vl(), 0);
}

#[test]
fn vsetvl_context_restore_preserves_vtype() {
    // vsetvl is used for context restore; ensure the full round-trip works
    let vtype_raw = u64::from(encode_vtype(Vsew::E16, Vlmul::M4, true, false));
    // VLMAX = (256*4)/16 = 64
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetvl {
        rd: Reg::A0,
        rs1: Reg::A1,
        rs2: Reg::A2,
    }]);
    state.ext_state.init_vector_csrs();
    state.regs.write(Reg::A1, 25);
    state.regs.write(Reg::A2, vtype_raw);

    execute(&mut state).unwrap();

    let vtype = state.ext_state.vtype().unwrap();
    assert_eq!(vtype.vsew(), Vsew::E16);
    assert_eq!(vtype.vlmul(), Vlmul::M4);
    assert!(vtype.vta());
    assert!(!vtype.vma());
    assert_eq!(state.ext_state.vl(), 25);
}

// mark_vs_dirty tracking

#[test]
fn vsetvli_marks_dirty() {
    let vtypei = encode_vtype(Vsew::E32, Vlmul::M1, false, false);
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetvli {
        rd: Reg::A0,
        rs1: Reg::A1,
        vtypei,
        rs2: Reg::Zero,
    }]);
    state.ext_state.init_vector_csrs();
    state.regs.write(Reg::A1, 1);

    execute(&mut state).unwrap();

    assert!(state.ext_state.vs_dirty_count() > 0);
}

#[test]
fn vsetvli_unsupported_still_marks_dirty() {
    // Even when setting vill, the vector state changed -> dirty
    let vtypei = 0b100;
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetivli {
        rd: Reg::A0,
        uimm: 1,
        vtypei,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.ext_state.init_vector_csrs();

    execute(&mut state).unwrap();

    assert!(state.ext_state.vs_dirty_count() > 0);
}

// vector_instructions_allowed check

#[test]
fn vsetvli_fails_when_vector_disabled() {
    let vtypei = encode_vtype(Vsew::E32, Vlmul::M1, false, false);
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetvli {
        rd: Reg::A0,
        rs1: Reg::A1,
        vtypei,
        rs2: Reg::Zero,
    }]);
    state.ext_state.init_vector_csrs();
    state.regs.write(Reg::A1, 1);
    state.ext_state.set_vector_allowed(false);

    let result = execute(&mut state);
    assert!(result.is_err());
}

#[test]
fn vsetivli_fails_when_vector_disabled() {
    let vtypei = encode_vtype(Vsew::E32, Vlmul::M1, false, false);
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetivli {
        rd: Reg::A0,
        uimm: 5,
        vtypei,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.ext_state.init_vector_csrs();
    state.ext_state.set_vector_allowed(false);

    let result = execute(&mut state);
    assert!(result.is_err());
}

#[test]
fn vsetvl_fails_when_vector_disabled() {
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetvl {
        rd: Reg::A0,
        rs1: Reg::A1,
        rs2: Reg::A2,
    }]);
    state.ext_state.init_vector_csrs();
    state.regs.write(Reg::A1, 1);
    state.regs.write(
        Reg::A2,
        u64::from(encode_vtype(Vsew::E32, Vlmul::M1, false, false)),
    );
    state.ext_state.set_vector_allowed(false);

    let result = execute(&mut state);
    assert!(result.is_err());
}

// CSR read/write via prepare_csr_read/prepare_csr_write

#[test]
fn prepare_csr_read_passes_through_vector_csrs() {
    let mut output = 0u64;
    let mut state = initialize_state::<ZveXxConfigInstruction<_>, _>([]);
    state.ext_state.init_vector_csrs();

    let result = <ZveXxConfigInstruction<_>>::prepare_csr_read(
        &state.ext_state,
        VCsr::Vstart as u16,
        42,
        &mut output,
    );
    assert!(result.unwrap());
    assert_eq!(output, 42);
}

#[test]
fn prepare_csr_read_ignores_non_vector_csrs() {
    let mut output = 0u64;
    let mut state = initialize_state::<ZveXxConfigInstruction<_>, _>([]);
    state.ext_state.init_vector_csrs();

    let result =
        <ZveXxConfigInstruction<_>>::prepare_csr_read(&state.ext_state, 0x300, 42, &mut output);
    // Returns Ok(false) meaning "not handled by this extension"
    assert!(!result.unwrap());
}

#[test]
fn prepare_csr_read_works_for_all_vector_csrs() {
    let mut state = initialize_state::<ZveXxConfigInstruction<_>, _>([]);
    state.ext_state.init_vector_csrs();
    let csr_indices = [
        VCsr::Vstart as u16,
        VCsr::Vxsat as u16,
        VCsr::Vxrm as u16,
        VCsr::Vcsr as u16,
        VCsr::Vl as u16,
        VCsr::Vtype as u16,
        VCsr::Vlenb as u16,
    ];

    for csr_index in csr_indices {
        let mut output = 0u64;
        let result = <ZveXxConfigInstruction<_>>::prepare_csr_read(
            &state.ext_state,
            csr_index,
            0xFF,
            &mut output,
        );
        assert!(result.unwrap(), "CSR {csr_index:#x} should be handled");
        assert_eq!(output, 0xFF, "CSR {csr_index:#x} should pass through");
    }
}

#[test]
fn prepare_csr_write_rejects_read_only_vl() {
    let mut output = 0u64;
    let mut state = initialize_state::<ZveXxConfigInstruction<_>, _>([]);
    state.ext_state.init_vector_csrs();

    let result = <ZveXxConfigInstruction<_>>::prepare_csr_write(
        &mut state.ext_state,
        VCsr::Vl as u16,
        42,
        &mut output,
    );
    assert!(result.is_err());
}

#[test]
fn prepare_csr_write_rejects_read_only_vtype() {
    let mut output = 0u64;
    let mut state = initialize_state::<ZveXxConfigInstruction<_>, _>([]);
    state.ext_state.init_vector_csrs();

    let result = <ZveXxConfigInstruction<_>>::prepare_csr_write(
        &mut state.ext_state,
        VCsr::Vtype as u16,
        42,
        &mut output,
    );
    assert!(result.is_err());
}

#[test]
fn prepare_csr_write_rejects_read_only_vlenb() {
    let mut output = 0u64;
    let mut state = initialize_state::<ZveXxConfigInstruction<_>, _>([]);
    state.ext_state.init_vector_csrs();

    let result = <ZveXxConfigInstruction<_>>::prepare_csr_write(
        &mut state.ext_state,
        VCsr::Vlenb as u16,
        42,
        &mut output,
    );
    assert!(result.is_err());
}

#[test]
fn prepare_csr_write_vxsat_masks_to_1_bit() {
    let mut output = 0u64;
    let mut state = initialize_state::<ZveXxConfigInstruction<_>, _>([]);
    state.ext_state.init_vector_csrs();

    let result = <ZveXxConfigInstruction<_>>::prepare_csr_write(
        &mut state.ext_state,
        VCsr::Vxsat as u16,
        0xFF,
        &mut output,
    );
    assert!(result.unwrap());
    assert_eq!(output, 1);
}

#[test]
fn prepare_csr_write_vxrm_masks_to_2_bits() {
    let mut output = 0u64;
    let mut state = initialize_state::<ZveXxConfigInstruction<_>, _>([]);
    state.ext_state.init_vector_csrs();

    let result = <ZveXxConfigInstruction<_>>::prepare_csr_write(
        &mut state.ext_state,
        VCsr::Vxrm as u16,
        0xFF,
        &mut output,
    );
    assert!(result.unwrap());
    assert_eq!(output, 0b11);
}

#[test]
fn prepare_csr_write_vcsr_masks_to_3_bits() {
    let mut output = 0u64;
    let mut state = initialize_state::<ZveXxConfigInstruction<_>, _>([]);
    state.ext_state.init_vector_csrs();

    let result = <ZveXxConfigInstruction<_>>::prepare_csr_write(
        &mut state.ext_state,
        VCsr::Vcsr as u16,
        0xFFFF,
        &mut output,
    );
    assert!(result.unwrap());
    assert_eq!(output, 0b111);
}

#[test]
fn prepare_csr_write_vstart_passes_full_value() {
    let mut output = 0u64;
    let mut state = initialize_state::<ZveXxConfigInstruction<_>, _>([]);
    state.ext_state.init_vector_csrs();

    let result = <ZveXxConfigInstruction<_>>::prepare_csr_write(
        &mut state.ext_state,
        VCsr::Vstart as u16,
        0x1234,
        &mut output,
    );
    assert!(result.unwrap());
    assert_eq!(output, 0x1234);
}

#[test]
fn prepare_csr_write_ignores_non_vector_csrs() {
    let mut output = 0u64;
    let mut state = initialize_state::<ZveXxConfigInstruction<_>, _>([]);
    state.ext_state.init_vector_csrs();

    let result = <ZveXxConfigInstruction<_>>::prepare_csr_write(
        &mut state.ext_state,
        0x300,
        42,
        &mut output,
    );
    assert!(!result.unwrap());
}

// vtype CSR raw value tracking

#[test]
fn vtype_csr_raw_value_matches_decoded() {
    let vtypei = encode_vtype(Vsew::E16, Vlmul::M2, true, false);
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetvli {
        rd: Reg::A0,
        rs1: Reg::A1,
        vtypei,
        rs2: Reg::Zero,
    }]);
    state.ext_state.init_vector_csrs();
    state.regs.write(Reg::A1, 1);

    execute(&mut state).unwrap();

    let raw = state.ext_state.read_csr(VCsr::Vtype as u16).unwrap();
    // Should match the encoded vtypei (low 8 bits)
    assert_eq!(raw, u64::from(vtypei));
}

#[test]
fn vtype_csr_vill_sets_bit_63() {
    let vtypei = 0b100;
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetvli {
        rd: Reg::A0,
        rs1: Reg::A1,
        vtypei,
        rs2: Reg::Zero,
    }]);
    state.ext_state.init_vector_csrs();
    state.regs.write(Reg::A1, 1);

    execute(&mut state).unwrap();

    let raw = state.ext_state.read_csr(VCsr::Vtype as u16).unwrap();
    assert_eq!(raw, 1u64 << (u64::BITS - 1));
}

#[test]
fn vl_csr_matches_vl_value() {
    let vtypei = encode_vtype(Vsew::E32, Vlmul::M1, false, false);
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetvli {
        rd: Reg::A0,
        rs1: Reg::A1,
        vtypei,
        rs2: Reg::Zero,
    }]);
    state.ext_state.init_vector_csrs();
    state.regs.write(Reg::A1, 3);

    execute(&mut state).unwrap();

    let raw = state.ext_state.read_csr(VCsr::Vl as u16).unwrap();
    assert_eq!(raw, 3);
}

#[test]
fn vlenb_csr_returns_correct_value() {
    let mut state = initialize_state::<ZveXxConfigInstruction<_>, _>([]);
    state.ext_state.init_vector_csrs();
    let raw = state.ext_state.read_csr(VCsr::Vlenb as u16).unwrap();
    assert_eq!(raw, u64::from(ExtState::VLENB));
}

// Sequential instruction tests

#[test]
fn sequential_vsetvli_overrides_previous() {
    let mut state = initialize_state([
        ZveXxConfigInstruction::Vsetvli {
            rd: Reg::A0,
            rs1: Reg::A1,
            vtypei: encode_vtype(Vsew::E32, Vlmul::M1, false, false),
            rs2: Reg::Zero,
        },
        ZveXxConfigInstruction::Vsetvli {
            rd: Reg::A2,
            rs1: Reg::A3,
            vtypei: encode_vtype(Vsew::E8, Vlmul::M2, true, true),
            rs2: Reg::Zero,
        },
    ]);
    state.ext_state.init_vector_csrs();
    state.regs.write(Reg::A1, 3);
    state.regs.write(Reg::A3, 10);

    execute(&mut state).unwrap();

    // Second instruction should have taken effect
    // VLMAX = (256*2)/8 = 64, AVL = 10 -> vl = 10
    assert_eq!(state.ext_state.vl(), 10);
    assert_eq!(state.regs.read(Reg::A2), 10);
    let vtype = state.ext_state.vtype().unwrap();
    assert_eq!(vtype.vsew(), Vsew::E8);
    assert_eq!(vtype.vlmul(), Vlmul::M2);
    assert!(vtype.vta());
    assert!(vtype.vma());
}

#[test]
fn vsetvli_after_vill_recovers() {
    let mut state = initialize_state([
        // First: unsupported -> vill
        ZveXxConfigInstruction::Vsetvli {
            rd: Reg::A0,
            rs1: Reg::A1,
            vtypei: 0b100,
            rs2: Reg::Zero,
        },
        // Second: valid config should recover
        ZveXxConfigInstruction::Vsetvli {
            rd: Reg::A2,
            rs1: Reg::A3,
            vtypei: encode_vtype(Vsew::E32, Vlmul::M1, false, false),
            rs2: Reg::Zero,
        },
    ]);
    state.ext_state.init_vector_csrs();
    state.regs.write(Reg::A1, 1);
    state.regs.write(Reg::A3, 2);

    execute(&mut state).unwrap();

    assert!(state.ext_state.vtype().is_some());
    assert_eq!(state.ext_state.vl(), 2);
    assert_eq!(state.regs.read(Reg::A2), 2);
}

// Mixed instruction type tests

#[test]
fn vsetivli_followed_by_vsetvl_x0_x0() {
    let mut state = initialize_state([
        // Set e16,m1 with AVL=5 -> vl=5, VLMAX=16
        ZveXxConfigInstruction::Vsetivli {
            rd: Reg::A0,
            uimm: 5,
            vtypei: encode_vtype(Vsew::E16, Vlmul::M1, false, false),
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
        // Change to ta,ma but keep same SEW/LMUL (same VLMAX=16)
        ZveXxConfigInstruction::Vsetvli {
            rd: Reg::Zero,
            rs1: Reg::Zero,
            vtypei: encode_vtype(Vsew::E16, Vlmul::M1, true, true),
            rs2: Reg::Zero,
        },
    ]);
    state.ext_state.init_vector_csrs();

    execute(&mut state).unwrap();

    // vl should remain 5
    assert_eq!(state.ext_state.vl(), 5);
    let vtype = state.ext_state.vtype().unwrap();
    assert!(vtype.vta());
    assert!(vtype.vma());
    assert_eq!(vtype.vsew(), Vsew::E16);
}

// Edge cases

#[test]
fn vsetvli_large_avl_in_register() {
    let vtypei = encode_vtype(Vsew::E32, Vlmul::M1, false, false);
    // VLMAX = 8, AVL = u64::MAX -> vl = 8
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetvli {
        rd: Reg::A0,
        rs1: Reg::A1,
        vtypei,
        rs2: Reg::Zero,
    }]);
    state.ext_state.init_vector_csrs();
    state.regs.write(Reg::A1, u64::MAX);

    execute(&mut state).unwrap();

    assert_eq!(state.ext_state.vl(), 8);
    assert_eq!(state.regs.read(Reg::A0), 8);
}

#[test]
fn vsetvl_all_bits_set_in_rs2_sets_vill() {
    let mut state = initialize_state([ZveXxConfigInstruction::Vsetvl {
        rd: Reg::A0,
        rs1: Reg::A1,
        rs2: Reg::A2,
    }]);
    state.ext_state.init_vector_csrs();
    state.regs.write(Reg::A1, 1);
    state.regs.write(Reg::A2, u64::MAX);

    execute(&mut state).unwrap();

    // All bits set means upper bits non-zero -> vill
    assert!(state.ext_state.vtype().is_none());
    assert_eq!(state.ext_state.vl(), 0);
}

// Vlmul::vlmax unit tests

#[test]
fn vlmul_vlmax_m1_e32_vlen128() {
    assert_eq!(Vlmul::M1.vlmax(128, 32), 4);
}

#[test]
fn vlmul_vlmax_m2_e32_vlen128() {
    assert_eq!(Vlmul::M2.vlmax(128, 32), 8);
}

#[test]
fn vlmul_vlmax_m4_e32_vlen128() {
    assert_eq!(Vlmul::M4.vlmax(128, 32), 16);
}

#[test]
fn vlmul_vlmax_m8_e8_vlen128() {
    assert_eq!(Vlmul::M8.vlmax(128, 8), 128);
}

#[test]
fn vlmul_vlmax_mf2_e32_vlen128() {
    assert_eq!(Vlmul::Mf2.vlmax(128, 32), 2);
}

#[test]
fn vlmul_vlmax_mf4_e16_vlen128() {
    // 128 / (16*4) = 2
    assert_eq!(Vlmul::Mf4.vlmax(128, 16), 2);
}

#[test]
fn vlmul_vlmax_mf8_e8_vlen128() {
    // 128 / (8*8) = 2
    assert_eq!(Vlmul::Mf8.vlmax(128, 8), 2);
}

#[test]
fn vlmul_vlmax_zero_when_too_small() {
    // e64 with mf8 on VLEN=128: 128/(64*8) = 0
    assert_eq!(Vlmul::Mf8.vlmax(128, 64), 0);
}

// Vtype decode/encode round-trip tests

#[test]
fn vtype_encode_decode_roundtrip() {
    let combos: &[(Vsew, Vlmul, bool, bool)] = &[
        (Vsew::E8, Vlmul::M1, false, false),
        (Vsew::E16, Vlmul::M2, true, false),
        (Vsew::E32, Vlmul::M4, false, true),
        (Vsew::E64, Vlmul::M8, true, true),
        (Vsew::E8, Vlmul::Mf2, false, false),
        (Vsew::E16, Vlmul::Mf4, true, true),
        (Vsew::E8, Vlmul::Mf8, false, true),
    ];

    for &(vsew, vlmul, vta, vma) in combos {
        let raw = u64::from(encode_vtype(vsew, vlmul, vta, vma));
        let decoded = Vtype::<{ ExtState::ELEN }, { ExtState::VLEN }>::from_raw::<Reg<u64>>(raw);
        assert!(
            decoded.is_some(),
            "Failed to decode vsew={vsew}, vlmul={vlmul}"
        );
        let decoded = decoded.unwrap();
        assert_eq!(decoded.vsew(), vsew);
        assert_eq!(decoded.vlmul(), vlmul);
        assert_eq!(decoded.vta(), vta);
        assert_eq!(decoded.vma(), vma);

        // Re-encode
        let re_encoded = decoded.to_raw::<Reg<u64>>();
        assert_eq!(re_encoded, raw);
    }
}

#[test]
fn vtype_from_raw_rejects_reserved_vsew() {
    // vsew = 0b100 (bits [5:3] = 4)
    let raw = 0b100_000u64;
    let result = Vtype::<{ ExtState::ELEN }, { ExtState::VLEN }>::from_raw::<Reg<u64>>(raw);
    assert!(result.is_none());
}

#[test]
fn vtype_from_raw_rejects_reserved_vlmul() {
    // vlmul = 0b100
    let raw = 0b100u64;
    let result = Vtype::<{ ExtState::ELEN }, { ExtState::VLEN }>::from_raw::<Reg<u64>>(raw);
    assert!(result.is_none());
}

#[test]
fn vtype_from_raw_rejects_upper_bits_set() {
    let raw = (1u64 << 8u8) | u64::from(encode_vtype(Vsew::E32, Vlmul::M1, false, false));
    let result = Vtype::<{ ExtState::ELEN }, { ExtState::VLEN }>::from_raw::<Reg<u64>>(raw);
    assert!(result.is_none());
}

#[test]
fn vtype_from_raw_rejects_sew_exceeding_elen() {
    // For Zve32x (ELEN=32), e64 should be rejected.
    // But our ELEN=64, so e64 is fine. Test with a smaller ELEN.
    let raw = u64::from(encode_vtype(Vsew::E64, Vlmul::M1, false, false));
    let result = Vtype::<32, { ExtState::VLEN }>::from_raw::<Reg<u64>>(raw);
    assert!(result.is_none());
}

#[test]
fn vtype_from_raw_rejects_zero_vlmax() {
    // e64 mf8 on VLEN=128: VLMAX = 0
    let raw = u64::from(encode_vtype(Vsew::E64, Vlmul::Mf8, false, false));
    let result = Vtype::<{ ExtState::ELEN }, { ExtState::VLEN }>::from_raw::<Reg<u64>>(raw);
    assert!(result.is_none());
}

// VectorCsr enum tests

#[test]
fn vector_csr_from_index_all_valid() {
    assert_eq!(VCsr::from_index(0x008), Some(VCsr::Vstart));
    assert_eq!(VCsr::from_index(0x009), Some(VCsr::Vxsat));
    assert_eq!(VCsr::from_index(0x00A), Some(VCsr::Vxrm));
    assert_eq!(VCsr::from_index(0x00F), Some(VCsr::Vcsr));
    assert_eq!(VCsr::from_index(0xC20), Some(VCsr::Vl));
    assert_eq!(VCsr::from_index(0xC21), Some(VCsr::Vtype));
    assert_eq!(VCsr::from_index(0xC22), Some(VCsr::Vlenb));
}

#[test]
fn vector_csr_from_index_invalid() {
    assert_eq!(VCsr::from_index(0x000), None);
    assert_eq!(VCsr::from_index(0x300), None);
    assert_eq!(VCsr::from_index(0xFFF), None);
}

// VectorRegistersExt derived accessor tests

#[test]
fn ext_vstart_read_write() {
    let mut state = initialize_state::<ZveXxConfigInstruction<_>, _>([]);
    state.ext_state.init_vector_csrs();
    VectorRegistersExt::<Reg<u64>>::set_vstart(&mut state.ext_state, 42);
    assert_eq!(VectorRegistersExt::<Reg<u64>>::vstart(&state.ext_state), 42);
}

#[test]
fn ext_vxrm_read_write() {
    let mut state = initialize_state::<ZveXxConfigInstruction<_>, _>([]);
    state.ext_state.init_vector_csrs();
    VectorRegistersExt::<Reg<u64>>::set_vxrm(&mut state.ext_state, Vxrm::Rod);
    assert_eq!(
        VectorRegistersExt::<Reg<u64>>::vxrm(&state.ext_state),
        Vxrm::Rod
    );
}

#[test]
fn ext_vxsat_read_write() {
    let mut state = initialize_state::<ZveXxConfigInstruction<_>, _>([]);
    state.ext_state.init_vector_csrs();
    VectorRegistersExt::<Reg<u64>>::set_vxsat(&mut state.ext_state, true);
    assert!(VectorRegistersExt::<Reg<u64>>::vxsat(&state.ext_state));
}

#[test]
fn ext_initialize_vector_state() {
    let mut state = initialize_state::<ZveXxConfigInstruction<_>, _>([]);
    state.ext_state.init_vector_csrs();
    // Dirty it up
    state.ext_state.set_vl(42);
    VectorRegistersExt::<Reg<u64>>::set_vstart(&mut state.ext_state, 7);
    VectorRegistersExt::<Reg<u64>>::set_vxrm(&mut state.ext_state, Vxrm::Rne);
    VectorRegistersExt::<Reg<u64>>::set_vxsat(&mut state.ext_state, true);

    // Reset
    state.ext_state.init_vector_csrs();

    assert!(state.ext_state.vtype().is_none());
    assert_eq!(state.ext_state.vl(), 0);
    assert_eq!(VectorRegistersExt::<Reg<u64>>::vstart(&state.ext_state), 0);
    assert_eq!(
        VectorRegistersExt::<Reg<u64>>::vxrm(&state.ext_state),
        Vxrm::Rnu
    );
    assert!(!VectorRegistersExt::<Reg<u64>>::vxsat(&state.ext_state));
}

// vcsr mirroring tests

#[test]
fn prepare_csr_write_vxsat_mirrors_into_vcsr() {
    let mut state = initialize_state::<ZveXxConfigInstruction<_>, _>([]);
    state.ext_state.init_vector_csrs();
    // Pre-set vcsr to have vxrm=0b10 (bits [2:1]), vxsat=0 -> vcsr = 0b100
    state.ext_state.write_csr(VCsr::Vcsr as u16, 0b100).unwrap();

    let mut output = 0u64;
    let result = <ZveXxConfigInstruction<_>>::prepare_csr_write(
        &mut state.ext_state,
        VCsr::Vxsat as u16,
        1,
        &mut output,
    );
    assert!(result.unwrap());
    assert_eq!(output, 1);

    // vcsr should now be 0b101: vxrm=0b10 preserved, vxsat=1 mirrored
    let vcsr = state.ext_state.read_csr(VCsr::Vcsr as u16).unwrap();
    assert_eq!(vcsr, 0b101);
}

#[test]
fn prepare_csr_write_vxsat_clear_mirrors_into_vcsr() {
    let mut state = initialize_state::<ZveXxConfigInstruction<_>, _>([]);
    state.ext_state.init_vector_csrs();
    // Pre-set vcsr = 0b111 (vxrm=0b11, vxsat=1)
    state.ext_state.write_csr(VCsr::Vcsr as u16, 0b111).unwrap();

    let mut output = 0u64;
    <ZveXxConfigInstruction<_>>::prepare_csr_write(
        &mut state.ext_state,
        VCsr::Vxsat as u16,
        0,
        &mut output,
    )
    .unwrap();

    // vcsr should now be 0b110: vxrm=0b11 preserved, vxsat=0
    let vcsr = state.ext_state.read_csr(VCsr::Vcsr as u16).unwrap();
    assert_eq!(vcsr, 0b110);
}

#[test]
fn prepare_csr_write_vxrm_mirrors_into_vcsr() {
    let mut state = initialize_state::<ZveXxConfigInstruction<_>, _>([]);
    state.ext_state.init_vector_csrs();
    // Pre-set vcsr = 0b001 (vxrm=0b00, vxsat=1)
    state.ext_state.write_csr(VCsr::Vcsr as u16, 0b001).unwrap();

    let mut output = 0u64;
    <ZveXxConfigInstruction<_>>::prepare_csr_write(
        &mut state.ext_state,
        VCsr::Vxrm as u16,
        0b11,
        &mut output,
    )
    .unwrap();
    assert_eq!(output, 0b11);

    // vcsr should now be 0b111: vxrm=0b11 mirrored, vxsat=1 preserved
    let vcsr = state.ext_state.read_csr(VCsr::Vcsr as u16).unwrap();
    assert_eq!(vcsr, 0b111);
}

#[test]
fn prepare_csr_write_vxrm_clear_mirrors_into_vcsr() {
    let mut state = initialize_state::<ZveXxConfigInstruction<_>, _>([]);
    state.ext_state.init_vector_csrs();
    // Pre-set vcsr = 0b111
    state.ext_state.write_csr(VCsr::Vcsr as u16, 0b111).unwrap();

    let mut output = 0u64;
    <ZveXxConfigInstruction<_>>::prepare_csr_write(
        &mut state.ext_state,
        VCsr::Vxrm as u16,
        0b00,
        &mut output,
    )
    .unwrap();

    // vcsr should now be 0b001: vxrm=0b00, vxsat=1 preserved
    let vcsr = state.ext_state.read_csr(VCsr::Vcsr as u16).unwrap();
    assert_eq!(vcsr, 0b001);
}

#[test]
fn prepare_csr_write_vcsr_mirrors_into_vxsat_and_vxrm() {
    let mut state = initialize_state::<ZveXxConfigInstruction<_>, _>([]);
    state.ext_state.init_vector_csrs();
    // Start with vxsat=0, vxrm=0
    state.ext_state.write_csr(VCsr::Vxsat as u16, 0).unwrap();
    state.ext_state.write_csr(VCsr::Vxrm as u16, 0).unwrap();

    let mut output = 0u64;
    // Write vcsr = 0b101 (vxrm=0b10, vxsat=1)
    <ZveXxConfigInstruction<_>>::prepare_csr_write(
        &mut state.ext_state,
        VCsr::Vcsr as u16,
        0b101,
        &mut output,
    )
    .unwrap();
    assert_eq!(output, 0b101);

    let vxsat = state.ext_state.read_csr(VCsr::Vxsat as u16).unwrap();
    assert_eq!(vxsat, 1);

    let vxrm = state.ext_state.read_csr(VCsr::Vxrm as u16).unwrap();
    assert_eq!(vxrm, 0b10);
}

#[test]
fn prepare_csr_write_vcsr_zero_clears_vxsat_and_vxrm() {
    let mut state = initialize_state::<ZveXxConfigInstruction<_>, _>([]);
    state.ext_state.init_vector_csrs();
    // Pre-set non-zero values
    state.ext_state.write_csr(VCsr::Vxsat as u16, 1).unwrap();
    state.ext_state.write_csr(VCsr::Vxrm as u16, 0b11).unwrap();

    let mut output = 0u64;
    <ZveXxConfigInstruction<_>>::prepare_csr_write(
        &mut state.ext_state,
        VCsr::Vcsr as u16,
        0,
        &mut output,
    )
    .unwrap();

    let vxsat = state.ext_state.read_csr(VCsr::Vxsat as u16).unwrap();
    assert_eq!(vxsat, 0);

    let vxrm = state.ext_state.read_csr(VCsr::Vxrm as u16).unwrap();
    assert_eq!(vxrm, 0);
}

#[test]
fn prepare_csr_write_vcsr_masks_then_mirrors() {
    let mut state = initialize_state::<ZveXxConfigInstruction<_>, _>([]);
    state.ext_state.init_vector_csrs();

    let mut output = 0u64;
    // Write 0xFF to vcsr; should mask to 0b111, then mirror
    <ZveXxConfigInstruction<_>>::prepare_csr_write(
        &mut state.ext_state,
        VCsr::Vcsr as u16,
        0xFF,
        &mut output,
    )
    .unwrap();
    assert_eq!(output, 0b111);

    let vxsat = state.ext_state.read_csr(VCsr::Vxsat as u16).unwrap();
    assert_eq!(vxsat, 1);

    let vxrm = state.ext_state.read_csr(VCsr::Vxrm as u16).unwrap();
    assert_eq!(vxrm, 0b11);
}

#[test]
fn mirroring_roundtrip_vxsat_to_vcsr_and_back() {
    let mut state = initialize_state::<ZveXxConfigInstruction<_>, _>([]);
    state.ext_state.init_vector_csrs();

    let mut output = 0u64;

    // Write vxrm=0b10 via vcsr
    <ZveXxConfigInstruction<_>>::prepare_csr_write(
        &mut state.ext_state,
        VCsr::Vcsr as u16,
        0b100,
        &mut output,
    )
    .unwrap();
    // Now write the masked vcsr value to the CSR storage itself
    state
        .ext_state
        .write_csr(VCsr::Vcsr as u16, output)
        .unwrap();

    // Write vxsat=1 directly
    <ZveXxConfigInstruction<_>>::prepare_csr_write(
        &mut state.ext_state,
        VCsr::Vxsat as u16,
        1,
        &mut output,
    )
    .unwrap();
    state
        .ext_state
        .write_csr(VCsr::Vxsat as u16, output)
        .unwrap();

    // Read back: vcsr should reflect both
    let vcsr = state.ext_state.read_csr(VCsr::Vcsr as u16).unwrap();
    assert_eq!(vcsr, 0b101);

    // vxrm standalone should still be 0b10
    let vxrm = state.ext_state.read_csr(VCsr::Vxrm as u16).unwrap();
    assert_eq!(vxrm, 0b10);

    // vxsat standalone should be 1
    let vxsat = state.ext_state.read_csr(VCsr::Vxsat as u16).unwrap();
    assert_eq!(vxsat, 1);
}

#[test]
fn prepare_csr_read_vcsr_reflects_separate_csr_values() {
    let mut state = initialize_state::<ZveXxConfigInstruction<_>, _>([]);
    state.ext_state.init_vector_csrs();

    // Set vxsat=1 and vxrm=0b10 directly in storage
    state.ext_state.write_csr(VCsr::Vxsat as u16, 1).unwrap();
    state.ext_state.write_csr(VCsr::Vxrm as u16, 0b10).unwrap();
    // Manually compose what vcsr should be: [2:1]=vxrm=0b10, [0]=vxsat=1 -> 0b101
    state.ext_state.write_csr(VCsr::Vcsr as u16, 0b101).unwrap();

    let mut output = 0u64;
    let raw = state.ext_state.read_csr(VCsr::Vcsr as u16).unwrap();
    <ZveXxConfigInstruction<_>>::prepare_csr_read(
        &state.ext_state,
        VCsr::Vcsr as u16,
        raw,
        &mut output,
    )
    .unwrap();
    assert_eq!(output, 0b101);
}
