use crate::instruction::GenericInstruction;
use crate::instruction::b_64_ext::zbb_64_ext::Zbb64ExtInstruction;
use crate::instruction::test_utils::{make_i_type_with_shamt, make_r_type};
use crate::registers::Reg64;

#[test]
fn test_andn() {
    let inst = make_r_type(0b0110011, 1, 0b111, 2, 3, 0b0100000);
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbb64ExtInstruction::Andn {
            rd: Reg64::Ra,
            rs1: Reg64::Sp,
            rs2: Reg64::Gp
        })
    );
}

#[test]
fn test_orn() {
    let inst = make_r_type(0b0110011, 1, 0b110, 2, 3, 0b0100000);
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbb64ExtInstruction::Orn {
            rd: Reg64::Ra,
            rs1: Reg64::Sp,
            rs2: Reg64::Gp
        })
    );
}

#[test]
fn test_xnor() {
    let inst = make_r_type(0b0110011, 1, 0b100, 2, 3, 0b0100000);
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbb64ExtInstruction::Xnor {
            rd: Reg64::Ra,
            rs1: Reg64::Sp,
            rs2: Reg64::Gp
        })
    );
}

#[test]
fn test_clz() {
    // Current encoding: clz ra, sp (low6=0)
    let inst = make_i_type_with_shamt(0b0010011, 1, 0b001, 2, 0, 0b011000);
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbb64ExtInstruction::Clz {
            rd: Reg64::Ra,
            rs1: Reg64::Sp
        })
    );
}

#[test]
fn test_clz_real_instruction() {
    // clz a0, a0 in current "B" extension
    let inst = 0x60051513_u32;
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbb64ExtInstruction::Clz {
            rd: Reg64::A0,
            rs1: Reg64::A0
        })
    );
}

#[test]
fn test_legacy_clz_reserved_subop() {
    // subop >2 with funct6=011000 in funct3=001 → reserved/None
    let inst = make_i_type_with_shamt(0b0010011, 1, 0b001, 2, 3, 0b011000);
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_reserved_subop_in_legacy_clz_space() {
    // funct6=011000 in funct3=001 with subop/rs2_bits not 0-2 → reserved/None
    // (0-2 are legacy aliases for clz/ctz/cpop)
    let inst = make_i_type_with_shamt(0b0010011, 1, 0b001, 2, 3, 0b011000);
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_old_zbb_clz_now_reserved() {
    let inst = make_r_type(0b0110011, 10, 0b001, 10, 0, 0b0000101);
    assert_eq!(Zbb64ExtInstruction::<Reg64>::try_decode(inst), None);
}

#[test]
fn test_clzw() {
    // Current encoding: clzw ra, sp (rs2=0, funct7=0110000)
    let inst = make_r_type(0b0011011, 1, 0b001, 2, 0, 0b0110000);
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbb64ExtInstruction::Clzw {
            rd: Reg64::Ra,
            rs1: Reg64::Sp
        })
    );
}

#[test]
fn test_ctz() {
    // Current encoding: ctz ra, sp (low6=1)
    let inst = make_i_type_with_shamt(0b0010011, 1, 0b001, 2, 1, 0b011000);
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbb64ExtInstruction::Ctz {
            rd: Reg64::Ra,
            rs1: Reg64::Sp
        })
    );
}

#[test]
fn test_ctz_legacy_encoding() {
    // Analogous legacy for ctz (rs2/subop=1)
    let inst = make_i_type_with_shamt(0b0010011, 1, 0b001, 2, 1, 0b011000);
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbb64ExtInstruction::Ctz {
            rd: Reg64::Ra,
            rs1: Reg64::Sp
        })
    );
}

#[test]
fn test_ctzw() {
    // Current encoding: ctzw ra, sp (rs2=1, funct7=0110000)
    let inst = make_r_type(0b0011011, 1, 0b001, 2, 1, 0b0110000);
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbb64ExtInstruction::Ctzw {
            rd: Reg64::Ra,
            rs1: Reg64::Sp
        })
    );
}

