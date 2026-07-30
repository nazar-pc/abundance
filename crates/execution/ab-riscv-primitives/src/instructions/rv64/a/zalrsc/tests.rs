use crate::instructions::Instruction;
use crate::instructions::rv64::a::zalrsc::Rv64ZalrscInstruction;
use crate::instructions::test_utils::make_r_type;
use crate::registers::general_purpose::Reg;

const OPCODE_AMO: u8 = 0b010_1111;
const FUNCT3_W: u8 = 0b010;
const FUNCT3_D: u8 = 0b011;

fn funct7(funct5: u8, aq: bool, rl: bool) -> u8 {
    (funct5 << 2) | (u8::from(aq) << 1) | u8::from(rl)
}

#[test]
fn test_lr_w() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_W, 2, 0, funct7(0b00010, false, false));
    let decoded = Rv64ZalrscInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZalrscInstruction::Lr {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            aq: false,
            rl: false,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_lr_w_nonzero_rs2_returns_none() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_W, 2, 3, funct7(0b00010, false, false));
    let decoded = Rv64ZalrscInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_sc_w() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_W, 2, 3, funct7(0b00011, false, false));
    let decoded = Rv64ZalrscInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZalrscInstruction::Sc {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_lr_d() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_D, 2, 0, funct7(0b00010, false, false));
    let decoded = Rv64ZalrscInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZalrscInstruction::LrD {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            aq: false,
            rl: false,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_lr_d_aqrl() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_D, 2, 0, funct7(0b00010, true, true));
    let decoded = Rv64ZalrscInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZalrscInstruction::LrD {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            aq: true,
            rl: true,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_lr_d_nonzero_rs2_returns_none() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_D, 2, 3, funct7(0b00010, false, false));
    let decoded = Rv64ZalrscInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_sc_d() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_D, 2, 3, funct7(0b00011, false, false));
    let decoded = Rv64ZalrscInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZalrscInstruction::ScD {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_sc_d_aq() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_D, 2, 3, funct7(0b00011, true, false));
    let decoded = Rv64ZalrscInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZalrscInstruction::ScD {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: true,
            rl: false,
        })
    );
}

#[test]
fn test_unknown_funct5_returns_none() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_W, 2, 3, funct7(0b00001, false, false));
    let decoded = Rv64ZalrscInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_wrong_opcode_returns_none() {
    let inst = make_r_type(0b011_0011, 1, FUNCT3_W, 2, 0, funct7(0b00010, false, false));
    let decoded = Rv64ZalrscInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_wrong_funct3_returns_none() {
    let inst = make_r_type(OPCODE_AMO, 1, 0b001, 2, 0, funct7(0b00010, false, false));
    let decoded = Rv64ZalrscInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}
