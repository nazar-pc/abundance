use crate::instructions::Instruction;
use crate::instructions::rv64::m::Rv64MInstruction;
use crate::instructions::test_utils::make_r_type;
use crate::registers::Reg;

#[test]
fn test_mul() {
    let inst = make_r_type(0b0110011, 1, 0b000, 2, 3, 0b0000001);
    let decoded = Rv64MInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64MInstruction::Mul {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_mulh() {
    let inst = make_r_type(0b0110011, 1, 0b001, 2, 3, 0b0000001);
    let decoded = Rv64MInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64MInstruction::Mulh {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_mulhsu() {
    let inst = make_r_type(0b0110011, 1, 0b010, 2, 3, 0b0000001);
    let decoded = Rv64MInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64MInstruction::Mulhsu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_mulhu() {
    let inst = make_r_type(0b0110011, 1, 0b011, 2, 3, 0b0000001);
    let decoded = Rv64MInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64MInstruction::Mulhu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_div() {
    let inst = make_r_type(0b0110011, 1, 0b100, 2, 3, 0b0000001);
    let decoded = Rv64MInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64MInstruction::Div {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_divu() {
    let inst = make_r_type(0b0110011, 1, 0b101, 2, 3, 0b0000001);
    let decoded = Rv64MInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64MInstruction::Divu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_rem() {
    let inst = make_r_type(0b0110011, 1, 0b110, 2, 3, 0b0000001);
    let decoded = Rv64MInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64MInstruction::Rem {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_remu() {
    let inst = make_r_type(0b0110011, 1, 0b111, 2, 3, 0b0000001);
    let decoded = Rv64MInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64MInstruction::Remu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_mulw() {
    let inst = make_r_type(0b0111011, 1, 0b000, 2, 3, 0b0000001);
    let decoded = Rv64MInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64MInstruction::Mulw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_divw() {
    let inst = make_r_type(0b0111011, 1, 0b100, 2, 3, 0b0000001);
    let decoded = Rv64MInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64MInstruction::Divw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_divuw() {
    let inst = make_r_type(0b0111011, 1, 0b101, 2, 3, 0b0000001);
    let decoded = Rv64MInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64MInstruction::Divuw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_remw() {
    let inst = make_r_type(0b0111011, 1, 0b110, 2, 3, 0b0000001);
    let decoded = Rv64MInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64MInstruction::Remw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_remuw() {
    let inst = make_r_type(0b0111011, 1, 0b111, 2, 3, 0b0000001);
    let decoded = Rv64MInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64MInstruction::Remuw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}
