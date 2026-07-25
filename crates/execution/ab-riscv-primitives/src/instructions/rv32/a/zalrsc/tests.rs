use crate::instructions::Instruction;
use crate::instructions::rv32::a::zalrsc::Rv32ZalrscInstruction;
use crate::instructions::test_utils::make_r_type;
use crate::registers::general_purpose::Reg;

const OPCODE_AMO: u8 = 0b010_1111;
const FUNCT3_W: u8 = 0b010;

fn funct7(funct5: u8, aq: bool, rl: bool) -> u8 {
    (funct5 << 2) | (u8::from(aq) << 1) | u8::from(rl)
}

#[test]
fn test_lr_w() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_W, 2, 0, funct7(0b00010, false, false));
    let decoded = Rv32ZalrscInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZalrscInstruction::Lr {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            aq: false,
            rl: false,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_lr_w_aqrl() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_W, 2, 0, funct7(0b00010, true, true));
    let decoded = Rv32ZalrscInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZalrscInstruction::Lr {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            aq: true,
            rl: true,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_lr_w_nonzero_rs2_returns_none() {
    // `rs2` must be zero for `lr`, otherwise the encoding is reserved
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_W, 2, 3, funct7(0b00010, false, false));
    let decoded = Rv32ZalrscInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_sc_w() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_W, 2, 3, funct7(0b00011, false, false));
    let decoded = Rv32ZalrscInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZalrscInstruction::Sc {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

#[test]
fn test_sc_w_aq() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_W, 2, 3, funct7(0b00011, true, false));
    let decoded = Rv32ZalrscInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZalrscInstruction::Sc {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: true,
            rl: false,
        })
    );
}

#[test]
fn test_sc_w_rl() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_W, 2, 3, funct7(0b00011, false, true));
    let decoded = Rv32ZalrscInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZalrscInstruction::Sc {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: true,
        })
    );
}

#[test]
fn test_unknown_funct5_returns_none() {
    let inst = make_r_type(OPCODE_AMO, 1, FUNCT3_W, 2, 3, funct7(0b00001, false, false));
    let decoded = Rv32ZalrscInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_wrong_opcode_returns_none() {
    let inst = make_r_type(0b011_0011, 1, FUNCT3_W, 2, 0, funct7(0b00010, false, false));
    let decoded = Rv32ZalrscInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_wrong_funct3_returns_none() {
    let inst = make_r_type(OPCODE_AMO, 1, 0b011, 2, 0, funct7(0b00010, false, false));
    let decoded = Rv32ZalrscInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(decoded, None);
}
