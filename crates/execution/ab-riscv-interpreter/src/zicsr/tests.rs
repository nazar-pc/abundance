use crate::rv64::test_utils::{execute, initialize_state};
use crate::{CsrError, Csrs, ExecutionError, RegisterFile};
use ab_riscv_primitives::prelude::*;
use core::assert_matches;

// CSR address constants
//
// Address encoding (Vol II §2.1):
//   bits [11:10] - read-only if 0b11
//   bits  [9:8]  - minimum privilege: 0b00=U, 0b01=S, 0b11=M
//
// 0x001 = fflags        - bits[11:10]=0b00, bits[9:8]=0b00 -> writable, U-mode
// 0x100 = sstatus       - bits[11:10]=0b00, bits[9:8]=0b01 -> writable, S-mode
// 0x300 = mstatus       - bits[11:10]=0b00, bits[9:8]=0b11 -> writable, M-mode
// 0xBFF = (no std name) - bits[11:10]=0b10, bits[9:8]=0b11 -> writable, M-mode
//                          (last address before the RO region)
// 0xC00 = cycle         - bits[11:10]=0b11, bits[9:8]=0b00 -> read-only, U-mode
// 0xC80 = cycleh        - bits[11:10]=0b11, bits[9:8]=0b00 -> read-only, U-mode
// 0x200 = (reserved)    - bits[11:10]=0b00, bits[9:8]=0b10 -> reserved privilege

/// Writable, User-accessible (bits[9:8] = 0b00).
const U_CSR: u16 = 0x001;

/// Writable, Supervisor-accessible (bits[9:8] = 0b01).
const S_CSR: u16 = 0x100;

/// Writable, Machine-accessible (bits[9:8] = 0b11).
const M_CSR: u16 = 0x300;

/// Last writable CSR address (bits[11:10] = 0b10, bits[9:8] = 0b11).
const LAST_WRITABLE_CSR: u16 = 0xBFF;

/// First read-only CSR address (bits[11:10] = 0b11, bits[9:8] = 0b00).
const RO_CSR: u16 = 0xC00;

/// A reserved-privilege CSR (bits[9:8] = 0b10).
const RESERVED_PRIV_CSR: u16 = 0x200;

/// A CSR index the test harness has no knowledge of.
const UNKNOWN_CSR: u16 = 0x7FF;

// Helper closures passed to `set_prepare_csr_read_write`.
// These model identity-passthrough transforms (no WARL masking).

fn allow_read(_csr_index: u16, raw_value: u64) -> Result<u64, CsrError> {
    Ok(raw_value)
}

fn allow_write(_csr_index: u16, write_value: u64) -> Result<u64, CsrError> {
    Ok(write_value)
}

// CSRRW

