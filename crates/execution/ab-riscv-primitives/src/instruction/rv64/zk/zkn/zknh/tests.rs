use crate::instruction::Instruction;
use crate::instruction::rv64::zk::zkn::zknh::Rv64ZknhInstruction;
use crate::instruction::test_utils::make_r_type;
use crate::registers::Reg;

#[test]
fn test_sha256sig0() {
    let inst = make_r_type(0b0010011, 1, 0b001, 2, 0b00010, 0b0001000);
    let decoded = Rv64ZknhInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZknhInstruction::Sha256Sig0 {
            rd: Reg::Ra,
            rs1: Reg::Sp
        })
    );
}

#[test]
fn test_sha256sig1() {
    let inst = make_r_type(0b0010011, 1, 0b001, 2, 0b00011, 0b0001000);
    let decoded = Rv64ZknhInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZknhInstruction::Sha256Sig1 {
            rd: Reg::Ra,
            rs1: Reg::Sp
        })
    );
}

#[test]
fn test_sha256sum0() {
    let inst = make_r_type(0b0010011, 1, 0b001, 2, 0b00000, 0b0001000);
    let decoded = Rv64ZknhInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZknhInstruction::Sha256Sum0 {
            rd: Reg::Ra,
            rs1: Reg::Sp
        })
    );
}

#[test]
fn test_sha256sum1() {
    let inst = make_r_type(0b0010011, 1, 0b001, 2, 0b00001, 0b0001000);
    let decoded = Rv64ZknhInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZknhInstruction::Sha256Sum1 {
            rd: Reg::Ra,
            rs1: Reg::Sp
        })
    );
}

#[test]
fn test_sha512sig0() {
    let inst = make_r_type(0b0010011, 1, 0b001, 2, 0b00110, 0b0001000);
    let decoded = Rv64ZknhInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZknhInstruction::Sha512Sig0 {
            rd: Reg::Ra,
            rs1: Reg::Sp
        })
    );
}

#[test]
fn test_sha512sig1() {
    let inst = make_r_type(0b0010011, 1, 0b001, 2, 0b00111, 0b0001000);
    let decoded = Rv64ZknhInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZknhInstruction::Sha512Sig1 {
            rd: Reg::Ra,
            rs1: Reg::Sp
        })
    );
}

#[test]
fn test_sha512sum0() {
    let inst = make_r_type(0b0010011, 1, 0b001, 2, 0b00100, 0b0001000);
    let decoded = Rv64ZknhInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZknhInstruction::Sha512Sum0 {
            rd: Reg::Ra,
            rs1: Reg::Sp
        })
    );
}

#[test]
fn test_sha512sum1() {
    let inst = make_r_type(0b0010011, 1, 0b001, 2, 0b00101, 0b0001000);
    let decoded = Rv64ZknhInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZknhInstruction::Sha512Sum1 {
            rd: Reg::Ra,
            rs1: Reg::Sp
        })
    );
}
