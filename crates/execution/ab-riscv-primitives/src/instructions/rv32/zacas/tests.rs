use crate::instructions::Instruction;
use crate::instructions::rv32::zacas::Rv32ZacasInstruction;
use crate::instructions::test_utils::make_r_type;
use crate::registers::general_purpose::Reg;

const OPCODE_AMO: u8 = 0b010_1111;
const FUNCT3_W: u8 = 0b010;
const FUNCT3_D: u8 = 0b011;
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
    let decoded = Rv32ZacasInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZacasInstruction::AmocasW {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_amocas_w_aqrl() {
    let inst = make_r_type(
        OPCODE_AMO,
        1,
        FUNCT3_W,
        2,
        3,
        funct7(FUNCT5_AMOCAS, true, true),
    );
    let decoded = Rv32ZacasInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZacasInstruction::AmocasW {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: true,
            rl: true,
        })
    );
}

#[test]
fn test_amocas_d_register_pair() {
    // rd=4 (t1), rs2=6 (t2): both even, valid register pair start
    let inst = make_r_type(
        OPCODE_AMO,
        4,
        FUNCT3_D,
        2,
        6,
        funct7(FUNCT5_AMOCAS, false, false),
    );
    let decoded = Rv32ZacasInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZacasInstruction::AmocasD {
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
fn test_amocas_d_odd_rd_reserved() {
    let inst = make_r_type(
        OPCODE_AMO,
        5,
        FUNCT3_D,
        2,
        6,
        funct7(FUNCT5_AMOCAS, false, false),
    );
    let decoded = Rv32ZacasInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_amocas_d_odd_rs2_reserved() {
    let inst = make_r_type(
        OPCODE_AMO,
        4,
        FUNCT3_D,
        2,
        7,
        funct7(FUNCT5_AMOCAS, false, false),
    );
    let decoded = Rv32ZacasInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_wrong_funct5_returns_none() {
    // funct5=0b00010 is `lr`, not part of Zaamo or Zacas
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_W, 2, 3, funct7(0b00010, false, false));
    let decoded = Rv32ZacasInstruction::<Reg<u32>>::try_decode(inst);
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
    let decoded = Rv32ZacasInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_amoadd_inherited_from_zaamo() {
    // Zacas inherits Zaamo, so plain amoadd.w should also decode
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_W, 2, 3, funct7(0b00000, false, false));
    let decoded = Rv32ZacasInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZacasInstruction::Amoadd {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}
