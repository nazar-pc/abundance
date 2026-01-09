use crate::instruction::Instruction;
use crate::instruction::m_64_ext::M64ExtInstruction;
use crate::instruction::test_utils::make_r_type;
use crate::registers::Reg;

#[test]
fn test_mul() {
    let inst = make_r_type(0b0110011, 1, 0b000, 2, 3, 0b0000001);
    let decoded = M64ExtInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(M64ExtInstruction::Mul {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_mulh() {
    let inst = make_r_type(0b0110011, 1, 0b001, 2, 3, 0b0000001);
    let decoded = M64ExtInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(M64ExtInstruction::Mulh {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_mulhsu() {
    let inst = make_r_type(0b0110011, 1, 0b010, 2, 3, 0b0000001);
    let decoded = M64ExtInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(M64ExtInstruction::Mulhsu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_mulhu() {
    let inst = make_r_type(0b0110011, 1, 0b011, 2, 3, 0b0000001);
    let decoded = M64ExtInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(M64ExtInstruction::Mulhu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_div() {
    let inst = make_r_type(0b0110011, 1, 0b100, 2, 3, 0b0000001);
    let decoded = M64ExtInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(M64ExtInstruction::Div {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_divu() {
    let inst = make_r_type(0b0110011, 1, 0b101, 2, 3, 0b0000001);
    let decoded = M64ExtInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(M64ExtInstruction::Divu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_rem() {
    let inst = make_r_type(0b0110011, 1, 0b110, 2, 3, 0b0000001);
    let decoded = M64ExtInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(M64ExtInstruction::Rem {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_remu() {
    let inst = make_r_type(0b0110011, 1, 0b111, 2, 3, 0b0000001);
    let decoded = M64ExtInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(M64ExtInstruction::Remu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_mulw() {
    let inst = make_r_type(0b0111011, 1, 0b000, 2, 3, 0b0000001);
    let decoded = M64ExtInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(M64ExtInstruction::Mulw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_divw() {
    let inst = make_r_type(0b0111011, 1, 0b100, 2, 3, 0b0000001);
    let decoded = M64ExtInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(M64ExtInstruction::Divw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_divuw() {
    let inst = make_r_type(0b0111011, 1, 0b101, 2, 3, 0b0000001);
    let decoded = M64ExtInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(M64ExtInstruction::Divuw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_remw() {
    let inst = make_r_type(0b0111011, 1, 0b110, 2, 3, 0b0000001);
    let decoded = M64ExtInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(M64ExtInstruction::Remw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_remuw() {
    let inst = make_r_type(0b0111011, 1, 0b111, 2, 3, 0b0000001);
    let decoded = M64ExtInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(M64ExtInstruction::Remuw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}
