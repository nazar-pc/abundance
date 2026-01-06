use crate::instruction::GenericInstruction;
use crate::instruction::m_ext::MExtInstruction;
use crate::instruction::test_utils::make_r_type;
use crate::registers::Reg;

#[test]
fn test_mul() {
    let inst = make_r_type(0b0110011, 1, 0b000, 2, 3, 0b0000001);
    let decoded = MExtInstruction::<Reg>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(MExtInstruction::Mul {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_mulh() {
    let inst = make_r_type(0b0110011, 1, 0b001, 2, 3, 0b0000001);
    let decoded = MExtInstruction::<Reg>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(MExtInstruction::Mulh {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_mulhsu() {
    let inst = make_r_type(0b0110011, 1, 0b010, 2, 3, 0b0000001);
    let decoded = MExtInstruction::<Reg>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(MExtInstruction::Mulhsu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_mulhu() {
    let inst = make_r_type(0b0110011, 1, 0b011, 2, 3, 0b0000001);
    let decoded = MExtInstruction::<Reg>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(MExtInstruction::Mulhu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_div() {
    let inst = make_r_type(0b0110011, 1, 0b100, 2, 3, 0b0000001);
    let decoded = MExtInstruction::<Reg>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(MExtInstruction::Div {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_divu() {
    let inst = make_r_type(0b0110011, 1, 0b101, 2, 3, 0b0000001);
    let decoded = MExtInstruction::<Reg>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(MExtInstruction::Divu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_rem() {
    let inst = make_r_type(0b0110011, 1, 0b110, 2, 3, 0b0000001);
    let decoded = MExtInstruction::<Reg>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(MExtInstruction::Rem {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_remu() {
    let inst = make_r_type(0b0110011, 1, 0b111, 2, 3, 0b0000001);
    let decoded = MExtInstruction::<Reg>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(MExtInstruction::Remu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}
