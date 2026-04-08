use crate::instructions::Instruction;
use crate::instructions::test_utils::make_r_type;
use crate::instructions::zicond::ZicondInstruction;
use crate::registers::general_purpose::Reg;

// opcode = 0x33 (OP), funct7 = 0x07 for both Zicond instructions
// czero.eqz: funct3 = 0b101
// czero.nez: funct3 = 0b111

#[test]
fn test_czero_eqz() {
    // czero.eqz a2, a0, a1
    let inst = make_r_type(0b011_0011, 12, 0b101, 10, 11, 0b000_0111);
    let decoded = ZicondInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZicondInstruction::CzeroEqz {
            rd: Reg::A2,
            rs1: Reg::A0,
            rs2: Reg::A1,
        })
    );
}

#[test]
fn test_czero_nez() {
    // czero.nez a2, a0, a1
    let inst = make_r_type(0b011_0011, 12, 0b111, 10, 11, 0b000_0111);
    let decoded = ZicondInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZicondInstruction::CzeroNez {
            rd: Reg::A2,
            rs1: Reg::A0,
            rs2: Reg::A1,
        })
    );
}

#[test]
fn test_czero_eqz_zero_registers() {
    // czero.eqz x0, x0, x0
    let inst = make_r_type(0b011_0011, 0, 0b101, 0, 0, 0b000_0111);
    let decoded = ZicondInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZicondInstruction::CzeroEqz {
            rd: Reg::Zero,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_czero_nez_zero_registers() {
    // czero.nez x0, x0, x0
    let inst = make_r_type(0b011_0011, 0, 0b111, 0, 0, 0b000_0111);
    let decoded = ZicondInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZicondInstruction::CzeroNez {
            rd: Reg::Zero,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_invalid_opcode() {
    // Wrong opcode (0x13 instead of 0x33)
    let inst = make_r_type(0b001_0011, 12, 0b101, 10, 11, 0b000_0111);
    let decoded = ZicondInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_invalid_funct7() {
    // Correct opcode/funct3 but wrong funct7 (0x00 instead of 0x07)
    let inst = make_r_type(0b011_0011, 12, 0b101, 10, 11, 0b000_0000);
    let decoded = ZicondInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_invalid_funct3() {
    // Correct opcode and funct7 but funct3 that is neither 0b101 nor 0b111
    let inst = make_r_type(0b011_0011, 12, 0b001, 10, 11, 0b000_0111);
    let decoded = ZicondInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}
