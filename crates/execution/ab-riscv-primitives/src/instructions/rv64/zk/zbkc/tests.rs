use crate::instructions::Instruction;
use crate::instructions::rv64::b::zbc::Rv64ZbcInstruction;
use crate::instructions::rv64::zk::zbkc::Rv64ZbkcInstruction;
use crate::instructions::test_utils::make_r_type;
use crate::registers::general_purpose::Reg;

#[test]
fn test_clmul() {
    let inst = make_r_type(0b011_0011, 1, 0b001, 2, 3, 0b000_0101);
    assert_eq!(
        Rv64ZbkcInstruction::<Reg<u64>>::try_decode(inst),
        Some(Rv64ZbkcInstruction::Clmul {
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
        Rv64ZbkcInstruction::<Reg<u64>>::try_decode(inst),
        Some(Rv64ZbkcInstruction::Clmulh {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
        })
    );
}

#[test]
fn test_clmulr_rejected() {
    let inst = make_r_type(0b011_0011, 1, 0b010, 2, 3, 0b000_0101);
    assert_eq!(Rv64ZbkcInstruction::<Reg<u64>>::try_decode(inst), None);
    // Zbc inherits Zbkc and adds clmulr on top
    assert_eq!(
        Rv64ZbcInstruction::<Reg<u64>>::try_decode(inst),
        Some(Rv64ZbcInstruction::Clmulr {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
        })
    );
}
