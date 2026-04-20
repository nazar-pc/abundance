use crate::rv64::test_utils::{TEST_BASE_ADDR, execute, initialize_state};
use crate::{ExecutionError, ProgramCounter, VirtualMemory};
use ab_riscv_primitives::prelude::*;

// C.ADDI4SPN

#[test]
fn test_caddi4spn() {
    let mut state = initialize_state([Rv64ZcaInstruction::CAddi4spn {
        rd: Reg::A0,
        nzuimm: 16,
    }]);
    state.regs.write(Reg::Sp, 100);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 116);
}

// C.LW / C.SW

#[test]
fn test_clw_sign_extends() {
    let mut state = initialize_state([Rv64ZcaInstruction::CLw {
        rd: Reg::A1,
        rs1: Reg::A0,
        uimm: 0,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<i32>(addr, -42).unwrap();
    state.regs.write(Reg::A0, addr);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A1), (-42i64).cast_unsigned());
}

#[test]
fn test_csw() {
    let mut state = initialize_state([Rv64ZcaInstruction::CSw {
        rs1: Reg::A0,
        rs2: Reg::A1,
        uimm: 4,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 0xDEAD_BEEF);
    execute(&mut state).unwrap();
    assert_eq!(state.memory.read::<u32>(addr + 4).unwrap(), 0xDEAD_BEEF);
}

// C.LD / C.SD

#[test]
fn test_cld() {
    let mut state = initialize_state([Rv64ZcaInstruction::CLd {
        rd: Reg::A1,
        rs1: Reg::A0,
        uimm: 0,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state
        .memory
        .write::<u64>(addr, 0xDEAD_BEEF_CAFE_1234)
        .unwrap();
    state.regs.write(Reg::A0, addr);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A1), 0xDEAD_BEEF_CAFE_1234);
}

#[test]
fn test_csd() {
    let mut state = initialize_state([Rv64ZcaInstruction::CSd {
        rs1: Reg::A0,
        rs2: Reg::A1,
        uimm: 8,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 0x1234_5678_9ABC_DEF0);
    execute(&mut state).unwrap();
    assert_eq!(
        state.memory.read::<u64>(addr + 8).unwrap(),
        0x1234_5678_9ABC_DEF0
    );
}

// C.NOP

#[test]
fn test_cnop() {
    let mut state = initialize_state([Rv64ZcaInstruction::CNop]);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::Zero), 0);
}

// C.ADDI

#[test]
fn test_caddi_positive() {
    let mut state = initialize_state([Rv64ZcaInstruction::CAddi {
        rd: Reg::A0,
        nzimm: 5,
    }]);
    state.regs.write(Reg::A0, 10);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 15);
}

#[test]
fn test_caddi_negative() {
    let mut state = initialize_state([Rv64ZcaInstruction::CAddi {
        rd: Reg::A0,
        nzimm: -1,
    }]);
    state.regs.write(Reg::A0, 1);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0);
}

// C.ADDIW

#[test]
fn test_caddiw_wraps_to_32bit() {
    // 0x7FFFFFFF + 1 overflows in 32-bit, sign-extends to negative 64-bit
    let mut state = initialize_state([Rv64ZcaInstruction::CAddiw {
        rd: Reg::A0,
        imm: 1,
    }]);
    state.regs.write(Reg::A0, 0x7FFF_FFFF);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0xFFFF_FFFF_8000_0000);
}

// C.LI

#[test]
fn test_cli() {
    let mut state = initialize_state([Rv64ZcaInstruction::CLi {
        rd: Reg::A0,
        imm: -7,
    }]);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), (-7i64).cast_unsigned());
}

// C.ADDI16SP

#[test]
fn test_caddi16sp_positive() {
    let mut state = initialize_state([Rv64ZcaInstruction::CAddi16sp { nzimm: 64 }]);
    state.regs.write(Reg::Sp, 256);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::Sp), 320);
}

#[test]
fn test_caddi16sp_negative() {
    let mut state = initialize_state([Rv64ZcaInstruction::CAddi16sp { nzimm: -32 }]);
    state.regs.write(Reg::Sp, 256);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::Sp), 224);
}

// C.LUI

#[test]
fn test_clui() {
    let mut state = initialize_state([Rv64ZcaInstruction::CLui {
        rd: Reg::A0,
        nzimm: 0x1000,
    }]);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0x1000);
}

// C.SRLI / C.SRAI

#[test]
fn test_csrli() {
    let mut state = initialize_state([Rv64ZcaInstruction::CSrli {
        rd: Reg::S0,
        shamt: 4,
    }]);
    state.regs.write(Reg::S0, 0xFFFF_FFFF_FFFF_FFFF);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::S0), 0x0FFF_FFFF_FFFF_FFFF);
}

#[test]
fn test_csrai() {
    let mut state = initialize_state([Rv64ZcaInstruction::CSrai {
        rd: Reg::S0,
        shamt: 4,
    }]);
    state.regs.write(Reg::S0, 0xFFFF_FFFF_FFFF_F000);
    execute(&mut state).unwrap();
    // Arithmetic: sign bit propagated
    assert_eq!(state.regs.read(Reg::S0), 0xFFFF_FFFF_FFFF_FF00);
}

