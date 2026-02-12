extern crate alloc;

use crate::instructions::Instruction;
use crate::instructions::rv64::v::zve64x::config::Rv64Zve64xConfigInstruction;
use crate::instructions::test_utils::{make_i_type, make_r_type};
use crate::registers::general_purpose::Reg;
use alloc::format;

// vsetvli: I-type encoding
// [0|zimm[10:0]|rs1|111|rd|1010111]
// make_i_type(opcode, rd, funct3, rs1, imm) where imm = 0|zimm[10:0]

#[test]
fn test_vsetvli_basic() {
    // vtypei=0b00000001011 (e32, m8) = 0x0b
    let inst = make_i_type(0b1010111, 1, 0b111, 2, 0x0b);
    let decoded = Rv64Zve64xConfigInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xConfigInstruction::Vsetvli {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            vtypei: 0x0b,
        })
    );
}

#[test]
fn test_vsetvli_e8_m1() {
    // vtypei=0b00000000000 (e8, m1) = 0x00
    let inst = make_i_type(0b1010111, 10, 0b111, 11, 0x000);
    let decoded = Rv64Zve64xConfigInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xConfigInstruction::Vsetvli {
            rd: Reg::A0,
            rs1: Reg::A1,
            vtypei: 0x000,
        })
    );
}

#[test]
fn test_vsetvli_e64_mf8_ta_ma() {
    // e64=0b011, mf8=0b101, ta=1, ma=1 => vtypei = 0b11_0_011_101 = 0x0dd
    // Actually: vlmul[2:0]=101, vsew[2:0]=011, vta=1, vma=1
    // bits: vma(7) | vta(6) | vsew(5:3) | vlmul(2:0) = 1_1_011_101 = 0xdd
    let vtypei: u32 = 0xdd;
    let inst = make_i_type(0b1010111, 5, 0b111, 6, vtypei);
    let decoded = Rv64Zve64xConfigInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xConfigInstruction::Vsetvli {
            rd: Reg::T0,
            rs1: Reg::T1,
            vtypei: 0xdd,
        })
    );
}

#[test]
fn test_vsetvli_max_vtypei() {
    // Maximum 11-bit immediate = 0x7ff (bit31 must remain 0, so max is 0x7ff)
    let inst = make_i_type(0b1010111, 1, 0b111, 2, 0x7ff);
    let decoded = Rv64Zve64xConfigInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xConfigInstruction::Vsetvli {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            vtypei: 0x7ff,
        })
    );
}

#[test]
fn test_vsetvli_rd_zero() {
    // vsetvli x0, rs1, vtypei - used to set vtype without writing vl to a register
    let inst = make_i_type(0b1010111, 0, 0b111, 5, 0x03);
    let decoded = Rv64Zve64xConfigInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xConfigInstruction::Vsetvli {
            rd: Reg::Zero,
            rs1: Reg::T0,
            vtypei: 0x03,
        })
    );
}

#[test]
fn test_vsetvli_rs1_zero_rd_nonzero() {
    // vsetvli rd, x0, vtypei - sets vl = VLMAX
    let inst = make_i_type(0b1010111, 1, 0b111, 0, 0x03);
    let decoded = Rv64Zve64xConfigInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xConfigInstruction::Vsetvli {
            rd: Reg::Ra,
            rs1: Reg::Zero,
            vtypei: 0x03,
        })
    );
}

#[test]
fn test_vsetvli_rs1_zero_rd_zero() {
    // vsetvli x0, x0, vtypei - change vtype keeping current vl
    let inst = make_i_type(0b1010111, 0, 0b111, 0, 0x03);
    let decoded = Rv64Zve64xConfigInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xConfigInstruction::Vsetvli {
            rd: Reg::Zero,
            rs1: Reg::Zero,
            vtypei: 0x03,
        })
    );
}

// vsetivli: I-type-like encoding
// [11|zimm[9:0]|uimm[4:0]|111|rd|1010111]
// imm[11:0] = 11|zimm[9:0], rs1 field = uimm[4:0]

#[test]
fn test_vsetivli_basic() {
    // uimm=4, vtypei=0b0000001011 (e32,m8) = 0x0b
    // imm = 0b11_0000001011 = 0xc0b
    let inst = make_i_type(0b1010111, 1, 0b111, 4, 0xc0b);
    let decoded = Rv64Zve64xConfigInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xConfigInstruction::Vsetivli {
            rd: Reg::Ra,
            uimm: 4,
            vtypei: 0x0b,
        })
    );
}

#[test]
fn test_vsetivli_uimm_zero() {
    // uimm=0, vtypei=0
    let inst = make_i_type(0b1010111, 10, 0b111, 0, 0xc00);
    let decoded = Rv64Zve64xConfigInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xConfigInstruction::Vsetivli {
            rd: Reg::A0,
            uimm: 0,
            vtypei: 0x000,
        })
    );
}

#[test]
fn test_vsetivli_uimm_max() {
    // uimm=31 (max 5-bit), vtypei=0x1ff (max 10-bit: 0b11_1111_1111)
    // imm = 0b11_1111111111 = 0xfff
    let inst = make_i_type(0b1010111, 1, 0b111, 31, 0xfff);
    let decoded = Rv64Zve64xConfigInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xConfigInstruction::Vsetivli {
            rd: Reg::Ra,
            uimm: 31,
            vtypei: 0x3ff,
        })
    );
}

