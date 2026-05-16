use crate::rv32::test_utils::{TEST_BASE_ADDR, execute, initialize_state};
use crate::{ExecutableInstruction, ExecutionError, ProgramCounter, RegisterFile, VirtualMemory};
use ab_riscv_primitives::prelude::*;

// C.ADDI4SPN

#[test]
fn test_caddi4spn() {
    let mut state = initialize_state([Rv32ZcaInstruction::CAddi4spn {
        rd: Reg::A0,
        nzuimm: 16,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::Sp, 100);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 116);
}

// C.LW

#[test]
fn test_clw_sign_extends_in_rv32() {
    let mut state = initialize_state([Rv32ZcaInstruction::CLw {
        rd: Reg::A1,
        rs1: Reg::A0,
        uimm: 0,
        rs2: Reg::Zero,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state
        .memory
        .write::<u32>(u64::from(addr), 0xDEAD_BEEF)
        .unwrap();
    state.regs.write(Reg::A0, addr);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A1), 0xDEAD_BEEF);
}

#[test]
fn test_clw_with_offset() {
    let mut state = initialize_state([Rv32ZcaInstruction::CLw {
        rd: Reg::A1,
        rs1: Reg::A0,
        uimm: 4,
        rs2: Reg::Zero,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.memory.write::<u32>(u64::from(addr + 4), 42).unwrap();
    state.regs.write(Reg::A0, addr);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A1), 42);
}

// C.SW

#[test]
fn test_csw() {
    let mut state = initialize_state([Rv32ZcaInstruction::CSw {
        rs1: Reg::A0,
        rs2: Reg::A1,
        uimm: 0,
    }]);
    let addr = TEST_BASE_ADDR + 0x100;
    state.regs.write(Reg::A0, addr);
    state.regs.write(Reg::A1, 0xCAFE_BABE);
    execute(&mut state).unwrap();
    assert_eq!(
        state.memory.read::<u32>(u64::from(addr)).unwrap(),
        0xCAFE_BABE
    );
}

// C.JAL (RV32 only)

#[test]
fn test_cjal_links_ra() {
    let mut state = initialize_state([Rv32ZcaInstruction::CJal {
        imm: 4,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    let pc_before = state.instruction_fetcher.get_pc();
    execute(&mut state).unwrap();
    // ra = pc+2 (size of compressed instruction)
    assert_eq!(state.regs.read(Reg::Ra), pc_before + 2);
    // pc = old_pc + 4
    assert_eq!(
        state.instruction_fetcher.get_pc(),
        pc_before.wrapping_add(4u32)
    );
}

#[test]
fn test_cjal_negative_offset() {
    let mut state = initialize_state::<Rv32ZcaInstruction<_>, _>([]);
    let pc_before = state.instruction_fetcher.get_pc();
    // Simulate instruction fetch
    let instruction = {
        let instruction = Rv32ZcaInstruction::CJal {
            imm: -4,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        };
        state
            .instruction_fetcher
            .set_pc(&state.memory, pc_before + instruction.size() as u32)
            .unwrap()
            .continue_ok()
            .unwrap();
        instruction
    };
    let (rd, rd_value) = instruction
        .execute(
            Default::default(),
            &mut state.regs,
            &mut state.ext_state,
            &mut state.memory,
            &mut state.instruction_fetcher,
            &mut state.system_instruction_handler,
        )
        .unwrap()
        .continue_ok()
        .unwrap();
    state.regs.write(rd, rd_value);
    assert_eq!(state.regs.read(Reg::Ra), pc_before + 2);
    assert_eq!(
        state.instruction_fetcher.get_pc(),
        pc_before.wrapping_sub(4u32)
    );
}

// C.NOP

#[test]
fn test_cnop() {
    let mut state = initialize_state([Rv32ZcaInstruction::CNop {
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::Zero), 0);
}

// C.ADDI

#[test]
fn test_caddi() {
    let mut state = initialize_state([Rv32ZcaInstruction::CAddi {
        rd: Reg::A0,
        nzimm: 10,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, 5);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 15);
}

#[test]
fn test_caddi_negative() {
    let mut state = initialize_state([Rv32ZcaInstruction::CAddi {
        rd: Reg::A0,
        nzimm: -3,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, 10);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 7);
}

// C.LI

#[test]
fn test_cli() {
    let mut state = initialize_state([Rv32ZcaInstruction::CLi {
        rd: Reg::A0,
        imm: -5,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), (-5i32).cast_unsigned());
}

// C.ADDI16SP

#[test]
fn test_caddi16sp() {
    let mut state = initialize_state([Rv32ZcaInstruction::CAddi16sp {
        nzimm: 32,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::Sp, 100);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::Sp), 132);
}

#[test]
fn test_caddi16sp_negative() {
    let mut state = initialize_state([Rv32ZcaInstruction::CAddi16sp {
        nzimm: -16,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::Sp, 256);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::Sp), 240);
}

// C.LUI

#[test]
fn test_clui() {
    let mut state = initialize_state([Rv32ZcaInstruction::CLui {
        rd: Reg::A0,
        nzimm: I24::from_i32(0x1000),
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0x1000);
}

// Shifts

#[test]
fn test_csrli() {
    let mut state = initialize_state([Rv32ZcaInstruction::CSrli {
        rd: Reg::S0,
        shamt: 4,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::S0, 0xFFFF_FFFF);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::S0), 0x0FFF_FFFF);
}

#[test]
fn test_csrai_propagates_sign() {
    let mut state = initialize_state([Rv32ZcaInstruction::CSrai {
        rd: Reg::S0,
        shamt: 4,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::S0, 0xFF00_0000);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::S0), 0xFFF0_0000);
}

// C.ANDI

#[test]
fn test_candi() {
    let mut state = initialize_state([Rv32ZcaInstruction::CAndi {
        rd: Reg::S0,
        imm: 0x0F,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::S0, 0xFF);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::S0), 0x0F);
}

// Arithmetic

#[test]
fn test_csub() {
    let mut state = initialize_state([Rv32ZcaInstruction::CSub {
        rd: Reg::S0,
        rs2: Reg::S1,
        rs1: Reg::Zero,
    }]);
    state.regs.write(Reg::S0, 10);
    state.regs.write(Reg::S1, 3);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::S0), 7);
}

#[test]
fn test_csub_wraps() {
    let mut state = initialize_state([Rv32ZcaInstruction::CSub {
        rd: Reg::S0,
        rs2: Reg::S1,
        rs1: Reg::Zero,
    }]);
    state.regs.write(Reg::S0, 0);
    state.regs.write(Reg::S1, 1);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::S0), u32::MAX);
}

#[test]
fn test_cxor() {
    let mut state = initialize_state([Rv32ZcaInstruction::CXor {
        rd: Reg::S0,
        rs2: Reg::S1,
        rs1: Reg::Zero,
    }]);
    state.regs.write(Reg::S0, 0xAAAA_AAAA);
    state.regs.write(Reg::S1, 0x5555_5555);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::S0), 0xFFFF_FFFF);
}

#[test]
fn test_cor() {
    let mut state = initialize_state([Rv32ZcaInstruction::COr {
        rd: Reg::S0,
        rs2: Reg::S1,
        rs1: Reg::Zero,
    }]);
    state.regs.write(Reg::S0, 0xF0);
    state.regs.write(Reg::S1, 0x0F);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::S0), 0xFF);
}

#[test]
fn test_cand() {
    let mut state = initialize_state([Rv32ZcaInstruction::CAnd {
        rd: Reg::S0,
        rs2: Reg::S1,
        rs1: Reg::Zero,
    }]);
    state.regs.write(Reg::S0, 0xFF);
    state.regs.write(Reg::S1, 0x0F);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::S0), 0x0F);
}

// Branches

#[test]
fn test_cj() {
    let mut state = initialize_state([Rv32ZcaInstruction::CJ {
        imm: 8,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    let pc_before = state.instruction_fetcher.get_pc();
    execute(&mut state).unwrap();
    assert_eq!(
        state.instruction_fetcher.get_pc(),
        pc_before.wrapping_add(8u32)
    );
}

#[test]
fn test_cbeqz_taken() {
    let mut state = initialize_state([Rv32ZcaInstruction::CBeqz {
        rs1: Reg::S0,
        imm: 8,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::S0, 0);
    let pc_before = state.instruction_fetcher.get_pc();
    execute(&mut state).unwrap();
    assert_eq!(
        state.instruction_fetcher.get_pc(),
        pc_before.wrapping_add(8u32)
    );
}

#[test]
fn test_cbeqz_not_taken() {
    let mut state = initialize_state([
        Rv32ZcaInstruction::CBeqz {
            rs1: Reg::S0,
            imm: 100,
            rs2: Reg::Zero,
        },
        Rv32ZcaInstruction::CAddi {
            rd: Reg::A0,
            nzimm: 42,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        },
    ]);
    state.regs.write(Reg::S0, 1);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 42);
}

#[test]
fn test_cbnez_taken() {
    let mut state = initialize_state([Rv32ZcaInstruction::CBnez {
        rs1: Reg::S0,
        imm: 4,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::S0, 99);
    let pc_before = state.instruction_fetcher.get_pc();
    execute(&mut state).unwrap();
    assert_eq!(
        state.instruction_fetcher.get_pc(),
        pc_before.wrapping_add(4u32)
    );
}

// Q10

#[test]
fn test_cslli() {
    let mut state = initialize_state([Rv32ZcaInstruction::CSlli {
        rd: Reg::A0,
        shamt: 3,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, 1);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 8);
}

#[test]
fn test_clwsp() {
    let mut state = initialize_state([Rv32ZcaInstruction::CLwsp {
        rd: Reg::A0,
        uimm: 0,
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);
    let sp_addr = TEST_BASE_ADDR + 0x200;
    state
        .memory
        .write::<u32>(u64::from(sp_addr), 0xCAFE_0001)
        .unwrap();
    state.regs.write(Reg::Sp, sp_addr);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0xCAFE_0001);
}

// Register ops

#[test]
fn test_cjr() {
    let mut state = initialize_state([Rv32ZcaInstruction::CJr {
        rs1: Reg::A0,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, TEST_BASE_ADDR + 0x100);
    execute(&mut state).unwrap();
    assert_eq!(state.instruction_fetcher.get_pc(), TEST_BASE_ADDR + 0x100);
}

#[test]
fn test_cjr_clears_lsb() {
    let mut state = initialize_state([Rv32ZcaInstruction::CJr {
        rs1: Reg::A0,
        rs2: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, TEST_BASE_ADDR + 0x101);
    execute(&mut state).unwrap();
    assert_eq!(state.instruction_fetcher.get_pc(), TEST_BASE_ADDR + 0x100);
}

#[test]
fn test_cmv() {
    let mut state = initialize_state([Rv32ZcaInstruction::CMv {
        rd: Reg::A0,
        rs2: Reg::A1,
        rs1: Reg::Zero,
    }]);
    state.regs.write(Reg::A1, 99);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 99);
}

#[test]
fn test_cjalr() {
    let mut state = initialize_state([Rv32ZcaInstruction::CJalr {
        rs1: Reg::A0,
        rs2: Reg::Zero,
    }]);
    let initial_pc = state.instruction_fetcher.get_pc();
    state.regs.write(Reg::A0, TEST_BASE_ADDR + 0x100);
    execute(&mut state).unwrap();
    // Return address = PC after the instruction (initial_pc + 2)
    assert_eq!(state.regs.read(Reg::Ra), initial_pc + 2);
    assert_eq!(state.instruction_fetcher.get_pc(), TEST_BASE_ADDR + 0x100);
}

#[test]
fn test_cadd() {
    let mut state = initialize_state([Rv32ZcaInstruction::CAdd {
        rd: Reg::A0,
        rs2: Reg::A1,
        rs1: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, 100);
    state.regs.write(Reg::A1, 200);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 300);
}

#[test]
fn test_cadd_wraps() {
    let mut state = initialize_state([Rv32ZcaInstruction::CAdd {
        rd: Reg::A0,
        rs2: Reg::A1,
        rs1: Reg::Zero,
    }]);
    state.regs.write(Reg::A0, u32::MAX);
    state.regs.write(Reg::A1, 1);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A0), 0);
}

#[test]
fn test_cswsp() {
    let mut state = initialize_state([Rv32ZcaInstruction::CSwsp {
        rs2: Reg::A0,
        uimm: 0,
        rs1: Reg::Zero,
    }]);
    let sp_addr = TEST_BASE_ADDR + 0x200;
    state.regs.write(Reg::Sp, sp_addr);
    state.regs.write(Reg::A0, 0x1234_5678);
    execute(&mut state).unwrap();
    assert_eq!(
        state.memory.read::<u32>(u64::from(sp_addr)).unwrap(),
        0x1234_5678
    );
}

#[test]
fn test_cunimp() {
    let mut state = initialize_state([Rv32ZcaInstruction::CUnimp {
        rs1: Reg::Zero,
        rs2: Reg::Zero,
    }]);

    let result = execute(&mut state);

    assert!(matches!(
        result,
        Err(ExecutionError::IllegalInstruction { .. })
    ));
}

// Memory errors

#[test]
fn test_clw_oob() {
    let mut state = initialize_state([Rv32ZcaInstruction::CLw {
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
