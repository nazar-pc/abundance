#![expect(clippy::unusual_byte_groupings, reason = "Test readability")]

use crate::instructions::Instruction;
use crate::instructions::rv32::b::zbb::Rv32ZbbInstruction;
use crate::instructions::test_utils::{make_i_type_with_shamt, make_r_type};
use crate::registers::general_purpose::Reg;

#[test]
fn test_andn() {
    let inst = make_r_type(0b011_0011, 1, 0b111, 2, 3, 0b010_0000);
    let decoded = Rv32ZbbInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbbInstruction::Andn {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_orn() {
    let inst = make_r_type(0b011_0011, 1, 0b110, 2, 3, 0b010_0000);
    let decoded = Rv32ZbbInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbbInstruction::Orn {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_xnor() {
    let inst = make_r_type(0b011_0011, 1, 0b100, 2, 3, 0b010_0000);
    let decoded = Rv32ZbbInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbbInstruction::Xnor {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_clz() {
    // RV32: funct7=011_0000, rs2=0, funct3=001, opcode=OP-IMM
    let inst = make_r_type(0b001_0011, 1, 0b001, 2, 0, 0b011_0000);
    let decoded = Rv32ZbbInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbbInstruction::Clz {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_ctz() {
    // RV32: funct7=011_0000, rs2=1, funct3=001
    let inst = make_r_type(0b001_0011, 1, 0b001, 2, 1, 0b011_0000);
    let decoded = Rv32ZbbInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbbInstruction::Ctz {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_cpop() {
    // RV32: funct7=011_0000, rs2=2, funct3=001
    let inst = make_r_type(0b001_0011, 1, 0b001, 2, 2, 0b011_0000);
    let decoded = Rv32ZbbInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbbInstruction::Cpop {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_sext_b() {
    // RV32: funct7=011_0000, rs2=4, funct3=001
    let inst = make_r_type(0b001_0011, 1, 0b001, 2, 4, 0b011_0000);
    let decoded = Rv32ZbbInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbbInstruction::Sextb {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_sext_h() {
    // RV32: funct7=011_0000, rs2=5, funct3=001
    let inst = make_r_type(0b001_0011, 1, 0b001, 2, 5, 0b011_0000);
    let decoded = Rv32ZbbInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbbInstruction::Sexth {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_min() {
    let inst = make_r_type(0b011_0011, 1, 0b100, 2, 3, 0b000_0101);
    let decoded = Rv32ZbbInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbbInstruction::Min {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_minu() {
    let inst = make_r_type(0b011_0011, 1, 0b101, 2, 3, 0b000_0101);
    let decoded = Rv32ZbbInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbbInstruction::Minu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_max() {
    let inst = make_r_type(0b011_0011, 1, 0b110, 2, 3, 0b000_0101);
    let decoded = Rv32ZbbInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbbInstruction::Max {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_maxu() {
    let inst = make_r_type(0b011_0011, 1, 0b111, 2, 3, 0b000_0101);
    let decoded = Rv32ZbbInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbbInstruction::Maxu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_rol() {
    let inst = make_r_type(0b011_0011, 1, 0b001, 2, 3, 0b011_0000);
    let decoded = Rv32ZbbInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbbInstruction::Rol {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_ror() {
    let inst = make_r_type(0b011_0011, 1, 0b101, 2, 3, 0b011_0000);
    let decoded = Rv32ZbbInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbbInstruction::Ror {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_rori() {
    // RV32 rori: funct6=01_1000, shamt=5 bits
    let inst = make_i_type_with_shamt(0b001_0011, 1, 0b101, 2, 5, 0b01_1000);
    let decoded = Rv32ZbbInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbbInstruction::Rori {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            shamt: 5,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_zext_h() {
    // RV32 zext.h: OP (0b011_0011), funct3=100, funct7=000_0100, rs2=0
    let inst = make_r_type(0b011_0011, 1, 0b100, 2, 0, 0b000_0100);
    let decoded = Rv32ZbbInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbbInstruction::Zexth {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_zext_h_nonzero_rs2_returns_none() {
    let inst = make_r_type(0b011_0011, 1, 0b100, 2, 1, 0b000_0100);
    let decoded = Rv32ZbbInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_orc_b() {
    // orc.b: funct12=0b0010_1000_0111
    let inst = make_i_type_with_shamt(0b001_0011, 1, 0b101, 2, 0b00_0111, 0b00_1010);
    let decoded = Rv32ZbbInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbbInstruction::Orcb {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_rev8() {
    // RV32 rev8: funct12=0b011010011000
    let inst = 0b011010011000_00010_101_00001_0010011u32;
    let decoded = Rv32ZbbInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbbInstruction::Rev8 {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_rv64_only_opcodes_return_none() {
    // OP-IMM-32 (0b0011011) is RV64-only
    let inst = make_r_type(0b001_1011, 1, 0b001, 2, 0, 0b011_0000);
    assert_eq!(Rv32ZbbInstruction::<Reg<u32>>::try_decode(inst), None);

    // OP-32 (0b0111011) is RV64-only
    let inst = make_r_type(0b011_1011, 1, 0b001, 2, 3, 0b011_0000);
    assert_eq!(Rv32ZbbInstruction::<Reg<u32>>::try_decode(inst), None);
}