#[test]
fn test_cpop() {
    // Current encoding: cpop ra, sp (low6=2)
    let inst = make_i_type_with_shamt(0b0010011, 1, 0b001, 2, 2, 0b011000);
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbb64ExtInstruction::Cpop {
            rd: Reg64::Ra,
            rs1: Reg64::Sp
        })
    );
}

#[test]
fn test_cpop_legacy_encoding() {
    // Legacy for cpop (rs2/subop=2)
    let inst = make_i_type_with_shamt(0b0010011, 1, 0b001, 2, 2, 0b011000);
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbb64ExtInstruction::Cpop {
            rd: Reg64::Ra,
            rs1: Reg64::Sp
        })
    );
}

#[test]
fn test_cpopw() {
    // Current encoding: cpopw ra, sp (rs2=2, funct7=0110000)
    let inst = make_r_type(0b0011011, 1, 0b001, 2, 2, 0b0110000);
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbb64ExtInstruction::Cpopw {
            rd: Reg64::Ra,
            rs1: Reg64::Sp
        })
    );
}

#[test]
fn test_min() {
    let inst = make_r_type(0b0110011, 1, 0b010, 2, 3, 0b0000101);
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbb64ExtInstruction::Min {
            rd: Reg64::Ra,
            rs1: Reg64::Sp,
            rs2: Reg64::Gp
        })
    );
}

#[test]
fn test_max() {
    let inst = make_r_type(0b0110011, 1, 0b100, 2, 3, 0b0000101);
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbb64ExtInstruction::Max {
            rd: Reg64::Ra,
            rs1: Reg64::Sp,
            rs2: Reg64::Gp
        })
    );
}

#[test]
fn test_minu() {
    let inst = make_r_type(0b0110011, 1, 0b011, 2, 3, 0b0000101);
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbb64ExtInstruction::Minu {
            rd: Reg64::Ra,
            rs1: Reg64::Sp,
            rs2: Reg64::Gp
        })
    );
}

#[test]
fn test_maxu() {
    let inst = make_r_type(0b0110011, 1, 0b101, 2, 3, 0b0000101);
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbb64ExtInstruction::Maxu {
            rd: Reg64::Ra,
            rs1: Reg64::Sp,
            rs2: Reg64::Gp
        })
    );
}

#[test]
fn test_rol() {
    let inst = make_r_type(0b0110011, 1, 0b001, 2, 3, 0b0110000);
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbb64ExtInstruction::Rol {
            rd: Reg64::Ra,
            rs1: Reg64::Sp,
            rs2: Reg64::Gp
        })
    );
}

#[test]
fn test_rolw() {
    let inst = make_r_type(0b0111011, 1, 0b001, 2, 3, 0b0110000);
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbb64ExtInstruction::Rolw {
            rd: Reg64::Ra,
            rs1: Reg64::Sp,
            rs2: Reg64::Gp
        })
    );
}

#[test]
fn test_ror() {
    let inst = make_r_type(0b0110011, 1, 0b101, 2, 3, 0b0110000);
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbb64ExtInstruction::Ror {
            rd: Reg64::Ra,
            rs1: Reg64::Sp,
            rs2: Reg64::Gp
        })
    );
}

#[test]
fn test_rorw() {
    let inst = make_r_type(0b0111011, 1, 0b101, 2, 3, 0b0110000);
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbb64ExtInstruction::Rorw {
            rd: Reg64::Ra,
            rs1: Reg64::Sp,
            rs2: Reg64::Gp
        })
    );
}

#[test]
fn test_rori() {
    let inst = make_i_type_with_shamt(0b0010011, 1, 0b101, 2, 5, 0b011000);
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbb64ExtInstruction::Rori {
            rd: Reg64::Ra,
            rs1: Reg64::Sp,
            shamt: 5
        })
    );
}

