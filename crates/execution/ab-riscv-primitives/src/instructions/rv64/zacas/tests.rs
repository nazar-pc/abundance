use crate::instructions::Instruction;
use crate::instructions::rv64::zacas::Rv64ZacasInstruction;
use crate::instructions::test_utils::make_r_type;
use crate::registers::general_purpose::Reg;

const OPCODE_AMO: u8 = 0b010_1111;
const FUNCT3_W: u8 = 0b010;
const FUNCT3_D: u8 = 0b011;
const FUNCT3_Q: u8 = 0b100;
const FUNCT5_AMOCAS: u8 = 0b00101;

fn funct7(funct5: u8, aq: bool, rl: bool) -> u8 {
    (funct5 << 2) | (u8::from(aq) << 1) | u8::from(rl)
}

#[test]
fn test_amocas_w() {
    let inst = make_r_type(
        OPCODE_AMO,
        1,
        FUNCT3_W,
        2,
        3,
        funct7(FUNCT5_AMOCAS, false, false),
    );
    let decoded = Rv64ZacasInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZacasInstruction::AmocasW {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_amocas_d() {
    let inst = make_r_type(
        OPCODE_AMO,
        1,
        FUNCT3_D,
        2,
        3,
        funct7(FUNCT5_AMOCAS, false, false),
    );
    let decoded = Rv64ZacasInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZacasInstruction::AmocasD {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_amocas_d_aqrl() {
    let inst = make_r_type(
        OPCODE_AMO,
        1,
        FUNCT3_D,
        2,
        3,
        funct7(FUNCT5_AMOCAS, true, true),
    );
    let decoded = Rv64ZacasInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZacasInstruction::AmocasD {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: true,
            rl: true,
        })
    );
}

#[test]
fn test_amocas_d_odd_registers_allowed_on_rv64() {
    // On RV64, `amocas.d` uses single registers, not pairs, so odd registers are fine
    let inst = make_r_type(
        OPCODE_AMO,
        5,
        FUNCT3_D,
        2,
        7,
        funct7(FUNCT5_AMOCAS, false, false),
    );
    let decoded = Rv64ZacasInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZacasInstruction::AmocasD {
            rd: Reg::T0,
            rs1: Reg::Sp,
            rs2: Reg::T2,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_amocas_q_register_pair() {
    // rd=4 (tp), rs2=6 (t1): both even, valid register pair start
    let inst = make_r_type(
        OPCODE_AMO,
        4,
        FUNCT3_Q,
        2,
        6,
        funct7(FUNCT5_AMOCAS, false, false),
    );
    let decoded = Rv64ZacasInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZacasInstruction::AmocasQ {
            rd: Reg::Tp,
            rs1: Reg::Sp,
            rs2: Reg::T1,
            rd_hi: Reg::T0,
            rs2_hi: Reg::T2,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_amocas_q_odd_rd_reserved() {
    let inst = make_r_type(
        OPCODE_AMO,
        5,
        FUNCT3_Q,
        2,
        6,
        funct7(FUNCT5_AMOCAS, false, false),
    );
    let decoded = Rv64ZacasInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_amocas_q_odd_rs2_reserved() {
    let inst = make_r_type(
        OPCODE_AMO,
        4,
        FUNCT3_Q,
        2,
        7,
        funct7(FUNCT5_AMOCAS, false, false),
    );
    let decoded = Rv64ZacasInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_wrong_funct5_returns_none() {
    // funct5=0b00010 is `lr`, not part of Zaamo or Zacas
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_W, 2, 3, funct7(0b00010, false, false));
    let decoded = Rv64ZacasInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_wrong_opcode_returns_none() {
    let inst = make_r_type(
        0b011_0011,
        1,
        FUNCT3_W,
        2,
        3,
        funct7(FUNCT5_AMOCAS, false, false),
    );
    let decoded = Rv64ZacasInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_amoadd_d_inherited_from_zaamo() {
    // Zacas inherits Zaamo, so plain amoadd.d should also decode
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_D, 2, 3, funct7(0b00000, false, false));
    let decoded = Rv64ZacasInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZacasInstruction::AmoaddD {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}
