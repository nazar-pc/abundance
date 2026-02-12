use crate::instructions::Instruction;
use crate::instructions::rv64::b::zbb::Rv64ZbbInstruction;
use crate::instructions::test_utils::{make_i_type_with_shamt, make_r_type};
use crate::registers::general_purpose::Reg;

#[test]
fn test_andn() {
    let inst = make_r_type(0b0110011, 1, 0b111, 2, 3, 0b0100000);
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbbInstruction::Andn {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_orn() {
    let inst = make_r_type(0b0110011, 1, 0b110, 2, 3, 0b0100000);
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbbInstruction::Orn {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_xnor() {
    let inst = make_r_type(0b0110011, 1, 0b100, 2, 3, 0b0100000);
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbbInstruction::Xnor {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_clz() {
    // Current encoding: clz ra, sp (low6=0)
    let inst = make_i_type_with_shamt(0b0010011, 1, 0b001, 2, 0, 0b011000);
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbbInstruction::Clz {
            rd: Reg::Ra,
            rs1: Reg::Sp
        })
    );
}

#[test]
fn test_clz_real_instruction() {
    // clz a0, a0 in current "B" extension
    let inst = 0x60051513_u32;
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbbInstruction::Clz {
            rd: Reg::A0,
            rs1: Reg::A0
        })
    );
}

#[test]
fn test_legacy_clz_reserved_subop() {
    // subop >2 with funct6=011000 in funct3=001 → reserved/None
    let inst = make_i_type_with_shamt(0b0010011, 1, 0b001, 2, 3, 0b011000);
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_reserved_subop_in_legacy_clz_space() {
    // funct6=011000 in funct3=001 with subop/rs2_bits not 0-2 → reserved/None
    // (0-2 are legacy aliases for clz/ctz/cpop)
    let inst = make_i_type_with_shamt(0b0010011, 1, 0b001, 2, 3, 0b011000);
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_old_zbb_clz_now_reserved() {
    let inst = make_r_type(0b0110011, 10, 0b001, 10, 0, 0b0000101);
    assert_eq!(Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst), None);
}

#[test]
fn test_clzw() {
    // Current encoding: clzw ra, sp (rs2=0, funct7=0110000)
    let inst = make_r_type(0b0011011, 1, 0b001, 2, 0, 0b0110000);
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbbInstruction::Clzw {
            rd: Reg::Ra,
            rs1: Reg::Sp
        })
    );
}

#[test]
fn test_ctz() {
    // Current encoding: ctz ra, sp (low6=1)
    let inst = make_i_type_with_shamt(0b0010011, 1, 0b001, 2, 1, 0b011000);
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbbInstruction::Ctz {
            rd: Reg::Ra,
            rs1: Reg::Sp
        })
    );
}

#[test]
fn test_ctz_legacy_encoding() {
    // Analogous legacy for ctz (rs2/subop=1)
    let inst = make_i_type_with_shamt(0b0010011, 1, 0b001, 2, 1, 0b011000);
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbbInstruction::Ctz {
            rd: Reg::Ra,
            rs1: Reg::Sp
        })
    );
}

#[test]
fn test_ctzw() {
    // Current encoding: ctzw ra, sp (rs2=1, funct7=0110000)
    let inst = make_r_type(0b0011011, 1, 0b001, 2, 1, 0b0110000);
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbbInstruction::Ctzw {
            rd: Reg::Ra,
            rs1: Reg::Sp
        })
    );
}

#[test]
fn test_cpop() {
    // Current encoding: cpop ra, sp (low6=2)
    let inst = make_i_type_with_shamt(0b0010011, 1, 0b001, 2, 2, 0b011000);
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbbInstruction::Cpop {
            rd: Reg::Ra,
            rs1: Reg::Sp
        })
    );
}

#[test]
fn test_cpop_legacy_encoding() {
    // Legacy for cpop (rs2/subop=2)
    let inst = make_i_type_with_shamt(0b0010011, 1, 0b001, 2, 2, 0b011000);
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbbInstruction::Cpop {
            rd: Reg::Ra,
            rs1: Reg::Sp
        })
    );
}

#[test]
fn test_cpopw() {
    // Current encoding: cpopw ra, sp (rs2=2, funct7=0110000)
    let inst = make_r_type(0b0011011, 1, 0b001, 2, 2, 0b0110000);
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbbInstruction::Cpopw {
            rd: Reg::Ra,
            rs1: Reg::Sp
        })
    );
}

