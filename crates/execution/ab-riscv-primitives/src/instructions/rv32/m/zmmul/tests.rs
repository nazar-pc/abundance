use crate::instructions::Instruction;
use crate::instructions::rv32::m::Rv32MInstruction;
use crate::instructions::rv32::m::zmmul::Rv32ZmmulInstruction;
use crate::instructions::test_utils::make_r_type;
use crate::registers::general_purpose::Reg;

#[test]
fn test_multiplication() {
    for (funct3, expected) in [
        (
            0b000,
            Rv32ZmmulInstruction::Mul {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                rs2: Reg::Gp,
            },
        ),
        (
            0b001,
            Rv32ZmmulInstruction::Mulh {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                rs2: Reg::Gp,
            },
        ),
        (
            0b010,
            Rv32ZmmulInstruction::Mulhsu {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                rs2: Reg::Gp,
            },
        ),
        (
            0b011,
            Rv32ZmmulInstruction::Mulhu {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                rs2: Reg::Gp,
            },
        ),
    ] {
        let inst = make_r_type(0b011_0011, 1, funct3, 2, 3, 0b000_0001);
        assert_eq!(
            Rv32ZmmulInstruction::<Reg<u32>>::try_decode(inst),
            Some(expected)
        );
    }
}

#[test]
fn test_division_rejected() {
    for funct3 in [0b100, 0b101, 0b110, 0b111] {
        let inst = make_r_type(0b011_0011, 1, funct3, 2, 3, 0b000_0001);
        assert_eq!(Rv32ZmmulInstruction::<Reg<u32>>::try_decode(inst), None);
        // M inherits Zmmul and adds division on top
        assert!(Rv32MInstruction::<Reg<u32>>::try_decode(inst).is_some());
    }
}