#[test]
fn test_csrrw_reads_old_value_into_rd() {
    let mut state = initialize_state([ZicsrInstruction::Csrrw {
        rd: Reg::A2,
        rs1: Reg::A0,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state.ext_state.write_csr(U_CSR, 0xDEAD_BEEF).unwrap();
    state.regs.write(Reg::A0, 0x1234_5678u64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xDEAD_BEEF);
}

#[test]
fn test_csrrw_writes_rs1_to_csr() {
    let mut state = initialize_state([ZicsrInstruction::Csrrw {
        rd: Reg::A2,
        rs1: Reg::A0,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state.ext_state.write_csr(U_CSR, 0xDEAD_BEEF).unwrap();
    state.regs.write(Reg::A0, 0x1234_5678u64);

    execute(&mut state).unwrap();

    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0x1234_5678);
}

#[test]
fn test_csrrw_rd_zero_skips_read_no_side_effects() {
    let mut state = initialize_state([ZicsrInstruction::Csrrw {
        rd: Reg::Zero,
        rs1: Reg::A0,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state.ext_state.write_csr(U_CSR, 0xAAAA_BBBB).unwrap();
    state.regs.write(Reg::A0, 0xCCCC_DDDDu64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::Zero), 0);
    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0xCCCC_DDDD);
}

#[test]
fn test_csrrw_all_ones() {
    let mut state = initialize_state([ZicsrInstruction::Csrrw {
        rd: Reg::A1,
        rs1: Reg::A0,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state.ext_state.write_csr(U_CSR, 0u64).unwrap();
    state.regs.write(Reg::A0, u64::MAX);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A1), 0);
    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), u64::MAX);
}

#[test]
fn test_csrrw_overwrites_completely() {
    let mut state = initialize_state([ZicsrInstruction::Csrrw {
        rd: Reg::A1,
        rs1: Reg::A0,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state
        .ext_state
        .write_csr(U_CSR, 0xFFFF_FFFF_FFFF_FFFFu64)
        .unwrap();
    state.regs.write(Reg::A0, 0u64);

    execute(&mut state).unwrap();

    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0);
}

// CSRRS

#[test]
fn test_csrrs_reads_old_value_into_rd() {
    let mut state = initialize_state([ZicsrInstruction::Csrrs {
        rd: Reg::A2,
        rs1: Reg::A0,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state.ext_state.write_csr(U_CSR, 0x00FF_00FFu64).unwrap();
    state.regs.write(Reg::A0, 0xFF00_FF00u64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0x00FF_00FF);
}

#[test]
fn test_csrrs_sets_bits() {
    let mut state = initialize_state([ZicsrInstruction::Csrrs {
        rd: Reg::A2,
        rs1: Reg::A0,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state.ext_state.write_csr(U_CSR, 0x00FF_00FFu64).unwrap();
    state.regs.write(Reg::A0, 0xFF00_FF00u64);

    execute(&mut state).unwrap();

    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0xFFFF_FFFF);
}

#[test]
fn test_csrrs_rs1_zero_no_write() {
    let mut state = initialize_state([ZicsrInstruction::Csrrs {
        rd: Reg::A2,
        rs1: Reg::Zero,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state.ext_state.write_csr(U_CSR, 0xCAFE_BABEu64).unwrap();

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xCAFE_BABE);
    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0xCAFE_BABE);
}

#[test]
fn test_csrrs_idempotent_when_bits_already_set() {
    let mut state = initialize_state([ZicsrInstruction::Csrrs {
        rd: Reg::A2,
        rs1: Reg::A0,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state
        .ext_state
        .write_csr(U_CSR, 0xFFFF_FFFF_FFFF_FFFFu64)
        .unwrap();
    state.regs.write(Reg::A0, 0xFFFF_FFFF_FFFF_FFFFu64);

    execute(&mut state).unwrap();

    assert_eq!(
        state.ext_state.read_csr(U_CSR).unwrap(),
        0xFFFF_FFFF_FFFF_FFFF
    );
}

// CSRRC

#[test]
fn test_csrrc_reads_old_value_into_rd() {
    let mut state = initialize_state([ZicsrInstruction::Csrrc {
        rd: Reg::A2,
        rs1: Reg::A0,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state.ext_state.write_csr(U_CSR, 0xFFFF_FFFFu64).unwrap();
    state.regs.write(Reg::A0, 0x0F0F_0F0Fu64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xFFFF_FFFF);
}

#[test]
fn test_csrrc_clears_bits() {
    let mut state = initialize_state([ZicsrInstruction::Csrrc {
        rd: Reg::A2,
        rs1: Reg::A0,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state.ext_state.write_csr(U_CSR, 0xFFFF_FFFFu64).unwrap();
    state.regs.write(Reg::A0, 0x0F0F_0F0Fu64);

    execute(&mut state).unwrap();

    // 0xFFFF_FFFF & !0x0F0F_0F0F = 0xF0F0_F0F0
    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0xF0F0_F0F0);
}

#[test]
fn test_csrrc_rs1_zero_no_write() {
    let mut state = initialize_state([ZicsrInstruction::Csrrc {
        rd: Reg::A2,
        rs1: Reg::Zero,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state.ext_state.write_csr(U_CSR, 0xDEAD_C0DEu64).unwrap();

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xDEAD_C0DE);
    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0xDEAD_C0DE);
}

#[test]
fn test_csrrc_clears_all_bits() {
    let mut state = initialize_state([ZicsrInstruction::Csrrc {
        rd: Reg::A2,
        rs1: Reg::A0,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state
        .ext_state
        .write_csr(U_CSR, 0xFFFF_FFFF_FFFF_FFFFu64)
        .unwrap();
    state.regs.write(Reg::A0, 0xFFFF_FFFF_FFFF_FFFFu64);

    execute(&mut state).unwrap();

    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0);
}

#[test]
fn test_csrrc_idempotent_when_bits_already_clear() {
    let mut state = initialize_state([ZicsrInstruction::Csrrc {
        rd: Reg::A2,
        rs1: Reg::A0,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state.ext_state.write_csr(U_CSR, 0u64).unwrap();
    state.regs.write(Reg::A0, 0xFFFF_FFFF_FFFF_FFFFu64);

    execute(&mut state).unwrap();

    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0);
}

// CSRRWI

#[test]
fn test_csrrwi_reads_old_value_into_rd() {
    let mut state = initialize_state([ZicsrInstruction::Csrrwi {
        rd: Reg::A2,
        zimm: 0b11111,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state.ext_state.write_csr(U_CSR, 0xABCD_EF01u64).unwrap();

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xABCD_EF01);
}

#[test]
fn test_csrrwi_writes_zimm_zero_extended() {
    let mut state = initialize_state([ZicsrInstruction::Csrrwi {
        rd: Reg::A2,
        zimm: 0b11111,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state
        .ext_state
        .write_csr(U_CSR, 0xFFFF_FFFF_FFFF_FFFFu64)
        .unwrap();

    execute(&mut state).unwrap();

    // zimm is 5-bit zero-extended to 64 bits
    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0b11111u64);
}

#[test]
fn test_csrrwi_rd_zero_skips_read() {
    let mut state = initialize_state([ZicsrInstruction::Csrrwi {
        rd: Reg::Zero,
        zimm: 0b00101,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state.ext_state.write_csr(U_CSR, 0xFFFF_FFFFu64).unwrap();

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::Zero), 0);
    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0b00101);
}

#[test]
fn test_csrrwi_zimm_zero_writes_zero() {
    let mut state = initialize_state([ZicsrInstruction::Csrrwi {
        rd: Reg::A1,
        zimm: 0,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state.ext_state.write_csr(U_CSR, 0xFFFF_FFFFu64).unwrap();

    execute(&mut state).unwrap();

    // zimm=0 is still a write (csrrwi always writes, unlike csrrsi/csrrci)
    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0);
}

#[test]
fn test_csrrwi_max_zimm() {
    let mut state = initialize_state([ZicsrInstruction::Csrrwi {
        rd: Reg::A1,
        zimm: 31,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state.ext_state.write_csr(U_CSR, 0u64).unwrap();

    execute(&mut state).unwrap();

    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 31);
}

// CSRRSI

#[test]
fn test_csrrsi_reads_old_value_into_rd() {
    let mut state = initialize_state([ZicsrInstruction::Csrrsi {
        rd: Reg::A2,
        zimm: 0b00001,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state.ext_state.write_csr(U_CSR, 0xF0F0_F0F0u64).unwrap();

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xF0F0_F0F0);
}

#[test]
fn test_csrrsi_sets_bits() {
    let mut state = initialize_state([ZicsrInstruction::Csrrsi {
        rd: Reg::A2,
        zimm: 0b00111,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state.ext_state.write_csr(U_CSR, 0u64).unwrap();

    execute(&mut state).unwrap();

    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0b00111);
}

#[test]
fn test_csrrsi_zimm_zero_no_write() {
    let mut state = initialize_state([ZicsrInstruction::Csrrsi {
        rd: Reg::A2,
        zimm: 0,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state.ext_state.write_csr(U_CSR, 0xBEEF_CAFEu64).unwrap();

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xBEEF_CAFE);
    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0xBEEF_CAFE);
}

#[test]
fn test_csrrsi_does_not_clear_existing_bits() {
    let mut state = initialize_state([ZicsrInstruction::Csrrsi {
        rd: Reg::A2,
        zimm: 0b10101,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state
        .ext_state
        .write_csr(U_CSR, 0xFFFF_FFFF_FFFF_FF00u64)
        .unwrap();

    execute(&mut state).unwrap();

    assert_eq!(
        state.ext_state.read_csr(U_CSR).unwrap(),
        0xFFFF_FFFF_FFFF_FF15
    );
}

// CSRRCI

#[test]
fn test_csrrci_reads_old_value_into_rd() {
    let mut state = initialize_state([ZicsrInstruction::Csrrci {
        rd: Reg::A2,
        zimm: 0b00001,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state.ext_state.write_csr(U_CSR, 0x1234_5678u64).unwrap();

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0x1234_5678);
}

#[test]
fn test_csrrci_clears_bits() {
    let mut state = initialize_state([ZicsrInstruction::Csrrci {
        rd: Reg::A2,
        zimm: 0b11111,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state
        .ext_state
        .write_csr(U_CSR, 0xFFFF_FFFF_FFFF_FFFFu64)
        .unwrap();

    execute(&mut state).unwrap();

    // Only the low 5 bits are cleared
    assert_eq!(
        state.ext_state.read_csr(U_CSR).unwrap(),
        0xFFFF_FFFF_FFFF_FFE0
    );
}

#[test]
fn test_csrrci_zimm_zero_no_write() {
    let mut state = initialize_state([ZicsrInstruction::Csrrci {
        rd: Reg::A2,
        zimm: 0,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state.ext_state.write_csr(U_CSR, 0xDEAD_BEEFu64).unwrap();

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xDEAD_BEEF);
    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0xDEAD_BEEF);
}

#[test]
fn test_csrrci_does_not_set_new_bits() {
    let mut state = initialize_state([ZicsrInstruction::Csrrci {
        rd: Reg::A2,
        zimm: 0b10101,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state.ext_state.write_csr(U_CSR, 0u64).unwrap();

    execute(&mut state).unwrap();

    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0);
}

#[test]
fn test_csrrci_partial_clear() {
    let mut state = initialize_state([ZicsrInstruction::Csrrci {
        rd: Reg::A2,
        zimm: 0b01010,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state.ext_state.write_csr(U_CSR, 0b11111u64).unwrap();

    execute(&mut state).unwrap();

    // 0b11111 & !0b01010 = 0b10101
    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0b10101);
}

// Cross-instruction: rd=x0 hardwiring

#[test]
fn test_csrrs_rd_zero_still_reads_no_gp_write() {
    let mut state = initialize_state([ZicsrInstruction::Csrrs {
        rd: Reg::Zero,
        rs1: Reg::A0,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state.ext_state.write_csr(U_CSR, 0b0000_0001u64).unwrap();
    state.regs.write(Reg::A0, 0b0000_0010u64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::Zero), 0);
    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0b0000_0011);
}

#[test]
fn test_csrrc_rd_zero_still_reads_no_gp_write() {
    let mut state = initialize_state([ZicsrInstruction::Csrrc {
        rd: Reg::Zero,
        rs1: Reg::A0,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state.ext_state.write_csr(U_CSR, 0b1111u64).unwrap();
    state.regs.write(Reg::A0, 0b0011u64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::Zero), 0);
    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0b1100);
}

// Atomicity: rd/rs1 aliasing

#[test]
fn test_csrrw_rd_rs1_alias() {
    let mut state = initialize_state([ZicsrInstruction::Csrrw {
        rd: Reg::A0,
        rs1: Reg::A0,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state.ext_state.write_csr(U_CSR, 0xAAAAu64).unwrap();
    state.regs.write(Reg::A0, 0xBBBBu64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A0), 0xAAAA);
    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0xBBBB);
}

#[test]
fn test_csrrs_rd_rs1_alias() {
    let mut state = initialize_state([ZicsrInstruction::Csrrs {
        rd: Reg::A0,
        rs1: Reg::A0,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state.ext_state.write_csr(U_CSR, 0b0101u64).unwrap();
    state.regs.write(Reg::A0, 0b1010u64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A0), 0b0101);
    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0b1111);
}

#[test]
fn test_csrrc_rd_rs1_alias() {
    let mut state = initialize_state([ZicsrInstruction::Csrrc {
        rd: Reg::A0,
        rs1: Reg::A0,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state.ext_state.write_csr(U_CSR, 0b1111u64).unwrap();
    state.regs.write(Reg::A0, 0b0011u64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A0), 0b1111);
    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0b1100);
}

// Read-only CSR enforcement (bits[11:10] == 0b11)

#[test]
fn test_csrrw_read_only_csr_is_rejected() {
    let mut state = initialize_state([ZicsrInstruction::Csrrw {
        rd: Reg::A2,
        rs1: Reg::A0,
        csr: RO_CSR,
    }]);
    state.ext_state.init_csr(RO_CSR, 0x1234);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state.regs.write(Reg::A0, 0xFFFFu64);

    let result = execute(&mut state);

    assert!(result.is_err());
    assert_eq!(state.ext_state.read_csr(RO_CSR).unwrap(), 0x1234);
}

#[test]
fn test_csrrwi_read_only_csr_is_rejected() {
    let mut state = initialize_state([ZicsrInstruction::Csrrwi {
        rd: Reg::A2,
        zimm: 1,
        csr: RO_CSR,
    }]);
    state.ext_state.init_csr(RO_CSR, 0x1234);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    let result = execute(&mut state);

    assert!(result.is_err());
    assert_eq!(state.ext_state.read_csr(RO_CSR).unwrap(), 0x1234);
}

#[test]
fn test_csrrs_read_only_csr_with_nonzero_rs1_is_rejected() {
    let mut state = initialize_state([ZicsrInstruction::Csrrs {
        rd: Reg::A2,
        rs1: Reg::A0,
        csr: RO_CSR,
    }]);
    state.ext_state.init_csr(RO_CSR, 0x1234);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state.regs.write(Reg::A0, 0b1u64);

    let result = execute(&mut state);

    assert!(result.is_err());
    assert_eq!(state.ext_state.read_csr(RO_CSR).unwrap(), 0x1234);
}

#[test]
fn test_csrrc_read_only_csr_with_nonzero_rs1_is_rejected() {
    let mut state = initialize_state([ZicsrInstruction::Csrrc {
        rd: Reg::A2,
        rs1: Reg::A0,
        csr: RO_CSR,
    }]);
    state.ext_state.init_csr(RO_CSR, 0x1234);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    state.regs.write(Reg::A0, 0b1u64);

    let result = execute(&mut state);

    assert!(result.is_err());
    assert_eq!(state.ext_state.read_csr(RO_CSR).unwrap(), 0x1234);
}

#[test]
fn test_csrrsi_read_only_csr_with_nonzero_zimm_is_rejected() {
    let mut state = initialize_state([ZicsrInstruction::Csrrsi {
        rd: Reg::A2,
        zimm: 1,
        csr: RO_CSR,
    }]);
    state.ext_state.init_csr(RO_CSR, 0x1234);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    let result = execute(&mut state);

    assert!(result.is_err());
    assert_eq!(state.ext_state.read_csr(RO_CSR).unwrap(), 0x1234);
}

#[test]
fn test_csrrci_read_only_csr_with_nonzero_zimm_is_rejected() {
    let mut state = initialize_state([ZicsrInstruction::Csrrci {
        rd: Reg::A2,
        zimm: 1,
        csr: RO_CSR,
    }]);
    state.ext_state.init_csr(RO_CSR, 0x1234);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    let result = execute(&mut state);

    assert!(result.is_err());
    assert_eq!(state.ext_state.read_csr(RO_CSR).unwrap(), 0x1234);
}

// Read-only CSR with a no-write access pattern is legal per spec.

#[test]
fn test_csrrs_read_only_csr_with_rs1_zero_is_legal() {
    // csrrs rd, ro_csr, x0 is the canonical CSR read idiom; must succeed even on RO CSRs.
    let mut state = initialize_state([ZicsrInstruction::Csrrs {
        rd: Reg::A2,
        rs1: Reg::Zero,
        csr: RO_CSR,
    }]);
    state.ext_state.init_csr(RO_CSR, 0xABCD);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xABCD);
    assert_eq!(state.ext_state.read_csr(RO_CSR).unwrap(), 0xABCD);
}

#[test]
fn test_csrrc_read_only_csr_with_rs1_zero_is_legal() {
    let mut state = initialize_state([ZicsrInstruction::Csrrc {
        rd: Reg::A2,
        rs1: Reg::Zero,
        csr: RO_CSR,
    }]);
    state.ext_state.init_csr(RO_CSR, 0xABCD);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xABCD);
    assert_eq!(state.ext_state.read_csr(RO_CSR).unwrap(), 0xABCD);
}

#[test]
fn test_csrrsi_read_only_csr_with_zimm_zero_is_legal() {
    let mut state = initialize_state([ZicsrInstruction::Csrrsi {
        rd: Reg::A2,
        zimm: 0,
        csr: RO_CSR,
    }]);
    state.ext_state.init_csr(RO_CSR, 0xABCD);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xABCD);
    assert_eq!(state.ext_state.read_csr(RO_CSR).unwrap(), 0xABCD);
}

#[test]
fn test_csrrci_read_only_csr_with_zimm_zero_is_legal() {
    let mut state = initialize_state([ZicsrInstruction::Csrrci {
        rd: Reg::A2,
        zimm: 0,
        csr: RO_CSR,
    }]);
    state.ext_state.init_csr(RO_CSR, 0xABCD);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xABCD);
    assert_eq!(state.ext_state.read_csr(RO_CSR).unwrap(), 0xABCD);
}

// Precise address-space boundary between writable and read-only.

#[test]
fn test_csrrw_last_writable_address_succeeds() {
    let mut state = initialize_state([ZicsrInstruction::Csrrw {
        rd: Reg::A1,
        rs1: Reg::A0,
        csr: LAST_WRITABLE_CSR,
    }]);
    state.ext_state.init_csr(LAST_WRITABLE_CSR, 0x10);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);
    state.ext_state.set_privilege_level(PrivilegeLevel::Machine);
    state.regs.write(Reg::A0, 0x20u64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A1), 0x10);
    assert_eq!(state.ext_state.read_csr(LAST_WRITABLE_CSR).unwrap(), 0x20);
}

#[test]
fn test_csrrw_first_read_only_address_is_rejected() {
    let mut state = initialize_state([ZicsrInstruction::Csrrw {
        rd: Reg::A1,
        rs1: Reg::A0,
        csr: RO_CSR,
    }]);
    state.ext_state.init_csr(RO_CSR, 0x10);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);
    state.ext_state.set_privilege_level(PrivilegeLevel::Machine);
    state.regs.write(Reg::A0, 0x20u64);

    assert!(execute(&mut state).is_err());
    assert_eq!(state.ext_state.read_csr(RO_CSR).unwrap(), 0x10);
}

// Privilege level enforcement
//
// CSR bits[9:8] encode minimum required privilege:
//   0b00 -> User (U_CSR = 0x001)
//   0b01 -> Supervisor (S_CSR = 0x100)
//   0b11 -> Machine (M_CSR = 0x300)
//
// For each privilege level we test:
//   - access at the exact required level succeeds
//   - access one level below is rejected
//   - Machine can access everything (the highest privilege)
//
// We use CSRRW as the representative instruction for brevity; the privilege
// check is shared code exercised identically across all six instructions.
// Dedicated per-instruction tests cover the boundary cases.

// --- User-mode CSR (bits[9:8] = 0b00) ---

#[test]
fn test_priv_user_csr_accessible_from_user_mode() {
    let mut state = initialize_state([ZicsrInstruction::Csrrw {
        rd: Reg::A1,
        rs1: Reg::A0,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);
    state.ext_state.set_privilege_level(PrivilegeLevel::User);
    state.regs.write(Reg::A0, 0x1u64);

    execute(&mut state).unwrap();
}

#[test]
fn test_priv_user_csr_accessible_from_supervisor_mode() {
    let mut state = initialize_state([ZicsrInstruction::Csrrw {
        rd: Reg::A1,
        rs1: Reg::A0,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);
    state
        .ext_state
        .set_privilege_level(PrivilegeLevel::Supervisor);
    state.regs.write(Reg::A0, 0x1u64);

    execute(&mut state).unwrap();
}

#[test]
fn test_priv_user_csr_accessible_from_machine_mode() {
    let mut state = initialize_state([ZicsrInstruction::Csrrw {
        rd: Reg::A1,
        rs1: Reg::A0,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);
    state.ext_state.set_privilege_level(PrivilegeLevel::Machine);
    state.regs.write(Reg::A0, 0x1u64);

    execute(&mut state).unwrap();
}

// --- Supervisor-mode CSR (bits[9:8] = 0b01) ---

#[test]
fn test_priv_supervisor_csr_rejected_from_user_mode() {
    let mut state = initialize_state([ZicsrInstruction::Csrrw {
        rd: Reg::A1,
        rs1: Reg::A0,
        csr: S_CSR,
    }]);
    state.ext_state.init_csr(S_CSR, 0xDEAD);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);
    state.ext_state.set_privilege_level(PrivilegeLevel::User);
    state.regs.write(Reg::A0, 0x1u64);

    assert!(execute(&mut state).is_err());
    assert_eq!(state.ext_state.read_csr(S_CSR).unwrap(), 0xDEAD);
}

#[test]
fn test_priv_supervisor_csr_accessible_from_supervisor_mode() {
    let mut state = initialize_state([ZicsrInstruction::Csrrw {
        rd: Reg::A1,
        rs1: Reg::A0,
        csr: S_CSR,
    }]);
    state.ext_state.init_csr(S_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);
    state
        .ext_state
        .set_privilege_level(PrivilegeLevel::Supervisor);
    state.regs.write(Reg::A0, 0x1u64);

    execute(&mut state).unwrap();
}

#[test]
fn test_priv_supervisor_csr_accessible_from_machine_mode() {
    let mut state = initialize_state([ZicsrInstruction::Csrrw {
        rd: Reg::A1,
        rs1: Reg::A0,
        csr: S_CSR,
    }]);
    state.ext_state.init_csr(S_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);
    state.ext_state.set_privilege_level(PrivilegeLevel::Machine);
    state.regs.write(Reg::A0, 0x1u64);

    execute(&mut state).unwrap();
}

// --- Machine-mode CSR (bits[9:8] = 0b11) ---

#[test]
fn test_priv_machine_csr_rejected_from_user_mode() {
    let mut state = initialize_state([ZicsrInstruction::Csrrw {
        rd: Reg::A1,
        rs1: Reg::A0,
        csr: M_CSR,
    }]);
    state.ext_state.init_csr(M_CSR, 0xDEAD);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);
    state.ext_state.set_privilege_level(PrivilegeLevel::User);
    state.regs.write(Reg::A0, 0x1u64);

    assert!(execute(&mut state).is_err());
    assert_eq!(state.ext_state.read_csr(M_CSR).unwrap(), 0xDEAD);
}

#[test]
fn test_priv_machine_csr_rejected_from_supervisor_mode() {
    let mut state = initialize_state([ZicsrInstruction::Csrrw {
        rd: Reg::A1,
        rs1: Reg::A0,
        csr: M_CSR,
    }]);
    state.ext_state.init_csr(M_CSR, 0xDEAD);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);
    state
        .ext_state
        .set_privilege_level(PrivilegeLevel::Supervisor);
    state.regs.write(Reg::A0, 0x1u64);

    assert!(execute(&mut state).is_err());
    assert_eq!(state.ext_state.read_csr(M_CSR).unwrap(), 0xDEAD);
}

#[test]
fn test_priv_machine_csr_accessible_from_machine_mode() {
    let mut state = initialize_state([ZicsrInstruction::Csrrw {
        rd: Reg::A1,
        rs1: Reg::A0,
        csr: M_CSR,
    }]);
    state.ext_state.init_csr(M_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);
    state.ext_state.set_privilege_level(PrivilegeLevel::Machine);
    state.regs.write(Reg::A0, 0x1u64);

    execute(&mut state).unwrap();
}

// Privilege check fires before any other side-effects: CSR must be unchanged.

#[test]
fn test_priv_check_fires_before_csr_is_read_or_written() {
    // Supervisor CSR accessed from User mode - rd would normally receive the old
    // value, but the privilege check must abort before any access occurs.
    let mut state = initialize_state([ZicsrInstruction::Csrrw {
        rd: Reg::A2,
        rs1: Reg::A0,
        csr: S_CSR,
    }]);
    state.ext_state.init_csr(S_CSR, 0xBEEF);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);
    state.ext_state.set_privilege_level(PrivilegeLevel::User);
    state.regs.write(Reg::A0, 0x1234u64);

    let _ = execute(&mut state);

    // Neither the general-purpose register nor the CSR must have been modified.
    assert_eq!(state.regs.read(Reg::A2), 0);
    assert_eq!(state.ext_state.read_csr(S_CSR).unwrap(), 0xBEEF);
}

// Per-instruction privilege check coverage (one failing case each).

#[test]
fn test_csrrs_privilege_check() {
    let mut state = initialize_state([ZicsrInstruction::Csrrs {
        rd: Reg::A2,
        rs1: Reg::Zero,
        csr: M_CSR,
    }]);
    state.ext_state.init_csr(M_CSR, 0xDEAD);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);
    state
        .ext_state
        .set_privilege_level(PrivilegeLevel::Supervisor);

    assert!(execute(&mut state).is_err());
    assert_eq!(state.ext_state.read_csr(M_CSR).unwrap(), 0xDEAD);
}

#[test]
fn test_csrrc_privilege_check() {
    let mut state = initialize_state([ZicsrInstruction::Csrrc {
        rd: Reg::A2,
        rs1: Reg::Zero,
        csr: M_CSR,
    }]);
    state.ext_state.init_csr(M_CSR, 0xDEAD);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);
    state
        .ext_state
        .set_privilege_level(PrivilegeLevel::Supervisor);

    assert!(execute(&mut state).is_err());
    assert_eq!(state.ext_state.read_csr(M_CSR).unwrap(), 0xDEAD);
}

#[test]
fn test_csrrwi_privilege_check() {
    let mut state = initialize_state([ZicsrInstruction::Csrrwi {
        rd: Reg::A2,
        zimm: 1,
        csr: M_CSR,
    }]);
    state.ext_state.init_csr(M_CSR, 0xDEAD);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);
    state.ext_state.set_privilege_level(PrivilegeLevel::User);

    assert!(execute(&mut state).is_err());
    assert_eq!(state.ext_state.read_csr(M_CSR).unwrap(), 0xDEAD);
}

#[test]
fn test_csrrsi_privilege_check() {
    let mut state = initialize_state([ZicsrInstruction::Csrrsi {
        rd: Reg::A2,
        zimm: 1,
        csr: S_CSR,
    }]);
    state.ext_state.init_csr(S_CSR, 0xDEAD);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);
    state.ext_state.set_privilege_level(PrivilegeLevel::User);

    assert!(execute(&mut state).is_err());
    assert_eq!(state.ext_state.read_csr(S_CSR).unwrap(), 0xDEAD);
}

#[test]
fn test_csrrci_privilege_check() {
    let mut state = initialize_state([ZicsrInstruction::Csrrci {
        rd: Reg::A2,
        zimm: 1,
        csr: S_CSR,
    }]);
    state.ext_state.init_csr(S_CSR, 0xDEAD);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);
    state.ext_state.set_privilege_level(PrivilegeLevel::User);

    assert!(execute(&mut state).is_err());
    assert_eq!(state.ext_state.read_csr(S_CSR).unwrap(), 0xDEAD);
}

// Reserved-privilege encoding (bits[9:8] = 0b10) maps to Machine, so any
// mode below Machine must be rejected.

#[test]
fn test_reserved_privilege_csr_rejected_from_supervisor_mode() {
    let mut state = initialize_state([ZicsrInstruction::Csrrw {
        rd: Reg::A1,
        rs1: Reg::A0,
        csr: RESERVED_PRIV_CSR,
    }]);
    state.ext_state.init_csr(RESERVED_PRIV_CSR, 0xDEAD);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);
    state
        .ext_state
        .set_privilege_level(PrivilegeLevel::Supervisor);
    state.regs.write(Reg::A0, 0x1u64);

    assert!(execute(&mut state).is_err());
    assert_eq!(state.ext_state.read_csr(RESERVED_PRIV_CSR).unwrap(), 0xDEAD);
}

#[test]
fn test_reserved_privilege_csr_rejected_from_user_mode() {
    let mut state = initialize_state([ZicsrInstruction::Csrrw {
        rd: Reg::A1,
        rs1: Reg::A0,
        csr: RESERVED_PRIV_CSR,
    }]);
    state.ext_state.init_csr(RESERVED_PRIV_CSR, 0xDEAD);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);
    state.ext_state.set_privilege_level(PrivilegeLevel::User);
    state.regs.write(Reg::A0, 0x1u64);

    assert!(execute(&mut state).is_err());
    assert_eq!(state.ext_state.read_csr(RESERVED_PRIV_CSR).unwrap(), 0xDEAD);
}

#[test]
fn test_reserved_privilege_csr() {
    let mut state = initialize_state([ZicsrInstruction::Csrrw {
        rd: Reg::A1,
        rs1: Reg::A0,
        csr: RESERVED_PRIV_CSR,
    }]);
    // Do not initialize unknown CSR
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, allow_write);
    state.ext_state.set_privilege_level(PrivilegeLevel::Machine);
    state.regs.write(Reg::A0, 0x1u64);

    assert_matches!(
        execute(&mut state),
        Err(ExecutionError::CsrError(CsrError::IllegalRead {
            csr_index: RESERVED_PRIV_CSR
        }))
    );
}

// Error propagation: prepare_csr_read / prepare_csr_write

#[test]
fn test_csrrw_prepare_read_error_is_propagated() {
    let mut state = initialize_state([ZicsrInstruction::Csrrw {
        rd: Reg::A2,
        rs1: Reg::A0,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0xDEAD);
    state.ext_state.set_prepare_csr_read_write(
        |csr_index, _| Err(CsrError::IllegalRead { csr_index }),
        allow_write,
    );
    state.regs.write(Reg::A0, 0xBEEFu64);

    assert!(execute(&mut state).is_err());
    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0xDEAD);
}

#[test]
fn test_csrrw_rd_zero_prepare_write_error_is_propagated() {
    let mut state = initialize_state([ZicsrInstruction::Csrrw {
        rd: Reg::Zero,
        rs1: Reg::A0,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0xDEAD);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, |csr_index, _| {
            Err(CsrError::IllegalWrite { csr_index })
        });
    state.regs.write(Reg::A0, 0xBEEFu64);

    assert!(execute(&mut state).is_err());
    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0xDEAD);
}

#[test]
fn test_csrrs_prepare_read_error_is_propagated() {
    let mut state = initialize_state([ZicsrInstruction::Csrrs {
        rd: Reg::A2,
        rs1: Reg::A0,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0xDEAD);
    state.ext_state.set_prepare_csr_read_write(
        |csr_index, _| Err(CsrError::IllegalRead { csr_index }),
        allow_write,
    );
    state.regs.write(Reg::A0, 0b1u64);

    assert!(execute(&mut state).is_err());
    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0xDEAD);
}

#[test]
fn test_csrrs_prepare_write_error_is_propagated() {
    let mut state = initialize_state([ZicsrInstruction::Csrrs {
        rd: Reg::A2,
        rs1: Reg::A0,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0xDEAD);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, |csr_index, _| {
            Err(CsrError::IllegalWrite { csr_index })
        });
    state.regs.write(Reg::A0, 0b1u64);

    assert!(execute(&mut state).is_err());
    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0xDEAD);
}

#[test]
fn test_csrrc_prepare_read_error_is_propagated() {
    let mut state = initialize_state([ZicsrInstruction::Csrrc {
        rd: Reg::A2,
        rs1: Reg::A0,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0xDEAD);
    state.ext_state.set_prepare_csr_read_write(
        |csr_index, _| Err(CsrError::IllegalRead { csr_index }),
        allow_write,
    );
    state.regs.write(Reg::A0, 0b1u64);

    assert!(execute(&mut state).is_err());
    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0xDEAD);
}

#[test]
fn test_csrrc_prepare_write_error_is_propagated() {
    let mut state = initialize_state([ZicsrInstruction::Csrrc {
        rd: Reg::A2,
        rs1: Reg::A0,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0xDEAD);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, |csr_index, _| {
            Err(CsrError::IllegalWrite { csr_index })
        });
    state.regs.write(Reg::A0, 0b1u64);

    assert!(execute(&mut state).is_err());
    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0xDEAD);
}

#[test]
fn test_csrrwi_prepare_read_error_is_propagated() {
    let mut state = initialize_state([ZicsrInstruction::Csrrwi {
        rd: Reg::A2,
        zimm: 1,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0xDEAD);
    state.ext_state.set_prepare_csr_read_write(
        |csr_index, _| Err(CsrError::IllegalRead { csr_index }),
        allow_write,
    );

    assert!(execute(&mut state).is_err());
    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0xDEAD);
}

#[test]
fn test_csrrwi_prepare_write_error_is_propagated() {
    let mut state = initialize_state([ZicsrInstruction::Csrrwi {
        rd: Reg::Zero,
        zimm: 1,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0xDEAD);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, |csr_index, _| {
            Err(CsrError::IllegalWrite { csr_index })
        });

    assert!(execute(&mut state).is_err());
    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0xDEAD);
}

#[test]
fn test_csrrsi_prepare_read_error_is_propagated() {
    let mut state = initialize_state([ZicsrInstruction::Csrrsi {
        rd: Reg::A2,
        zimm: 1,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0xDEAD);
    state.ext_state.set_prepare_csr_read_write(
        |csr_index, _| Err(CsrError::IllegalRead { csr_index }),
        allow_write,
    );

    assert!(execute(&mut state).is_err());
    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0xDEAD);
}

#[test]
fn test_csrrsi_prepare_write_error_is_propagated() {
    let mut state = initialize_state([ZicsrInstruction::Csrrsi {
        rd: Reg::A2,
        zimm: 1,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0xDEAD);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, |csr_index, _| {
            Err(CsrError::IllegalWrite { csr_index })
        });

    assert!(execute(&mut state).is_err());
    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0xDEAD);
}

#[test]
fn test_csrrci_prepare_read_error_is_propagated() {
    let mut state = initialize_state([ZicsrInstruction::Csrrci {
        rd: Reg::A2,
        zimm: 1,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0xDEAD);
    state.ext_state.set_prepare_csr_read_write(
        |csr_index, _| Err(CsrError::IllegalRead { csr_index }),
        allow_write,
    );

    assert!(execute(&mut state).is_err());
    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0xDEAD);
}

#[test]
fn test_csrrci_prepare_write_error_is_propagated() {
    let mut state = initialize_state([ZicsrInstruction::Csrrci {
        rd: Reg::A2,
        zimm: 1,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0xDEAD);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, |csr_index, _| {
            Err(CsrError::IllegalWrite { csr_index })
        });

    assert!(execute(&mut state).is_err());
    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0xDEAD);
}

// Unknown CSR index

#[test]
fn test_csrrw_unknown_csr_returns_error() {
    let mut state = initialize_state([ZicsrInstruction::Csrrw {
        rd: Reg::A2,
        rs1: Reg::A0,
        csr: UNKNOWN_CSR,
    }]);
    state.regs.write(Reg::A0, 0x1234u64);

    assert!(execute(&mut state).is_err());
}

#[test]
fn test_csrrs_unknown_csr_returns_error() {
    let mut state = initialize_state([ZicsrInstruction::Csrrs {
        rd: Reg::A2,
        rs1: Reg::A0,
        csr: UNKNOWN_CSR,
    }]);
    state.regs.write(Reg::A0, 0x1u64);

    assert!(execute(&mut state).is_err());
}

#[test]
fn test_csrrc_unknown_csr_returns_error() {
    let mut state = initialize_state([ZicsrInstruction::Csrrc {
        rd: Reg::A2,
        rs1: Reg::A0,
        csr: UNKNOWN_CSR,
    }]);
    state.regs.write(Reg::A0, 0x1u64);

    assert!(execute(&mut state).is_err());
}

#[test]
fn test_csrrwi_unknown_csr_returns_error() {
    let mut state = initialize_state([ZicsrInstruction::Csrrwi {
        rd: Reg::A2,
        zimm: 1,
        csr: UNKNOWN_CSR,
    }]);

    assert!(execute(&mut state).is_err());
}

#[test]
fn test_csrrsi_unknown_csr_returns_error() {
    let mut state = initialize_state([ZicsrInstruction::Csrrsi {
        rd: Reg::A2,
        zimm: 1,
        csr: UNKNOWN_CSR,
    }]);

    assert!(execute(&mut state).is_err());
}

#[test]
fn test_csrrci_unknown_csr_returns_error() {
    let mut state = initialize_state([ZicsrInstruction::Csrrci {
        rd: Reg::A2,
        zimm: 1,
        csr: UNKNOWN_CSR,
    }]);

    assert!(execute(&mut state).is_err());
}

// prepare_csr_read/write filtering (non-identity transforms / WARL)

#[test]
fn test_prepare_csr_read_filtered_value_reaches_rd() {
    // Simulates a 32-bit-wide CSR that is zero-extended on read.
    let mut state = initialize_state([ZicsrInstruction::Csrrs {
        rd: Reg::A2,
        rs1: Reg::Zero,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0xDEAD_BEEF_1234_5678u64);
    state
        .ext_state
        .set_prepare_csr_read_write(|_, raw| Ok(raw & 0xFFFF_FFFF), allow_write);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0x1234_5678);
}

#[test]
fn test_prepare_csr_write_filtered_value_reaches_csr() {
    // Simulates WARL: low byte is ignored on write.
    let mut state = initialize_state([ZicsrInstruction::Csrrw {
        rd: Reg::A2,
        rs1: Reg::A0,
        csr: U_CSR,
    }]);
    state.ext_state.init_csr(U_CSR, 0);
    state
        .ext_state
        .set_prepare_csr_read_write(allow_read, |_, val| Ok(val & !0xFF));
    state.regs.write(Reg::A0, 0xABCD_EFFFu64);

    execute(&mut state).unwrap();

    assert_eq!(state.ext_state.read_csr(U_CSR).unwrap(), 0xABCD_EF00);
}
