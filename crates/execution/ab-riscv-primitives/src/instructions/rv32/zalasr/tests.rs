use crate::instructions::Instruction;
use crate::instructions::rv32::zalasr::Rv32ZalasrInstruction;
use crate::instructions::test_utils::make_r_type;
use crate::registers::general_purpose::Reg;

const OPCODE_AMO: u8 = 0b010_1111;
const FUNCT5_LOAD: u8 = 0b00110;
const FUNCT5_STORE: u8 = 0b00111;

fn funct7(funct5: u8, aq: bool, rl: bool) -> u8 {
    (funct5 << 2) | (u8::from(aq) << 1) | u8::from(rl)
}

#[test]
fn test_lb_aq() {
    let inst = make_r_type(OPCODE_AMO, 1, 0b000, 2, 0, funct7(FUNCT5_LOAD, true, false));
    let decoded = Rv32ZalasrInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZalasrInstruction::LbAq {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rl: false,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_lh_aq() {
    let inst = make_r_type(OPCODE_AMO, 1, 0b001, 2, 0, funct7(FUNCT5_LOAD, true, false));
    let decoded = Rv32ZalasrInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZalasrInstruction::LhAq {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rl: false,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_lw_aqrl() {
    let inst = make_r_type(OPCODE_AMO, 1, 0b010, 2, 0, funct7(FUNCT5_LOAD, true, true));
    let decoded = Rv32ZalasrInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZalasrInstruction::LwAq {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rl: true,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_load_without_aq_bit_reserved() {
    // `aq` is mandatory for load-acquire; without it the encoding is reserved
    let inst = make_r_type(
        OPCODE_AMO,
        1,
        0b010,
        2,
        0,
        funct7(FUNCT5_LOAD, false, false),
    );
    let decoded = Rv32ZalasrInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_load_nonzero_rs2_reserved() {
    let inst = make_r_type(OPCODE_AMO, 1, 0b010, 2, 3, funct7(FUNCT5_LOAD, true, false));
    let decoded = Rv32ZalasrInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_sb_rl() {
    let inst = make_r_type(
        OPCODE_AMO,
        0,
        0b000,
        2,
        3,
        funct7(FUNCT5_STORE, false, true),
    );
    let decoded = Rv32ZalasrInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZalasrInstruction::SbRl {
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
        })
    );
}

#[test]
fn test_sh_rl() {
    let inst = make_r_type(
        OPCODE_AMO,
        0,
        0b001,
        2,
        3,
        funct7(FUNCT5_STORE, false, true),
    );
    let decoded = Rv32ZalasrInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZalasrInstruction::ShRl {
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
        })
    );
}

#[test]
fn test_sw_aqrl() {
    let inst = make_r_type(OPCODE_AMO, 0, 0b010, 2, 3, funct7(FUNCT5_STORE, true, true));
    let decoded = Rv32ZalasrInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZalasrInstruction::SwRl {
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: true,
        })
    );
}

#[test]
fn test_store_without_rl_bit_reserved() {
    // `rl` is mandatory for store-release; without it the encoding is reserved
    let inst = make_r_type(
        OPCODE_AMO,
        0,
        0b010,
        2,
        3,
        funct7(FUNCT5_STORE, false, false),
    );
    let decoded = Rv32ZalasrInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_store_nonzero_rd_reserved() {
    let inst = make_r_type(
        OPCODE_AMO,
        1,
        0b010,
        2,
        3,
        funct7(FUNCT5_STORE, false, true),
    );
    let decoded = Rv32ZalasrInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_wrong_opcode_returns_none() {
    let inst = make_r_type(0b011_0011, 1, 0b010, 2, 0, funct7(FUNCT5_LOAD, true, false));
    let decoded = Rv32ZalasrInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_doubleword_size_not_available_on_rv32() {
    let inst = make_r_type(OPCODE_AMO, 1, 0b011, 2, 0, funct7(FUNCT5_LOAD, true, false));
    let decoded = Rv32ZalasrInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(decoded, None);
}
