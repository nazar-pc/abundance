use crate::rv32::test_utils::{execute, initialize_state};
use ab_riscv_primitives::prelude::*;

#[test]
fn test_andn() {
    let mut state = initialize_state([Rv32ZbbInstruction::Andn {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0b11110000u32);
    state.regs.write(Reg::A1, 0b11001100u32);

    execute(&mut state).unwrap();

    // 11110000 & ~11001100 = 11110000 & 00110011 = 00110000
    assert_eq!(state.regs.read(Reg::A2), 0b00110000);
}

#[test]
fn test_orn() {
    let mut state = initialize_state([Rv32ZbbInstruction::Orn {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0b11110000u32);
    state.regs.write(Reg::A1, 0b11001100u32);

    execute(&mut state).unwrap();

    // 11110000 | ~11001100 = 11110000 | 00110011 = 11110011
    assert_eq!(state.regs.read(Reg::A2) & 0xFF, 0b11110011);
}

#[test]
fn test_xnor() {
    let mut state = initialize_state([Rv32ZbbInstruction::Xnor {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0b11110000u32);
    state.regs.write(Reg::A1, 0b11001100u32);

    execute(&mut state).unwrap();

    // ~(11110000 ^ 11001100) = ~00111100 = ...11000011
    assert_eq!(state.regs.read(Reg::A2) & 0xFF, 0b11000011);
}

#[test]
fn test_clz() {
    let mut state = initialize_state([Rv32ZbbInstruction::Clz {
        rd: Reg::A2,
        rs1: Reg::A0,
    }]);

    state.regs.write(Reg::A0, 0x0100_0000u32);

    execute(&mut state).unwrap();

    // 0x0100_0000 has bit 24 set; leading zeros = 32 - 25 = 7
    assert_eq!(state.regs.read(Reg::A2), 7);
}

#[test]
fn test_ctz() {
    let mut state = initialize_state([Rv32ZbbInstruction::Ctz {
        rd: Reg::A2,
        rs1: Reg::A0,
    }]);

    state.regs.write(Reg::A0, 0x0000_1000u32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 12);
}

#[test]
fn test_cpop() {
    let mut state = initialize_state([Rv32ZbbInstruction::Cpop {
        rd: Reg::A2,
        rs1: Reg::A0,
    }]);

    state.regs.write(Reg::A0, 0b11010101u32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 5);
}

#[test]
fn test_max() {
    let mut state = initialize_state([Rv32ZbbInstruction::Max {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 10u32);
    state.regs.write(Reg::A1, (-5i32).cast_unsigned());

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 10);
}

#[test]
fn test_min() {
    let mut state = initialize_state([Rv32ZbbInstruction::Min {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 10u32);
    state.regs.write(Reg::A1, (-5i32).cast_unsigned());

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), (-5i32).cast_unsigned());
}

#[test]
fn test_sext_b() {
    let mut state = initialize_state([Rv32ZbbInstruction::Sextb {
        rd: Reg::A2,
        rs1: Reg::A0,
    }]);

    state.regs.write(Reg::A0, 0xFFu32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), (-1i32).cast_unsigned());
}

#[test]
fn test_sext_h() {
    let mut state = initialize_state([Rv32ZbbInstruction::Sexth {
        rd: Reg::A2,
        rs1: Reg::A0,
    }]);

    state.regs.write(Reg::A0, 0xFFFFu32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), (-1i32).cast_unsigned());
}

#[test]
fn test_zext_h() {
    let mut state = initialize_state([Rv32ZbbInstruction::Zexth {
        rd: Reg::A2,
        rs1: Reg::A0,
    }]);

    state.regs.write(Reg::A0, 0xFFFF_FFFFu32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xFFFF);
}

#[test]
fn test_rol() {
    let mut state = initialize_state([Rv32ZbbInstruction::Rol {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0x8000_0001u32);
    state.regs.write(Reg::A1, 1u32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0x0000_0003u32);
}

#[test]
fn test_ror() {
    let mut state = initialize_state([Rv32ZbbInstruction::Ror {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0x8000_0001u32);
    state.regs.write(Reg::A1, 1u32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xC000_0000u32);
}

#[test]
fn test_rori() {
    let mut state = initialize_state([Rv32ZbbInstruction::Rori {
        rd: Reg::A2,
        rs1: Reg::A0,
        shamt: 1,
    }]);

    state.regs.write(Reg::A0, 0x8000_0001u32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xC000_0000u32);
}

#[test]
fn test_orc_b() {
    let mut state = initialize_state([Rv32ZbbInstruction::Orcb {
        rd: Reg::A2,
        rs1: Reg::A0,
    }]);

    state.regs.write(Reg::A0, 0x0001_0002u32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0x00FF_00FFu32);
}

#[test]
fn test_rev8() {
    let mut state = initialize_state([Rv32ZbbInstruction::Rev8 {
        rd: Reg::A2,
        rs1: Reg::A0,
    }]);

    state.regs.write(Reg::A0, 0x0123_4567u32);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0x6745_2301u32);
}
