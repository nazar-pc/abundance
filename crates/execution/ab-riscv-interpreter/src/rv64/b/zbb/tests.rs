use crate::rv64::test_utils::{execute, initialize_state};
use ab_riscv_primitives::instruction::rv64::b::zbb::Rv64ZbbInstruction;
use ab_riscv_primitives::registers::EReg;

#[test]
fn test_andn() {
    let mut state = initialize_state([Rv64ZbbInstruction::Andn {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 0b11110000);
    state.regs.write(EReg::A1, 0b11001100);

    execute(&mut state).unwrap();

    // 11110000 & ~11001100 = 11110000 & 00110011 = 00110000
    assert_eq!(state.regs.read(EReg::A2), 0b00110000);
}

#[test]
fn test_orn() {
    let mut state = initialize_state([Rv64ZbbInstruction::Orn {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 0b11110000);
    state.regs.write(EReg::A1, 0b11001100);

    execute(&mut state).unwrap();

    // 11110000 | ~11001100 = 11110000 | 00110011 = 11110011
    assert_eq!(state.regs.read(EReg::A2) & 0xFF, 0b11110011);
}

#[test]
fn test_xnor() {
    let mut state = initialize_state([Rv64ZbbInstruction::Xnor {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 0b11110000);
    state.regs.write(EReg::A1, 0b11001100);

    execute(&mut state).unwrap();

    // ~(11110000 ^ 11001100) = ~00111100 = ...11000011
    assert_eq!(state.regs.read(EReg::A2) & 0xFF, 0b11000011);
}

#[test]
fn test_clz() {
    let mut state = initialize_state([Rv64ZbbInstruction::Clz {
        rd: EReg::A2,
        rs1: EReg::A0,
    }]);

    state.regs.write(EReg::A0, 0x0000_0000_0100_0000);

    execute(&mut state).unwrap();

    // 0x0000_0000_0100_0000 has bit 24 set (0x01 in byte position 3)
    // Leading zeros = 64 - 25 = 39
    assert_eq!(state.regs.read(EReg::A2), 39);
}

#[test]
fn test_ctz() {
    let mut state = initialize_state([Rv64ZbbInstruction::Ctz {
        rd: EReg::A2,
        rs1: EReg::A0,
    }]);

    state.regs.write(EReg::A0, 0x0000_1000);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 12);
}

#[test]
fn test_cpop() {
    let mut state = initialize_state([Rv64ZbbInstruction::Cpop {
        rd: EReg::A2,
        rs1: EReg::A0,
    }]);

    state.regs.write(EReg::A0, 0b11010101);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 5);
}

#[test]
fn test_max() {
    let mut state = initialize_state([Rv64ZbbInstruction::Max {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 10);
    state.regs.write(EReg::A1, (-5i64).cast_unsigned());

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 10);
}

#[test]
fn test_min() {
    let mut state = initialize_state([Rv64ZbbInstruction::Min {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 10);
    state.regs.write(EReg::A1, (-5i64).cast_unsigned());

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), (-5i64).cast_unsigned());
}

#[test]
fn test_sext_b() {
    let mut state = initialize_state([Rv64ZbbInstruction::Sextb {
        rd: EReg::A2,
        rs1: EReg::A0,
    }]);

    state.regs.write(EReg::A0, 0xFF);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), (-1i64).cast_unsigned());
}

#[test]
fn test_sext_h() {
    let mut state = initialize_state([Rv64ZbbInstruction::Sexth {
        rd: EReg::A2,
        rs1: EReg::A0,
    }]);

    state.regs.write(EReg::A0, 0xFFFF);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), (-1i64).cast_unsigned());
}

#[test]
fn test_zext_h() {
    let mut state = initialize_state([Rv64ZbbInstruction::Zexth {
        rd: EReg::A2,
        rs1: EReg::A0,
    }]);

    state.regs.write(EReg::A0, 0xFFFF_FFFF_FFFF_FFFFu64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 0xFFFF);
}

#[test]
fn test_rol() {
    let mut state = initialize_state([Rv64ZbbInstruction::Rol {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 0x8000_0000_0000_0001u64);
    state.regs.write(EReg::A1, 1);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 0x0000_0000_0000_0003u64);
}

#[test]
fn test_ror() {
    let mut state = initialize_state([Rv64ZbbInstruction::Ror {
        rd: EReg::A2,
        rs1: EReg::A0,
        rs2: EReg::A1,
    }]);

    state.regs.write(EReg::A0, 0x8000_0000_0000_0001u64);
    state.regs.write(EReg::A1, 1);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 0xC000_0000_0000_0000u64);
}

#[test]
fn test_rori() {
    let mut state = initialize_state([Rv64ZbbInstruction::Rori {
        rd: EReg::A2,
        rs1: EReg::A0,
        shamt: 1,
    }]);

    state.regs.write(EReg::A0, 0x8000_0000_0000_0001u64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 0xC000_0000_0000_0000u64);
}

#[test]
fn test_orc_b() {
    let mut state = initialize_state([Rv64ZbbInstruction::Orcb {
        rd: EReg::A2,
        rs1: EReg::A0,
    }]);

    state.regs.write(EReg::A0, 0x0001_0002_0000_0304u64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 0x00FF_00FF_0000_FFFFu64);
}

#[test]
fn test_rev8() {
    let mut state = initialize_state([Rv64ZbbInstruction::Rev8 {
        rd: EReg::A2,
        rs1: EReg::A0,
    }]);

    state.regs.write(EReg::A0, 0x0123_4567_89AB_CDEFu64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(EReg::A2), 0xEFCD_AB89_6745_2301u64);
}