#[test]
fn test_rori_large_shamt() {
    // shamt = 40 = 0b101000
    let inst = make_i_type_with_shamt(0b0010011, 1, 0b101, 2, 40, 0b011000);
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbb64ExtInstruction::Rori {
            rd: Reg64::Ra,
            rs1: Reg64::Sp,
            shamt: 40
        })
    );
}

#[test]
fn test_rori_real_instruction() {
    // Real instruction: rori a1,t1,0xe (0x60e35593)
    let inst = 0x60e35593u32;
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbb64ExtInstruction::Rori {
            rd: Reg64::A1,
            rs1: Reg64::T1,
            shamt: 0xe
        })
    );
}

#[test]
fn test_roriw() {
    let inst = make_i_type_with_shamt(0b0011011, 1, 0b101, 2, 12, 0b011000);
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbb64ExtInstruction::Roriw {
            rd: Reg64::Ra,
            rs1: Reg64::Sp,
            shamt: 12
        })
    );
}

#[test]
fn test_sext_b() {
    #[expect(clippy::unusual_byte_groupings)]
    let inst = 0b011000000100_00010_001_00001_0010011_u32;
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbb64ExtInstruction::Sextb {
            rd: Reg64::Ra,
            rs1: Reg64::Sp
        })
    );
}

#[test]
fn test_sext_h() {
    #[expect(clippy::unusual_byte_groupings)]
    let inst = 0b011000000101_00010_001_00001_0010011_u32;
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbb64ExtInstruction::Sexth {
            rd: Reg64::Ra,
            rs1: Reg64::Sp
        })
    );
}

#[test]
fn test_zext_h() {
    // Ratified encoding: zext.h ra, sp (rd=1, rs1=2, rs2=0, funct3=100, funct7=0000100,
    // opcode=0111011)
    let inst = make_r_type(0b0111011, 1, 0b100, 2, 0, 0b0000100);
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbb64ExtInstruction::Zexth {
            rd: Reg64::Ra,
            rs1: Reg64::Sp
        })
    );
}

#[test]
fn test_zext_h_real_instruction() {
    // zext.h a2, a1 (real encoding from current tools/spec: 0x0805c63b)
    let inst = 0x0805c63b_u32;
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbb64ExtInstruction::Zexth {
            rd: Reg64::A2,
            rs1: Reg64::A1
        })
    );
}

#[test]
fn test_no_zext_h_with_nonzero_rs2() {
    // Same encoding but rs2 != 0 → reserved/invalid for Zbb
    let inst = make_r_type(0b0111011, 1, 0b100, 2, 1, 0b0000100);
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_old_draft_zext_h_now_ror() {
    let inst = make_r_type(0b0110011, 1, 0b101, 2, 0b00100, 0b0110000);
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbb64ExtInstruction::Ror {
            rd: Reg64::Ra,
            rs1: Reg64::Sp,
            rs2: Reg64::Tp
        })
    );
}

#[test]
fn test_rev8() {
    // rev8 is an I-type instruction with funct12 = 0b011010111000
    #[expect(clippy::unusual_byte_groupings)]
    let inst = 0b011010111000_00010_101_00001_0010011u32;
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbb64ExtInstruction::Rev8 {
            rd: Reg64::Ra,
            rs1: Reg64::Sp
        })
    );
}

#[test]
fn test_rev8_real_instruction() {
    // Real instruction: rev8 a3,a3 (0x6b86d693)
    let inst = 0x6b86d693u32;
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbb64ExtInstruction::Rev8 {
            rd: Reg64::A3,
            rs1: Reg64::A3
        })
    );
}

#[test]
fn test_orc_b() {
    let inst = make_r_type(0b0110011, 1, 0b101, 2, 0b00111, 0b0000101);
    let decoded = Zbb64ExtInstruction::<Reg64>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Zbb64ExtInstruction::Orcb {
            rd: Reg64::Ra,
            rs1: Reg64::Sp
        })
    );
}