#[test]
fn test_min() {
    let inst = make_r_type(0b0110011, 1, 0b010, 2, 3, 0b0000101);
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbbInstruction::Min {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_max() {
    let inst = make_r_type(0b0110011, 1, 0b100, 2, 3, 0b0000101);
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbbInstruction::Max {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_minu() {
    let inst = make_r_type(0b0110011, 1, 0b011, 2, 3, 0b0000101);
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbbInstruction::Minu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_maxu() {
    let inst = make_r_type(0b0110011, 1, 0b101, 2, 3, 0b0000101);
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbbInstruction::Maxu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_rol() {
    let inst = make_r_type(0b0110011, 1, 0b001, 2, 3, 0b0110000);
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbbInstruction::Rol {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_rolw() {
    let inst = make_r_type(0b0111011, 1, 0b001, 2, 3, 0b0110000);
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbbInstruction::Rolw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_ror() {
    let inst = make_r_type(0b0110011, 1, 0b101, 2, 3, 0b0110000);
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbbInstruction::Ror {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_rorw() {
    let inst = make_r_type(0b0111011, 1, 0b101, 2, 3, 0b0110000);
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbbInstruction::Rorw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_rori() {
    let inst = make_i_type_with_shamt(0b0010011, 1, 0b101, 2, 5, 0b011000);
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbbInstruction::Rori {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            shamt: 5
        })
    );
}

#[test]
fn test_rori_large_shamt() {
    // shamt = 40 = 0b101000
    let inst = make_i_type_with_shamt(0b0010011, 1, 0b101, 2, 40, 0b011000);
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbbInstruction::Rori {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            shamt: 40
        })
    );
}

#[test]
fn test_rori_real_instruction() {
    // Real instruction: rori a1,t1,0xe (0x60e35593)
    let inst = 0x60e35593u32;
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbbInstruction::Rori {
            rd: Reg::A1,
            rs1: Reg::T1,
            shamt: 0xe
        })
    );
}

#[test]
fn test_roriw() {
    let inst = make_i_type_with_shamt(0b0011011, 1, 0b101, 2, 12, 0b011000);
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbbInstruction::Roriw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            shamt: 12
        })
    );
}

#[test]
fn test_sext_b() {
    #[expect(clippy::unusual_byte_groupings)]
    let inst = 0b011000000100_00010_001_00001_0010011_u32;
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbbInstruction::Sextb {
            rd: Reg::Ra,
            rs1: Reg::Sp
        })
    );
}

#[test]
fn test_sext_h() {
    #[expect(clippy::unusual_byte_groupings)]
    let inst = 0b011000000101_00010_001_00001_0010011_u32;
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbbInstruction::Sexth {
            rd: Reg::Ra,
            rs1: Reg::Sp
        })
    );
}

#[test]
fn test_zext_h() {
    // Ratified encoding: zext.h ra, sp (rd=1, rs1=2, rs2=0, funct3=100, funct7=0000100,
    // opcode=0111011)
    let inst = make_r_type(0b0111011, 1, 0b100, 2, 0, 0b0000100);
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbbInstruction::Zexth {
            rd: Reg::Ra,
            rs1: Reg::Sp
        })
    );
}

#[test]
fn test_zext_h_real_instruction() {
    // zext.h a2, a1 (real encoding from current tools/spec: 0x0805c63b)
    let inst = 0x0805c63b_u32;
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbbInstruction::Zexth {
            rd: Reg::A2,
            rs1: Reg::A1
        })
    );
}

#[test]
fn test_no_zext_h_with_nonzero_rs2() {
    // Same encoding but rs2 != 0 → reserved/invalid for Zbb
    let inst = make_r_type(0b0111011, 1, 0b100, 2, 1, 0b0000100);
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_old_draft_zext_h_now_ror() {
    let inst = make_r_type(0b0110011, 1, 0b101, 2, 0b00100, 0b0110000);
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbbInstruction::Ror {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Tp
        })
    );
}

#[test]
fn test_rev8() {
    // rev8 is an I-type instruction with funct12 = 0b011010111000
    #[expect(clippy::unusual_byte_groupings)]
    let inst = 0b011010111000_00010_101_00001_0010011u32;
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbbInstruction::Rev8 {
            rd: Reg::Ra,
            rs1: Reg::Sp
        })
    );
}

#[test]
fn test_rev8_real_instruction() {
    // Real instruction: rev8 a3,a3 (0x6b86d693)
    let inst = 0x6b86d693u32;
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbbInstruction::Rev8 {
            rd: Reg::A3,
            rs1: Reg::A3
        })
    );
}

#[test]
fn test_orc_b() {
    let inst = make_r_type(0b0110011, 1, 0b101, 2, 0b00111, 0b0000101);
    let decoded = Rv64ZbbInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZbbInstruction::Orcb {
            rd: Reg::Ra,
            rs1: Reg::Sp
        })
    );
}
