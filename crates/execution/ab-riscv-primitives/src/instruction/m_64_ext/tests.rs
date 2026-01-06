use crate::instruction::GenericInstruction;
use crate::instruction::m_64_ext::M64ExtInstruction;
use crate::instruction::test_utils::make_r_type;
use crate::registers::Reg64;

#[test]
fn test_mul() {
    let inst = make_r_type(0b0110011, 1, 0b000, 2, 3, 0b0000001);
    let decoded = M64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(M64ExtInstruction::Mul {
            rd: Reg64::Ra,
            rs1: Reg64::Sp,
            rs2: Reg64::Gp
        })
    );
}

#[test]
fn test_mulh() {
    let inst = make_r_type(0b0110011, 1, 0b001, 2, 3, 0b0000001);
    let decoded = M64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(M64ExtInstruction::Mulh {
            rd: Reg64::Ra,
            rs1: Reg64::Sp,
            rs2: Reg64::Gp
        })
    );
}

#[test]
fn test_mulhsu() {
    let inst = make_r_type(0b0110011, 1, 0b010, 2, 3, 0b0000001);
    let decoded = M64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(M64ExtInstruction::Mulhsu {
            rd: Reg64::Ra,
            rs1: Reg64::Sp,
            rs2: Reg64::Gp
        })
    );
}

#[test]
fn test_mulhu() {
    let inst = make_r_type(0b0110011, 1, 0b011, 2, 3, 0b0000001);
    let decoded = M64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(M64ExtInstruction::Mulhu {
            rd: Reg64::Ra,
            rs1: Reg64::Sp,
            rs2: Reg64::Gp
        })
    );
}

#[test]
fn test_div() {
    let inst = make_r_type(0b0110011, 1, 0b100, 2, 3, 0b0000001);
    let decoded = M64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(M64ExtInstruction::Div {
            rd: Reg64::Ra,
            rs1: Reg64::Sp,
            rs2: Reg64::Gp
        })
    );
}

#[test]
fn test_divu() {
    let inst = make_r_type(0b0110011, 1, 0b101, 2, 3, 0b0000001);
    let decoded = M64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(M64ExtInstruction::Divu {
            rd: Reg64::Ra,
            rs1: Reg64::Sp,
            rs2: Reg64::Gp
        })
    );
}

#[test]
fn test_rem() {
    let inst = make_r_type(0b0110011, 1, 0b110, 2, 3, 0b0000001);
    let decoded = M64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(M64ExtInstruction::Rem {
            rd: Reg64::Ra,
            rs1: Reg64::Sp,
            rs2: Reg64::Gp
        })
    );
}

#[test]
fn test_remu() {
    let inst = make_r_type(0b0110011, 1, 0b111, 2, 3, 0b0000001);
    let decoded = M64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(M64ExtInstruction::Remu {
            rd: Reg64::Ra,
            rs1: Reg64::Sp,
            rs2: Reg64::Gp
        })
    );
}
