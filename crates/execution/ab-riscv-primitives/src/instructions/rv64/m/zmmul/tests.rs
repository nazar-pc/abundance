use crate::hart::BasicHart;
use crate::instructions::Instruction;
use crate::instructions::rv64::m::Rv64MInstruction;
use crate::instructions::rv64::m::zmmul::Rv64ZmmulInstruction;
use crate::instructions::test_utils::make_r_type;
use crate::registers::general_purpose::Reg;

#[test]
fn test_multiplication() {
    for (opcode, funct3, expected) in [
        (
            0b011_0011,
            0b000,
            Rv64ZmmulInstruction::Mul {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                rs2: Reg::Gp,
            },
        ),
        (
            0b011_0011,
            0b001,
            Rv64ZmmulInstruction::Mulh {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                rs2: Reg::Gp,
            },
        ),
        (
            0b011_0011,
            0b010,
            Rv64ZmmulInstruction::Mulhsu {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                rs2: Reg::Gp,
            },
        ),
        (
            0b011_0011,
            0b011,
            Rv64ZmmulInstruction::Mulhu {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                rs2: Reg::Gp,
            },
        ),
        (
            0b011_1011,
            0b000,
            Rv64ZmmulInstruction::Mulw {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                rs2: Reg::Gp,
            },
        ),
    ] {
        let inst = make_r_type(opcode, 1, funct3, 2, 3, 0b000_0001);
        assert_eq!(
            Rv64ZmmulInstruction::<BasicHart<Reg<u64>>>::try_decode(inst),
            Some(expected)
        );
    }
}

#[test]
fn test_division_rejected() {
    for opcode in [0b011_0011, 0b011_1011] {
        for funct3 in [0b100, 0b101, 0b110, 0b111] {
            let inst = make_r_type(opcode, 1, funct3, 2, 3, 0b000_0001);
            assert_eq!(
                Rv64ZmmulInstruction::<BasicHart<Reg<u64>>>::try_decode(inst),
                None
            );
            // M inherits Zmmul and adds division on top
            assert!(Rv64MInstruction::<BasicHart<Reg<u64>>>::try_decode(inst).is_some());
        }
    }
}
