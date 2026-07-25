use crate::instructions::Instruction;
use crate::instructions::rv64::zabha::Rv64ZabhaInstruction;
use crate::instructions::test_utils::make_r_type;
use crate::registers::general_purpose::Reg;

const OPCODE_AMO: u8 = 0b010_1111;
const FUNCT3_B: u8 = 0b000;
const FUNCT3_H: u8 = 0b001;

fn funct7(funct5: u8, aq: bool, rl: bool) -> u8 {
    (funct5 << 2) | (u8::from(aq) << 1) | u8::from(rl)
}

#[test]
fn test_amoswap_b() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_B, 2, 3, funct7(0b00001, false, false));
    let decoded = Rv64ZabhaInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZabhaInstruction::AmoswapB {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_amoadd_h() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_H, 2, 3, funct7(0b00000, false, false));
    let decoded = Rv64ZabhaInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZabhaInstruction::AmoaddH {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_amoxor_b() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_B, 2, 3, funct7(0b00100, false, false));
    let decoded = Rv64ZabhaInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZabhaInstruction::AmoxorB {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_amoand_h() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_H, 2, 3, funct7(0b01100, false, false));
    let decoded = Rv64ZabhaInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZabhaInstruction::AmoandH {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_amoor_b() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_B, 2, 3, funct7(0b01000, false, false));
    let decoded = Rv64ZabhaInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZabhaInstruction::AmoorB {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_amomin_h() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_H, 2, 3, funct7(0b10000, false, false));
    let decoded = Rv64ZabhaInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZabhaInstruction::AmominH {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_amomax_b() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_B, 2, 3, funct7(0b10100, false, false));
    let decoded = Rv64ZabhaInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZabhaInstruction::AmomaxB {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_amominu_h() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_H, 2, 3, funct7(0b11000, false, false));
    let decoded = Rv64ZabhaInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZabhaInstruction::AmominuH {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_amomaxu_b() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_B, 2, 3, funct7(0b11100, false, false));
    let decoded = Rv64ZabhaInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZabhaInstruction::AmomaxuB {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_amocas_b() {
    // Zabha always defines amocas.b/h itself; whether it's pulled in further up the chain
    // depends on whether Zacas is also present there
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_B, 2, 3, funct7(0b00101, false, false));
    let decoded = Rv64ZabhaInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZabhaInstruction::AmocasB {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_amocas_h() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_H, 2, 3, funct7(0b00101, false, false));
    let decoded = Rv64ZabhaInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZabhaInstruction::AmocasH {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_amoadd_w_inherited_from_zaamo() {
    // Zabha inherits Zaamo, so plain word-sized ops should also decode
    let inst = make_r_type(OPCODE_AMO, 1, 0b010, 2, 3, funct7(0b00000, false, false));
    let decoded = Rv64ZabhaInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZabhaInstruction::Amoadd {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_wrong_opcode_returns_none() {
    let inst = make_r_type(0b011_0011, 1, FUNCT3_B, 2, 3, funct7(0b00000, false, false));
    let decoded = Rv64ZabhaInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_unknown_funct5_returns_none() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_B, 2, 3, funct7(0b00010, false, false));
    let decoded = Rv64ZabhaInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}