#[test]
fn test_vsetivli_e64_m1_ta_ma() {
    // vtypei: vma=1, vta=1, vsew=011(e64), vlmul=000(m1) = 0b11_011_000 = 0xd8
    // imm = 0b11_00_1101_1000 = 0xcd8
    let inst = make_i_type(0b1010111, 5, 0b111, 16, 0xcd8);
    let decoded = Rv64Zve64xConfigInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xConfigInstruction::Vsetivli {
            rd: Reg::T0,
            uimm: 16,
            vtypei: 0x0d8,
        })
    );
}

// vsetvl: R-type encoding
// [1000000|rs2|rs1|111|rd|1010111]
// make_r_type(opcode, rd, funct3, rs1, rs2, funct7)

#[test]
fn test_vsetvl_basic() {
    let inst = make_r_type(0b1010111, 1, 0b111, 2, 3, 0b1000000);
    let decoded = Rv64Zve64xConfigInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xConfigInstruction::Vsetvl {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        })
    );
}

#[test]
fn test_vsetvl_all_arg_regs() {
    let inst = make_r_type(0b1010111, 10, 0b111, 11, 12, 0b1000000);
    let decoded = Rv64Zve64xConfigInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xConfigInstruction::Vsetvl {
            rd: Reg::A0,
            rs1: Reg::A1,
            rs2: Reg::A2
        })
    );
}

#[test]
fn test_vsetvl_rd_zero() {
    let inst = make_r_type(0b1010111, 0, 0b111, 5, 6, 0b1000000);
    let decoded = Rv64Zve64xConfigInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xConfigInstruction::Vsetvl {
            rd: Reg::Zero,
            rs1: Reg::T0,
            rs2: Reg::T1
        })
    );
}

#[test]
fn test_vsetvl_rs1_zero_rd_nonzero() {
    // Sets vl = VLMAX
    let inst = make_r_type(0b1010111, 1, 0b111, 0, 7, 0b1000000);
    let decoded = Rv64Zve64xConfigInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64Zve64xConfigInstruction::Vsetvl {
            rd: Reg::Ra,
            rs1: Reg::Zero,
            rs2: Reg::T2
        })
    );
}

// Negative tests

#[test]
fn test_wrong_opcode() {
    // Use OP (0b0110011) instead of OP-V
    let inst = make_r_type(0b0110011, 1, 0b111, 2, 3, 0b1000000);
    let decoded = Rv64Zve64xConfigInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_wrong_funct3() {
    // funct3=0b000 (OPIVV) instead of 0b111 (OPCFG)
    let inst = make_r_type(0b1010111, 1, 0b000, 2, 3, 0b1000000);
    let decoded = Rv64Zve64xConfigInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_vsetvl_wrong_funct7() {
    // bit31=1, bit30=0, but bits[29:25] != 0 (funct7=0b1000001)
    let inst = make_r_type(0b1010111, 1, 0b111, 2, 3, 0b1000001);
    let decoded = Rv64Zve64xConfigInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_vsetvl_nonzero_bits_29_25() {
    // bit31=1, bit30=0, bits[29:25]=0b00001
    let inst = make_r_type(0b1010111, 1, 0b111, 2, 3, 0b1000010);
    let decoded = Rv64Zve64xConfigInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_vsetvli_bit31_clear() {
    // Verify that when bit31=0, we get vsetvli not vsetivli
    // imm = 0b0_111_1111_1111 = 0x7ff => vtypei = 0x7ff
    let inst = make_i_type(0b1010111, 1, 0b111, 2, 0x7ff);
    let decoded = Rv64Zve64xConfigInstruction::<Reg<u64>>::try_decode(inst);
    assert!(matches!(
        decoded,
        Some(Rv64Zve64xConfigInstruction::Vsetvli { .. })
    ));
}

#[test]
fn test_vsetivli_bits_31_30_set() {
    // imm with bits[11:10]=11 => vsetivli
    let inst = make_i_type(0b1010111, 1, 0b111, 2, 0xc00);
    let decoded = Rv64Zve64xConfigInstruction::<Reg<u64>>::try_decode(inst);
    assert!(matches!(
        decoded,
        Some(Rv64Zve64xConfigInstruction::Vsetivli { .. })
    ));
}

#[test]
fn test_display_vsetvli() {
    let inst = make_i_type(0b1010111, 1, 0b111, 2, 0x0b);
    let decoded = Rv64Zve64xConfigInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vsetvli ra, sp, 11");
}

#[test]
fn test_display_vsetivli() {
    let inst = make_i_type(0b1010111, 1, 0b111, 4, 0xc0b);
    let decoded = Rv64Zve64xConfigInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vsetivli ra, 4, 11");
}

#[test]
fn test_display_vsetvl() {
    let inst = make_r_type(0b1010111, 1, 0b111, 2, 3, 0b1000000);
    let decoded = Rv64Zve64xConfigInstruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(format!("{}", decoded), "vsetvl ra, sp, gp");
}
