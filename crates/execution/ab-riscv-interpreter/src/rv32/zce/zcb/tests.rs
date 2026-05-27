use crate::rv32::test_utils::{TEST_BASE_ADDR, execute, initialize_state};
use crate::{RegisterFile, VirtualMemory};
use ab_riscv_primitives::prelude::*;

// C.LBU

#[test]
fn test_clbu_zero_extends() {
    let mut state = initialize_state([Rv32ZcbOnlyInstruction::CLbu {
        rd: Reg::A1,
        rs1: Reg::A0,
        uimm: 0,
        rs2: Reg::Zero,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u8>(u64::from(addr), 0xFF).unwrap();
    state.regs.write(Reg::A0, addr);
    execute(&mut state).unwrap();
    // Zero-extend: 0xFF -> 255, not sign-extended to -1
    assert_eq!(state.regs.read(Reg::A1), 255);
}

#[test]
fn test_clbu_with_uimm_offset() {
    let mut state = initialize_state([Rv32ZcbOnlyInstruction::CLbu {
        rd: Reg::A1,
        rs1: Reg::A0,
        uimm: 3,
        rs2: Reg::Zero,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u8>(u64::from(addr + 3), 42).unwrap();
    state.regs.write(Reg::A0, addr);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A1), 42);
}

#[test]
fn test_clbu_oob() {
    let mut state = initialize_state([Rv32ZcbOnlyInstruction::CLbu {
        rd: Reg::A1,
        rs1: Reg::A0,
        uimm: 0,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, 0);
    assert!(matches!(
        execute(&mut state),
        Err(crate::ExecutionError::MemoryAccess(_))
    ));
}

// C.LHU

#[test]
fn test_clhu_zero_extends() {
    let mut state = initialize_state([Rv32ZcbOnlyInstruction::CLhu {
        rd: Reg::A1,
        rs1: Reg::A0,
        uimm: 0,
        rs2: Reg::Zero,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u16>(u64::from(addr), 0xFFFF).unwrap();
    state.regs.write(Reg::A0, addr);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A1), 0xFFFF);
}

#[test]
fn test_clhu_with_uimm2() {
    let mut state = initialize_state([Rv32ZcbOnlyInstruction::CLhu {
        rd: Reg::A1,
        rs1: Reg::A0,
        uimm: 2,
        rs2: Reg::Zero,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state
        .memory
        .write::<u16>(u64::from(addr + 2), 0x1234)
        .unwrap();
    state.regs.write(Reg::A0, addr);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A1), 0x1234);
}

#[test]
fn test_clhu_oob() {
    let mut state = initialize_state([Rv32ZcbOnlyInstruction::CLhu {
        rd: Reg::A1,
        rs1: Reg::A0,
        uimm: 0,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, 0);
    assert!(matches!(
        execute(&mut state),
        Err(crate::ExecutionError::MemoryAccess(_))
    ));
}

// C.LH

#[test]
fn test_clh_sign_extends() {
    let mut state = initialize_state([Rv32ZcbOnlyInstruction::CLh {
        rd: Reg::A1,
        rs1: Reg::A0,
        uimm: 0,
        rs2: Reg::Zero,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<i16>(u64::from(addr), -1).unwrap();
    state.regs.write(Reg::A0, addr);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A1), u32::MAX);
}

#[test]
fn test_clh_sign_extends_positive() {
    let mut state = initialize_state([Rv32ZcbOnlyInstruction::CLh {
        rd: Reg::A1,
        rs1: Reg::A0,
        uimm: 0,
        rs2: Reg::Zero,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<i16>(u64::from(addr), 100).unwrap();
    state.regs.write(Reg::A0, addr);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A1), 100);
}

#[test]
fn test_clh_oob() {
    let mut state = initialize_state([Rv32ZcbOnlyInstruction::CLh {
        rd: Reg::A1,
        rs1: Reg::A0,
        uimm: 0,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, 0);
    assert!(matches!(
        execute(&mut state),
        Err(crate::ExecutionError::MemoryAccess(_))
    ));
}

// C.SB / C.SH

#[test]
fn test_csb() {
    let mut state = initialize_state([Rv32ZcbOnlyInstruction::CSb {
        rs1: Reg::A0,
        rs2: Reg::A1,
        uimm: 0,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 0xDEAD_BE42);
    execute(&mut state).unwrap();
    assert_eq!(state.memory.read::<u8>(u64::from(addr)).unwrap(), 0x42);
}

#[test]
fn test_csb_with_uimm_offset() {
    let mut state = initialize_state([Rv32ZcbOnlyInstruction::CSb {
        rs1: Reg::A0,
        rs2: Reg::A1,
        uimm: 1,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 0xDEAD_BE42);
    execute(&mut state).unwrap();
    assert_eq!(state.memory.read::<u8>(u64::from(addr + 1)).unwrap(), 0x42);
}

#[test]
fn test_csb_oob() {
    let mut state = initialize_state([Rv32ZcbOnlyInstruction::CSb {
        rs1: Reg::A0,
        rs2: Reg::A1,
        uimm: 0,
    }]);
    state.regs.write(Reg::A0, 0);
    assert!(matches!(
        execute(&mut state),
        Err(crate::ExecutionError::MemoryAccess(_))
    ));
}

#[test]
fn test_csh() {
    let mut state = initialize_state([Rv32ZcbOnlyInstruction::CSh {
        rs1: Reg::A0,
        rs2: Reg::A1,
        uimm: 0,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 0xDEAD_1234);
    execute(&mut state).unwrap();
    assert_eq!(state.memory.read::<u16>(u64::from(addr)).unwrap(), 0x1234);
}

#[test]
fn test_csh_oob() {
    let mut state = initialize_state([Rv32ZcbOnlyInstruction::CSh {
        rs1: Reg::A0,
        rs2: Reg::A1,
        uimm: 0,
    }]);
    state.regs.write(Reg::A0, 0);
    assert!(matches!(
        execute(&mut state),
        Err(crate::ExecutionError::MemoryAccess(_))
    ));
}

// Unary ops

#[test]
fn test_czext_b() {
    let mut state = initialize_state([Rv32ZcbOnlyInstruction::CZextB {
        rd: Reg::A0,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, 0xDEAD_BE42);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0x42);
}

#[test]
fn test_czext_b_zero() {
    let mut state = initialize_state([Rv32ZcbOnlyInstruction::CZextB {
        rd: Reg::A0,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, 0xFFFF_FF00);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0);
}

#[test]
fn test_csext_b_negative() {
    let mut state = initialize_state([Rv32ZcbOnlyInstruction::CSextB {
        rd: Reg::A0,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, 0xFF);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0xFFFF_FFFF);
}

#[test]
fn test_csext_b_positive() {
    let mut state = initialize_state([Rv32ZcbOnlyInstruction::CSextB {
        rd: Reg::A0,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, 0x7F);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0x7F);
}

#[test]
fn test_czext_h() {
    let mut state = initialize_state([Rv32ZcbOnlyInstruction::CZextH {
        rd: Reg::A0,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, 0xDEAD_1234);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0x1234);
}

#[test]
fn test_csext_h_negative() {
    let mut state = initialize_state([Rv32ZcbOnlyInstruction::CSextH {
        rd: Reg::A0,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, 0x8000);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0xFFFF_8000);
}

#[test]
fn test_csext_h_positive() {
    let mut state = initialize_state([Rv32ZcbOnlyInstruction::CSextH {
        rd: Reg::A0,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, 0x7FFF);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0x7FFF);
}

#[test]
fn test_cnot() {
    let mut state = initialize_state([Rv32ZcbOnlyInstruction::CNot {
        rd: Reg::A0,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, 0x5555_5555);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0xAAAA_AAAA);
}

#[test]
fn test_cnot_all_zeros() {
    let mut state = initialize_state([Rv32ZcbOnlyInstruction::CNot {
        rd: Reg::A0,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, 0);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), u32::MAX);
}

#[test]
fn test_cmul() {
    let mut state = initialize_state([Rv32ZcbOnlyInstruction::CMul {
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
fn test_cmul_wraps_32bit() {
    let mut state = initialize_state([Rv32ZcbOnlyInstruction::CMul {
        rd: Reg::S0,
        rs2: Reg::S1,
        rs1: Reg::Zero,
    }]);
    state.regs.write(Reg::S0, 0x8000_0000);
    state.regs.write(Reg::S1, 2);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::S0), 0);
}
