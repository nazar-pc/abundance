use crate::instructions::Instruction;
use crate::instructions::rv32::b::zba::Rv32ZbaInstruction;
use crate::instructions::test_utils::make_r_type;
use crate::registers::general_purpose::Reg;

#[test]
fn test_sh1add() {
    let inst = make_r_type(0b0110011, 1, 0b010, 2, 3, 0b0010000);
    let decoded = Rv32ZbaInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbaInstruction::Sh1add {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_sh2add() {
    let inst = make_r_type(0b0110011, 1, 0b100, 2, 3, 0b0010000);
    let decoded = Rv32ZbaInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbaInstruction::Sh2add {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_sh3add() {
    let inst = make_r_type(0b0110011, 1, 0b110, 2, 3, 0b0010000);
    let decoded = Rv32ZbaInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZbaInstruction::Sh3add {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_rv64_only_opcode_returns_none() {
    // OP-32 (0b0111011) is RV64-only; must not decode in RV32 Zba
    let inst = make_r_type(0b0111011, 1, 0b000, 2, 3, 0b0000100);
    let decoded = Rv32ZbaInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(decoded, None);
}