// C.ANDI

#[test]
fn test_candi_mask() {
    let mut state = initialize_state([Rv64ZcaInstruction::CAndi {
        rd: Reg::S0,
        imm: 0x0F,
    }]);
    state.regs.write(Reg::S0, 0xFF);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::S0), 0x0F);
}

#[test]
fn test_candi_negative_sign_extends() {
    // imm=-1 sign-extends to 0xFFFFFFFFFFFFFFFF
    let mut state = initialize_state([Rv64ZcaInstruction::CAndi {
        rd: Reg::S0,
        imm: -1,
    }]);
    state.regs.write(Reg::S0, 0xDEAD_BEEF);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::S0), 0xDEAD_BEEF);
}

// Arithmetic

#[test]
fn test_csub() {
    let mut state = initialize_state([Rv64ZcaInstruction::CSub {
        rd: Reg::S0,
        rs2: Reg::S1,
    }]);
    state.regs.write(Reg::S0, 10);
    state.regs.write(Reg::S1, 3);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::S0), 7);
}

#[test]
fn test_cxor() {
    let mut state = initialize_state([Rv64ZcaInstruction::CXor {
        rd: Reg::S0,
        rs2: Reg::S1,
    }]);
    state.regs.write(Reg::S0, 0b1010);
    state.regs.write(Reg::S1, 0b1100);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::S0), 0b0110);
}

#[test]
fn test_cor() {
    let mut state = initialize_state([Rv64ZcaInstruction::COr {
        rd: Reg::S0,
        rs2: Reg::S1,
    }]);
    state.regs.write(Reg::S0, 0b1010);
    state.regs.write(Reg::S1, 0b0101);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::S0), 0b1111);
}

#[test]
fn test_cand() {
    let mut state = initialize_state([Rv64ZcaInstruction::CAnd {
        rd: Reg::S0,
        rs2: Reg::S1,
    }]);
    state.regs.write(Reg::S0, 0b1010);
    state.regs.write(Reg::S1, 0b1100);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::S0), 0b1000);
}

#[test]
fn test_csubw_sign_extends() {
    let mut state = initialize_state([Rv64ZcaInstruction::CSubw {
        rd: Reg::S0,
        rs2: Reg::S1,
    }]);
    state.regs.write(Reg::S0, 1);
    state.regs.write(Reg::S1, 2);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::S0), 0xFFFF_FFFF_FFFF_FFFF);
}

#[test]
fn test_caddw_sign_extends() {
    let mut state = initialize_state([Rv64ZcaInstruction::CAddw {
        rd: Reg::S0,
        rs2: Reg::S1,
    }]);
    state.regs.write(Reg::S0, 0x7FFF_FFFF);
    state.regs.write(Reg::S1, 1);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::S0), 0xFFFF_FFFF_8000_0000);
}

// Branches

#[test]
fn test_cj() {
    let mut state = initialize_state([Rv64ZcaInstruction::CJ { imm: 4 }]);
    let initial_pc = state.instruction_fetcher.get_pc();
    execute(&mut state).unwrap();
    assert_eq!(
        state.instruction_fetcher.get_pc(),
        initial_pc.wrapping_add(4)
    );
}

#[test]
fn test_cbeqz_taken() {
    let mut state = initialize_state([Rv64ZcaInstruction::CBeqz {
        rs1: Reg::S0,
        imm: 8,
    }]);
    state.regs.write(Reg::S0, 0);
    let initial_pc = state.instruction_fetcher.get_pc();
    execute(&mut state).unwrap();
    assert_eq!(
        state.instruction_fetcher.get_pc(),
        initial_pc.wrapping_add(8)
    );
}

#[test]
fn test_cbeqz_not_taken() {
    let mut state = initialize_state([
        Rv64ZcaInstruction::CBeqz {
            rs1: Reg::S0,
            imm: 8,
        },
        Rv64ZcaInstruction::CAddi {
            rd: Reg::A0,
            nzimm: 42,
        },
    ]);
    state.regs.write(Reg::S0, 1);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 42);
}

#[test]
fn test_cbnez_taken() {
    let mut state = initialize_state([Rv64ZcaInstruction::CBnez {
        rs1: Reg::S0,
        imm: 4,
    }]);
    state.regs.write(Reg::S0, 99);
    let initial_pc = state.instruction_fetcher.get_pc();
    execute(&mut state).unwrap();
    assert_eq!(
        state.instruction_fetcher.get_pc(),
        initial_pc.wrapping_add(4)
    );
}

// Stack pointer loads

#[test]
fn test_clwsp() {
    let mut state = initialize_state([Rv64ZcaInstruction::CLwsp {
        rd: Reg::A0,
        uimm: 0,
    }]);
    let sp_addr = TEST_BASE_ADDR + 0x200;
    state.memory.write::<i32>(sp_addr, -100).unwrap();
    state.regs.write(Reg::Sp, sp_addr);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), (-100i64).cast_unsigned());
}

