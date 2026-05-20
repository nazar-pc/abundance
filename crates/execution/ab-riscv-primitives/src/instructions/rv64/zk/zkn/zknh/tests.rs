use crate::instructions::Instruction;
use crate::instructions::rv64::zk::zkn::zknh::Rv64ZknhInstruction;
use crate::instructions::test_utils::make_r_type;
use crate::registers::general_purpose::Reg;

#[test]
fn test_sha256sig0() {
    let inst = make_r_type(0b001_0011, 1, 0b001, 2, 0b0_0010, 0b000_1000);
    let decoded = Rv64ZknhInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZknhInstruction::Sha256Sig0 {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_sha256sig1() {
    let inst = make_r_type(0b001_0011, 1, 0b001, 2, 0b0_0011, 0b000_1000);
    let decoded = Rv64ZknhInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZknhInstruction::Sha256Sig1 {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_sha256sum0() {
    let inst = make_r_type(0b001_0011, 1, 0b001, 2, 0b0_0000, 0b000_1000);
    let decoded = Rv64ZknhInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZknhInstruction::Sha256Sum0 {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_sha256sum1() {
    let inst = make_r_type(0b001_0011, 1, 0b001, 2, 0b0_0001, 0b000_1000);
    let decoded = Rv64ZknhInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZknhInstruction::Sha256Sum1 {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_sha512sig0() {
    let inst = make_r_type(0b001_0011, 1, 0b001, 2, 0b0_0110, 0b000_1000);
    let decoded = Rv64ZknhInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZknhInstruction::Sha512Sig0 {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_sha512sig1() {
    let inst = make_r_type(0b001_0011, 1, 0b001, 2, 0b0_0111, 0b000_1000);
    let decoded = Rv64ZknhInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZknhInstruction::Sha512Sig1 {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_sha512sum0() {
    let inst = make_r_type(0b001_0011, 1, 0b001, 2, 0b0_0100, 0b000_1000);
    let decoded = Rv64ZknhInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZknhInstruction::Sha512Sum0 {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_sha512sum1() {
    let inst = make_r_type(0b001_0011, 1, 0b001, 2, 0b0_0101, 0b000_1000);
    let decoded = Rv64ZknhInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZknhInstruction::Sha512Sum1 {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
}
