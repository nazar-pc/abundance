use crate::instructions::Instruction;
use crate::instructions::rv32::zk::zkn::zknh::Rv32ZknhInstruction;
use crate::instructions::test_utils::make_r_type;
use crate::registers::general_purpose::Reg;

// SHA-256 (I-type, identical encoding to RV64)

#[test]
fn test_sha256sig0() {
    let inst = make_r_type(0b001_0011, 1, 0b001, 2, 0b0_0010, 0b000_1000);
    let decoded = Rv32ZknhInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZknhInstruction::Sha256Sig0 {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_sha256sig1() {
    let inst = make_r_type(0b001_0011, 1, 0b001, 2, 0b0_0011, 0b000_1000);
    let decoded = Rv32ZknhInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZknhInstruction::Sha256Sig1 {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_sha256sum0() {
    let inst = make_r_type(0b001_0011, 1, 0b001, 2, 0b0_0000, 0b000_1000);
    let decoded = Rv32ZknhInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZknhInstruction::Sha256Sum0 {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_sha256sum1() {
    let inst = make_r_type(0b001_0011, 1, 0b001, 2, 0b0_0001, 0b000_1000);
    let decoded = Rv32ZknhInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZknhInstruction::Sha256Sum1 {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Zero,
        })
    );
}

// SHA-512 (R-type, RV32-only two-register instructions)

#[test]
fn test_sha512sig1h() {
    // funct7 = 0b0101111 = 47
    let inst = make_r_type(0b011_0011, 1, 0b000, 2, 3, 0b010_1111);
    let decoded = Rv32ZknhInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZknhInstruction::Sha512Sig1h {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_sha512sig1l() {
    // funct7 = 0b0101011 = 43
    let inst = make_r_type(0b011_0011, 1, 0b000, 2, 3, 0b010_1011);
    let decoded = Rv32ZknhInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZknhInstruction::Sha512Sig1l {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_sha512sum0r() {
    // funct7 = 0b0101000 = 40
    let inst = make_r_type(0b011_0011, 1, 0b000, 2, 3, 0b010_1000);
    let decoded = Rv32ZknhInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZknhInstruction::Sha512Sum0r {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_sha512sum1r() {
    // funct7 = 0b0101001 = 41
    let inst = make_r_type(0b011_0011, 1, 0b000, 2, 3, 0b010_1001);
    let decoded = Rv32ZknhInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZknhInstruction::Sha512Sum1r {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_sha512sig0h() {
    // funct7 = 0b0101110 = 46
    let inst = make_r_type(0b011_0011, 1, 0b000, 2, 3, 0b010_1110);
    let decoded = Rv32ZknhInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZknhInstruction::Sha512Sig0h {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_sha512sig0l() {
    // funct7 = 0b0101010 = 42
    let inst = make_r_type(0b011_0011, 1, 0b000, 2, 3, 0b010_1010);
    let decoded = Rv32ZknhInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv32ZknhInstruction::Sha512Sig0l {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

// Negative tests

#[test]
fn test_wrong_funct3_sha256() {
    // SHA-256 requires funct3 = 0b001; 0b000 must not decode
    let inst = make_r_type(0b001_0011, 1, 0b000, 2, 0b0_0010, 0b000_1000);
    assert_eq!(Rv32ZknhInstruction::<Reg<u32>>::try_decode(inst), None);
}

#[test]
fn test_wrong_funct3_sha512() {
    // SHA-512 R-type requires funct3 = 0b000; 0b001 must not decode
    let inst = make_r_type(0b011_0011, 1, 0b001, 2, 3, 0b010_1110);
    assert_eq!(Rv32ZknhInstruction::<Reg<u32>>::try_decode(inst), None);
}

#[test]
fn test_unknown_funct7_sha512() {
    // funct7 = 0b010_0000 = 32 is not assigned to any SHA-512 RV32 instruction
    let inst = make_r_type(0b011_0011, 1, 0b000, 2, 3, 0b010_0000);
    assert_eq!(Rv32ZknhInstruction::<Reg<u32>>::try_decode(inst), None);
}