#[test]
fn test_cldsp() {
    let mut state = initialize_state([Rv64ZcaInstruction::CLdsp {
        rd: Reg::A0,
        uimm: 16,
    }]);
    let sp_addr = TEST_BASE_ADDR + 0x200;
    state
        .memory
        .write::<u64>(sp_addr + 16, 0xCAFE_BABE)
        .unwrap();
    state.regs.write(Reg::Sp, sp_addr);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0xCAFE_BABE);
}

// Register ops

#[test]
fn test_cjr() {
    let mut state = initialize_state([Rv64ZcaInstruction::CJr { rs1: Reg::A0 }]);
    state.regs.write(Reg::A0, TEST_BASE_ADDR + 0x100);
    execute(&mut state).unwrap();
    assert_eq!(state.instruction_fetcher.get_pc(), TEST_BASE_ADDR + 0x100);
}

#[test]
fn test_cjr_clears_lsb() {
    let mut state = initialize_state([Rv64ZcaInstruction::CJr { rs1: Reg::A0 }]);
    state.regs.write(Reg::A0, TEST_BASE_ADDR + 0x101);
    execute(&mut state).unwrap();
    assert_eq!(state.instruction_fetcher.get_pc(), TEST_BASE_ADDR + 0x100);
}

#[test]
fn test_cmv() {
    let mut state = initialize_state([Rv64ZcaInstruction::CMv {
        rd: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A1, 42);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 42);
}

#[test]
fn test_cjalr() {
    let mut state = initialize_state([Rv64ZcaInstruction::CJalr { rs1: Reg::A0 }]);
    let initial_pc = state.instruction_fetcher.get_pc();
    state.regs.write(Reg::A0, TEST_BASE_ADDR + 0x100);
    execute(&mut state).unwrap();
    // Return address = PC after the instruction (initial_pc + 2)
    assert_eq!(state.regs.read(Reg::Ra), initial_pc + 2);
    assert_eq!(state.instruction_fetcher.get_pc(), TEST_BASE_ADDR + 0x100);
}

#[test]
fn test_cadd() {
    let mut state = initialize_state([Rv64ZcaInstruction::CAdd {
        rd: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A0, 10);
    state.regs.write(Reg::A1, 20);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 30);
}

#[test]
fn test_cslli() {
    let mut state = initialize_state([Rv64ZcaInstruction::CSlli {
        rd: Reg::A0,
        shamt: 3,
    }]);
    state.regs.write(Reg::A0, 1);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 8);
}

// Stack pointer stores

#[test]
fn test_cswsp() {
    let mut state = initialize_state([Rv64ZcaInstruction::CSwsp {
        rs2: Reg::A0,
        uimm: 0,
    }]);
    let sp_addr = TEST_BASE_ADDR + 0x200;
    state.regs.write(Reg::Sp, sp_addr);
    state.regs.write(Reg::A0, 0xDEAD_BEEF);
    execute(&mut state).unwrap();
    assert_eq!(state.memory.read::<u32>(sp_addr).unwrap(), 0xDEAD_BEEF);
}

#[test]
fn test_csdsp() {
    let mut state = initialize_state([Rv64ZcaInstruction::CSdsp {
        rs2: Reg::A0,
        uimm: 8,
    }]);
    let sp_addr = TEST_BASE_ADDR + 0x200;
    state.regs.write(Reg::Sp, sp_addr);
    state.regs.write(Reg::A0, 0xDEAD_BEEF_CAFE_0000);
    execute(&mut state).unwrap();
    assert_eq!(
        state.memory.read::<u64>(sp_addr + 8).unwrap(),
        0xDEAD_BEEF_CAFE_0000
    );
}

// Write-to-zero protection

#[test]
fn test_write_to_zero_via_addi() {
    // C.ADDI with rd=Zero should not change x0
    // (In compressed format rd=Zero with ADDI is C.NOP; we test via regular path)
    let mut state = initialize_state([Rv64ZcaInstruction::CNop]);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::Zero), 0);
}

// Memory errors

#[test]
fn test_clw_out_of_bounds() {
    let mut state = initialize_state([Rv64ZcaInstruction::CLw {
        rd: Reg::A1,
        rs1: Reg::A0,
        uimm: 0,
    }]);
    state.regs.write(Reg::A0, 0);
    let result = execute(&mut state);
    assert!(matches!(result, Err(ExecutionError::MemoryAccess(_))));
}

#[test]
fn test_csd_out_of_bounds() {
    let mut state = initialize_state([Rv64ZcaInstruction::CSd {
        rs1: Reg::A0,
        rs2: Reg::A1,
        uimm: 0,
    }]);
    state.regs.write(Reg::A0, 0);
    let result = execute(&mut state);
    assert!(matches!(result, Err(ExecutionError::MemoryAccess(_))));
}

#[test]
fn test_cunimp() {
    let mut state = initialize_state([Rv64ZcaInstruction::CUnimp]);

    let result = execute(&mut state);

    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}
