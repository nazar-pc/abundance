use crate::instructions::Instruction;
use crate::instructions::rv32::b::zbc::Rv32ZbcInstruction;
use crate::instructions::rv32::zk::zbkc::Rv32ZbkcInstruction;
use crate::instructions::test_utils::make_r_type;
use crate::registers::general_purpose::Reg;

#[test]
fn test_clmul() {
    let inst = make_r_type(0b011_0011, 1, 0b001, 2, 3, 0b000_0101);
    assert_eq!(
        Rv32ZbkcInstruction::<Reg<u32>>::try_decode(inst),
        Some(Rv32ZbkcInstruction::Clmul {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
        })
    );
}

#[test]
fn test_clmulh() {
    let inst = make_r_type(0b011_0011, 1, 0b011, 2, 3, 0b000_0101);
    assert_eq!(
        Rv32ZbkcInstruction::<Reg<u32>>::try_decode(inst),
        Some(Rv32ZbkcInstruction::Clmulh {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
        })
    );
}

#[test]
fn test_clmulr_rejected() {
    let inst = make_r_type(0b011_0011, 1, 0b010, 2, 3, 0b000_0101);
    assert_eq!(Rv32ZbkcInstruction::<Reg<u32>>::try_decode(inst), None);
    // Zbc inherits Zbkc and adds clmulr on top
    assert_eq!(
        Rv32ZbcInstruction::<Reg<u32>>::try_decode(inst),
        Some(Rv32ZbcInstruction::Clmulr {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
        })
    );
}
