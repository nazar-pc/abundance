use crate::rv64::test_utils::{TEST_BASE_ADDR, execute, initialize_state};
use crate::{ExecutionError, RegisterFile, VirtualMemory};
use ab_riscv_primitives::prelude::*;

// C.LBU

#[test]
fn test_clbu_zero_extends() {
    let mut state = initialize_state([Rv64ZcbOnlyInstruction::CLbu {
        rd: Reg::A1,
        rs1: Reg::A0,
        uimm: 0,
        rs2: Reg::Zero,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u8>(addr, 0xFF).unwrap();
    state.regs.write(Reg::A0, addr);
    execute(&mut state).unwrap();
    // Zero-extend: 0xFF -> 255, not sign-extended to -1
    assert_eq!(state.regs.read(Reg::A1), 255);
}

#[test]
fn test_clbu_with_uimm_offset() {
    let mut state = initialize_state([Rv64ZcbOnlyInstruction::CLbu {
        rd: Reg::A1,
        rs1: Reg::A0,
        uimm: 3,
        rs2: Reg::Zero,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u8>(addr + 3, 42).unwrap();
    state.regs.write(Reg::A0, addr);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A1), 42);
}

#[test]
fn test_clbu_oob() {
    let mut state = initialize_state([Rv64ZcbOnlyInstruction::CLbu {
        rd: Reg::A1,
        rs1: Reg::A0,
        uimm: 0,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, 0);
    assert!(matches!(
        execute(&mut state),
        Err(ExecutionError::MemoryAccess(_))
    ));
}

// C.LHU

#[test]
fn test_clhu_zero_extends() {
    let mut state = initialize_state([Rv64ZcbOnlyInstruction::CLhu {
        rd: Reg::A1,
        rs1: Reg::A0,
        uimm: 0,
        rs2: Reg::Zero,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u16>(addr, 0xFFFF).unwrap();
    state.regs.write(Reg::A0, addr);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A1), 0xFFFF);
}

#[test]
fn test_clhu_with_uimm2() {
    let mut state = initialize_state([Rv64ZcbOnlyInstruction::CLhu {
        rd: Reg::A1,
        rs1: Reg::A0,
        uimm: 2,
        rs2: Reg::Zero,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u16>(addr + 2, 0x1234).unwrap();
    state.regs.write(Reg::A0, addr);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A1), 0x1234);
}

#[test]
fn test_clhu_oob() {
    let mut state = initialize_state([Rv64ZcbOnlyInstruction::CLhu {
        rd: Reg::A1,
        rs1: Reg::A0,
        uimm: 0,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, 0);
    assert!(matches!(
        execute(&mut state),
        Err(ExecutionError::MemoryAccess(_))
    ));
}

// C.LH

#[test]
fn test_clh_sign_extends_negative() {
    let mut state = initialize_state([Rv64ZcbOnlyInstruction::CLh {
        rd: Reg::A1,
        rs1: Reg::A0,
        uimm: 0,
        rs2: Reg::Zero,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<i16>(addr, -1).unwrap();
    state.regs.write(Reg::A0, addr);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A1), u64::MAX);
}

#[test]
fn test_clh_sign_extends_positive() {
    let mut state = initialize_state([Rv64ZcbOnlyInstruction::CLh {
        rd: Reg::A1,
        rs1: Reg::A0,
        uimm: 0,
        rs2: Reg::Zero,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<i16>(addr, 100).unwrap();
    state.regs.write(Reg::A0, addr);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A1), 100);
}

#[test]
fn test_clh_oob() {
    let mut state = initialize_state([Rv64ZcbOnlyInstruction::CLh {
        rd: Reg::A1,
        rs1: Reg::A0,
        uimm: 0,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, 0);
    assert!(matches!(
        execute(&mut state),
        Err(ExecutionError::MemoryAccess(_))
    ));
}

// C.SB

#[test]
fn test_csb_stores_low_byte() {
    let mut state = initialize_state([Rv64ZcbOnlyInstruction::CSb {
        rs1: Reg::A0,
        rs2: Reg::A1,
        uimm: 1,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 0xDEAD_BEEF_CAFE_0042);
    execute(&mut state).unwrap();
    assert_eq!(state.memory.read::<u8>(addr + 1).unwrap(), 0x42);
}

#[test]
fn test_csb_oob() {
    let mut state = initialize_state([Rv64ZcbOnlyInstruction::CSb {
        rs1: Reg::A0,
        rs2: Reg::A1,
        uimm: 0,
    }]);
    state.regs.write(Reg::A0, 0);
    assert!(matches!(
        execute(&mut state),
        Err(ExecutionError::MemoryAccess(_))
    ));
}

// C.SH

#[test]
fn test_csh_stores_low_halfword() {
    let mut state = initialize_state([Rv64ZcbOnlyInstruction::CSh {
        rs1: Reg::A0,
        rs2: Reg::A1,
        uimm: 0,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 0xDEAD_BEEF_CAFE_1234);
    execute(&mut state).unwrap();
    assert_eq!(state.memory.read::<u16>(addr).unwrap(), 0x1234);
}

#[test]
fn test_csh_oob() {
    let mut state = initialize_state([Rv64ZcbOnlyInstruction::CSh {
        rs1: Reg::A0,
        rs2: Reg::A1,
        uimm: 0,
    }]);
    state.regs.write(Reg::A0, 0);
    assert!(matches!(
        execute(&mut state),
        Err(ExecutionError::MemoryAccess(_))
    ));
}

// C.ZEXT.B

#[test]
fn test_czext_b() {
    let mut state = initialize_state([Rv64ZcbOnlyInstruction::CZextB {
        rd: Reg::A0,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, 0xDEAD_BEEF_CAFE_0042);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0x42);
}

#[test]
fn test_czext_b_zero() {
    let mut state = initialize_state([Rv64ZcbOnlyInstruction::CZextB {
        rd: Reg::A0,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, 0xFFFF_FFFF_FFFF_FF00);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0);
}

// C.SEXT.B

#[test]
fn test_csext_b_negative() {
    let mut state = initialize_state([Rv64ZcbOnlyInstruction::CSextB {
        rd: Reg::A0,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, 0xFF);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0xFFFF_FFFF_FFFF_FFFF);
}

#[test]
fn test_csext_b_positive() {
    let mut state = initialize_state([Rv64ZcbOnlyInstruction::CSextB {
        rd: Reg::A0,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, 0x7F);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0x7F);
}

// C.ZEXT.H

#[test]
fn test_czext_h() {
    let mut state = initialize_state([Rv64ZcbOnlyInstruction::CZextH {
        rd: Reg::A0,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, 0xDEAD_BEEF_CAFE_1234);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0x1234);
}

// C.SEXT.H

#[test]
fn test_csext_h_negative() {
    let mut state = initialize_state([Rv64ZcbOnlyInstruction::CSextH {
        rd: Reg::A0,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, 0x8000);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0xFFFF_FFFF_FFFF_8000);
}

#[test]
fn test_csext_h_positive() {
    let mut state = initialize_state([Rv64ZcbOnlyInstruction::CSextH {
        rd: Reg::A0,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, 0x7FFF);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0x7FFF);
}

// C.ZEXT.W

#[test]
fn test_czext_w() {
    let mut state = initialize_state([Rv64ZcbOnlyInstruction::CZextW {
        rd: Reg::A0,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, 0xDEAD_BEEF_FFFF_FFFF);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0xFFFF_FFFF);
}

#[test]
fn test_czext_w_zeros_upper() {
    let mut state = initialize_state([Rv64ZcbOnlyInstruction::CZextW {
        rd: Reg::A0,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, 0xFFFF_FFFF_0000_0000);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0);
}

// C.NOT

#[test]
fn test_cnot() {
    let mut state = initialize_state([Rv64ZcbOnlyInstruction::CNot {
        rd: Reg::A0,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, 0x5555_5555_5555_5555);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0xAAAA_AAAA_AAAA_AAAA);
}

#[test]
fn test_cnot_all_zeros() {
    let mut state = initialize_state([Rv64ZcbOnlyInstruction::CNot {
        rd: Reg::A0,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, 0);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), u64::MAX);
}

// C.MUL

#[test]
fn test_cmul_basic() {
    let mut state = initialize_state([Rv64ZcbOnlyInstruction::CMul {
        rd: Reg::S0,
        rs2: Reg::S1,
        rs1: Reg::Zero,
    }]);
    state.regs.write(Reg::S0, 7);
    state.regs.write(Reg::S1, 6);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::S0), 42);
}

#[test]
fn test_cmul_wraps() {
    let mut state = initialize_state([Rv64ZcbOnlyInstruction::CMul {
        rd: Reg::S0,
        rs2: Reg::S1,
        rs1: Reg::Zero,
    }]);
    state.regs.write(Reg::S0, u64::MAX);
    state.regs.write(Reg::S1, 2);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::S0), u64::MAX.wrapping_mul(2));
}
