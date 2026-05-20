use crate::RegisterFile;
use crate::rv64::test_utils::{execute, initialize_state};
use ab_riscv_primitives::prelude::*;

#[test]
fn test_andn() {
    let mut state = initialize_state([Rv64ZbbInstruction::Andn {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0b1111_0000);
    state.regs.write(Reg::A1, 0b1100_1100);

    execute(&mut state).unwrap();

    // 1111_0000 & ~1100_1100 = 1111_0000 & 0011_0011 = 0011_0000
    assert_eq!(state.regs.read(Reg::A2), 0b0011_0000);
}

#[test]
fn test_orn() {
    let mut state = initialize_state([Rv64ZbbInstruction::Orn {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0b1111_0000);
    state.regs.write(Reg::A1, 0b1100_1100);

    execute(&mut state).unwrap();

    // 1111_0000 | ~1100_1100 = 1111_0000 | 0011_0011 = 1111_0011
    assert_eq!(state.regs.read(Reg::A2) & 0xFF, 0b1111_0011);
}

#[test]
fn test_xnor() {
    let mut state = initialize_state([Rv64ZbbInstruction::Xnor {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0b1111_0000);
    state.regs.write(Reg::A1, 0b1100_1100);

    execute(&mut state).unwrap();

    // ~(1111_0000 ^ 1100_1100) = ~0011_1100 = ...1100_0011
    assert_eq!(state.regs.read(Reg::A2) & 0xFF, 0b1100_0011);
}

#[test]
fn test_clz() {
    let mut state = initialize_state([Rv64ZbbInstruction::Clz {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::Zero,
    }]);

    state.regs.write(Reg::A0, 0x0000_0000_0100_0000);

    execute(&mut state).unwrap();

    // 0x0000_0000_0100_0000 has bit 24 set (0x01 in byte position 3)
    // Leading zeros = 64 - 25 = 39
    assert_eq!(state.regs.read(Reg::A2), 39);
}

#[test]
fn test_ctz() {
    let mut state = initialize_state([Rv64ZbbInstruction::Ctz {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::Zero,
    }]);

    state.regs.write(Reg::A0, 0x0000_1000);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 12);
}

#[test]
fn test_cpop() {
    let mut state = initialize_state([Rv64ZbbInstruction::Cpop {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::Zero,
    }]);

    state.regs.write(Reg::A0, 0b1101_0101);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 5);
}

#[test]
fn test_max() {
    let mut state = initialize_state([Rv64ZbbInstruction::Max {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 10);
    state.regs.write(Reg::A1, (-5i64).cast_unsigned());

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 10);
}

#[test]
fn test_min() {
    let mut state = initialize_state([Rv64ZbbInstruction::Min {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 10);
    state.regs.write(Reg::A1, (-5i64).cast_unsigned());

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), (-5i64).cast_unsigned());
}

#[test]
fn test_sext_b() {
    let mut state = initialize_state([Rv64ZbbInstruction::Sextb {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::Zero,
    }]);

    state.regs.write(Reg::A0, 0xFF);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), (-1i64).cast_unsigned());
}

#[test]
fn test_sext_h() {
    let mut state = initialize_state([Rv64ZbbInstruction::Sexth {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::Zero,
    }]);

    state.regs.write(Reg::A0, 0xFFFF);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), (-1i64).cast_unsigned());
}

#[test]
fn test_zext_h() {
    let mut state = initialize_state([Rv64ZbbInstruction::Zexth {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::Zero,
    }]);

    state.regs.write(Reg::A0, 0xFFFF_FFFF_FFFF_FFFF);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xFFFF);
}

#[test]
fn test_rol() {
    let mut state = initialize_state([Rv64ZbbInstruction::Rol {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0x8000_0000_0000_0001);
    state.regs.write(Reg::A1, 1);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0x0000_0000_0000_0003);
}

#[test]
fn test_ror() {
    let mut state = initialize_state([Rv64ZbbInstruction::Ror {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);

    state.regs.write(Reg::A0, 0x8000_0000_0000_0001);
    state.regs.write(Reg::A1, 1);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xC000_0000_0000_0000);
}

#[test]
fn test_rori() {
    let mut state = initialize_state([Rv64ZbbInstruction::Rori {
        rd: Reg::A2,
        rs1: Reg::A0,
        shamt: 1,
        rs2: Reg::Zero,
    }]);

    state.regs.write(Reg::A0, 0x8000_0000_0000_0001);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xC000_0000_0000_0000);
}

#[test]
fn test_orc_b() {
    let mut state = initialize_state([Rv64ZbbInstruction::Orcb {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::Zero,
    }]);

    state.regs.write(Reg::A0, 0x0001_0002_0000_0304);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0x00FF_00FF_0000_FFFF);
}

#[test]
fn test_rev8() {
    let mut state = initialize_state([Rv64ZbbInstruction::Rev8 {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::Zero,
    }]);

    state.regs.write(Reg::A0, 0x0123_4567_89AB_CDEF);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 0xEFCD_AB89_6745_2301);
}
