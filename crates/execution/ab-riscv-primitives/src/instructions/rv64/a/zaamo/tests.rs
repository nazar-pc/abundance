use crate::instructions::Instruction;
use crate::instructions::rv64::a::zaamo::Rv64ZaamoInstruction;
use crate::instructions::test_utils::make_r_type;
use crate::registers::general_purpose::Reg;

const OPCODE_AMO: u8 = 0b010_1111;
const FUNCT3_W: u8 = 0b010;
const FUNCT3_D: u8 = 0b011;

fn funct7(funct5: u8, aq: bool, rl: bool) -> u8 {
    (funct5 << 2) | (u8::from(aq) << 1) | u8::from(rl)
}

#[test]
fn test_amoswap_w() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_W, 2, 3, funct7(0b00001, false, false));
    let decoded = Rv64ZaamoInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZaamoInstruction::Amoswap {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_amoadd_w() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_W, 2, 3, funct7(0b00000, false, false));
    let decoded = Rv64ZaamoInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZaamoInstruction::Amoadd {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_amoxor_w() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_W, 2, 3, funct7(0b00100, false, false));
    let decoded = Rv64ZaamoInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZaamoInstruction::Amoxor {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_amoand_w() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_W, 2, 3, funct7(0b01100, false, false));
    let decoded = Rv64ZaamoInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZaamoInstruction::Amoand {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_amoor_w() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_W, 2, 3, funct7(0b01000, false, false));
    let decoded = Rv64ZaamoInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZaamoInstruction::Amoor {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_amomin_w() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_W, 2, 3, funct7(0b10000, false, false));
    let decoded = Rv64ZaamoInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZaamoInstruction::Amomin {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_amomax_w() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_W, 2, 3, funct7(0b10100, false, false));
    let decoded = Rv64ZaamoInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZaamoInstruction::Amomax {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_amominu_w() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_W, 2, 3, funct7(0b11000, false, false));
    let decoded = Rv64ZaamoInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZaamoInstruction::Amominu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_amomaxu_w() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_W, 2, 3, funct7(0b11100, false, false));
    let decoded = Rv64ZaamoInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZaamoInstruction::Amomaxu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_amoswap_d() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_D, 2, 3, funct7(0b00001, false, false));
    let decoded = Rv64ZaamoInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZaamoInstruction::AmoswapD {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_amoadd_d() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_D, 2, 3, funct7(0b00000, false, false));
    let decoded = Rv64ZaamoInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZaamoInstruction::AmoaddD {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_amoxor_d() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_D, 2, 3, funct7(0b00100, false, false));
    let decoded = Rv64ZaamoInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZaamoInstruction::AmoxorD {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_amoand_d() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_D, 2, 3, funct7(0b01100, false, false));
    let decoded = Rv64ZaamoInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZaamoInstruction::AmoandD {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_amoor_d() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_D, 2, 3, funct7(0b01000, false, false));
    let decoded = Rv64ZaamoInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZaamoInstruction::AmoorD {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_amomin_d() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_D, 2, 3, funct7(0b10000, false, false));
    let decoded = Rv64ZaamoInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZaamoInstruction::AmominD {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_amomax_d() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_D, 2, 3, funct7(0b10100, false, false));
    let decoded = Rv64ZaamoInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZaamoInstruction::AmomaxD {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_amominu_d() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_D, 2, 3, funct7(0b11000, false, false));
    let decoded = Rv64ZaamoInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZaamoInstruction::AmominuD {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_amomaxu_d() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_D, 2, 3, funct7(0b11100, false, false));
    let decoded = Rv64ZaamoInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZaamoInstruction::AmomaxuD {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_amoadd_w_aqrl() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_W, 2, 3, funct7(0b00000, true, true));
    let decoded = Rv64ZaamoInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZaamoInstruction::Amoadd {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: true,
            rl: true,
        })
    );
}

#[test]
fn test_unknown_funct5_returns_none() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_W, 2, 3, funct7(0b00010, false, false));
    let decoded = Rv64ZaamoInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_wrong_opcode_returns_none() {
    let inst = make_r_type(0b011_0011, 1, FUNCT3_W, 2, 3, funct7(0b00000, false, false));
    let decoded = Rv64ZaamoInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_wrong_funct3_returns_none() {
    let inst = make_r_type(OPCODE_AMO, 1, 0b001, 2, 3, funct7(0b00000, false, false));
    let decoded = Rv64ZaamoInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}
