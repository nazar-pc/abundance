use crate::rv32::test_utils::{execute, initialize_state};
use crate::rv32::zk::zkn::zknd::rv32_zknd_helpers::gmul;
use ab_riscv_primitives::instructions::rv32::zk::zkn::zknd::{Rv32AesBs, Rv32ZkndInstruction};
use ab_riscv_primitives::registers::general_purpose::Reg;

// aes32dsi
//
// Semantics: rd = rs1 ^ rol32(INV_SBOX[(rs2 >> (bs*8)) & 0xff] as u32, bs*8)
// Note: rs1 == rd (destructive encoding); the interpreter reads rs1 before writing rd,
// so the aliasing is not observable at the instruction level.

#[test]
fn test_aes32dsi_zero_rs1_bs0() {
    let mut state = initialize_state([Rv32ZkndInstruction::Aes32Dsi {
        rd: Reg::A2,
        rs1: Reg::A2,
        rs2: Reg::A0,
        bs: Rv32AesBs::B0,
    }]);
    // rs1=0, rs2=0x00: INV_SBOX[0x00]=0x52, shamt=0 → rol32(0x52, 0)=0x00000052
    // rd = 0 ^ 0x52 = 0x52
    state.regs.write(Reg::A2, 0u32);
    state.regs.write(Reg::A0, 0u32);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), 0x52u32);
}

#[test]
fn test_aes32dsi_zero_rs1_bs1() {
    let mut state = initialize_state([Rv32ZkndInstruction::Aes32Dsi {
        rd: Reg::A2,
        rs1: Reg::A2,
        rs2: Reg::A0,
        bs: Rv32AesBs::B1,
    }]);
    // rs1=0, rs2=0x0000_0100: byte at shamt=8 is 0x01, INV_SBOX[0x01]=0x09
    // rol32(0x09, 8) = 0x00000900
    // rd = 0 ^ 0x00000900 = 0x00000900
    state.regs.write(Reg::A2, 0u32);
    state.regs.write(Reg::A0, 0x0000_0100u32);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), 0x0000_0900u32);
}

#[test]
fn test_aes32dsi_zero_rs1_bs2() {
    let mut state = initialize_state([Rv32ZkndInstruction::Aes32Dsi {
        rd: Reg::A2,
        rs1: Reg::A2,
        rs2: Reg::A0,
        bs: Rv32AesBs::B2,
    }]);
    // rs1=0, rs2=0x00FF_0000: byte at shamt=16 is 0xff, INV_SBOX[0xff]=0x7d
    // rol32(0x7d, 16) = 0x007d_0000
    // rd = 0 ^ 0x007d_0000 = 0x007d_0000
    state.regs.write(Reg::A2, 0u32);
    state.regs.write(Reg::A0, 0x00ff_0000u32);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), 0x007d_0000u32);
}

#[test]
fn test_aes32dsi_zero_rs1_bs3() {
    let mut state = initialize_state([Rv32ZkndInstruction::Aes32Dsi {
        rd: Reg::A2,
        rs1: Reg::A2,
        rs2: Reg::A0,
        bs: Rv32AesBs::B3,
    }]);
    // rs1=0, rs2=0x0100_0000: byte at shamt=24 is 0x01, INV_SBOX[0x01]=0x09
    // rol32(0x09, 24) = 0x0900_0000
    // rd = 0 ^ 0x0900_0000 = 0x0900_0000
    state.regs.write(Reg::A2, 0u32);
    state.regs.write(Reg::A0, 0x0100_0000u32);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), 0x0900_0000u32);
}

#[test]
fn test_aes32dsi_nonzero_rs1_xors() {
    let mut state = initialize_state([Rv32ZkndInstruction::Aes32Dsi {
        rd: Reg::A2,
        rs1: Reg::A2,
        rs2: Reg::A0,
        bs: Rv32AesBs::B0,
    }]);
    // rs1=0xffff_ffff, rs2=0x00: INV_SBOX[0x00]=0x52, shamt=0 → 0x52
    // rd = 0xffff_ffff ^ 0x52 = 0xffff_ffad
    state.regs.write(Reg::A2, 0xffff_ffffu32);
    state.regs.write(Reg::A0, 0u32);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), 0xffff_ffadu32);
}

#[test]
fn test_aes32dsi_arg_swap_differs() {
    // aes32dsi with rs2=v0 vs rs2=v1 should differ for non-symmetric inputs
    let v0 = 0x0011_2233u32;
    let v1 = 0x8899_aabbu32;
    let mut state1 = initialize_state([Rv32ZkndInstruction::Aes32Dsi {
        rd: Reg::A2,
        rs1: Reg::A2,
        rs2: Reg::A0,
        bs: Rv32AesBs::B0,
    }]);
    let mut state2 = initialize_state([Rv32ZkndInstruction::Aes32Dsi {
        rd: Reg::A2,
        rs1: Reg::A2,
        rs2: Reg::A0,
        bs: Rv32AesBs::B0,
    }]);
    state1.regs.write(Reg::A2, v0);
    state1.regs.write(Reg::A0, v1);
    state2.regs.write(Reg::A2, v1);
    state2.regs.write(Reg::A0, v0);
    execute(&mut state1).unwrap();
    execute(&mut state2).unwrap();
    assert_ne!(state1.regs.read(Reg::A2), state2.regs.read(Reg::A2));
}

// aes32dsmi
//
// Semantics: rd = rs1 ^ rol32(inv_mix_col_byte(INV_SBOX[(rs2>>(bs*8))&0xff]), bs*8)

#[test]
fn test_aes32dsmi_zero_rs1_bs0() {
    let mut state = initialize_state([Rv32ZkndInstruction::Aes32Dsmi {
        rd: Reg::A2,
        rs1: Reg::A2,
        rs2: Reg::A0,
        bs: Rv32AesBs::B0,
    }]);
    // rs1=0, rs2=0x00: INV_SBOX[0x00]=0x52
    // inv_mix_col_byte(0x52):
    //   gmul(0x52, 0x0e) = ?  gmul(0x52, 0x09) = ?  gmul(0x52, 0x0d) = ?  gmul(0x52, 0x0b) = ?
    // We compute expected via the same gmul the soft backend uses.
    let b: u8 = 0x52;
    let r0 = gmul(b, 0x0e);
    let r1 = gmul(b, 0x09);
    let r2 = gmul(b, 0x0d);
    let r3 = gmul(b, 0x0b);
    let expected =
        u32::from(r0) | (u32::from(r1) << 8) | (u32::from(r2) << 16) | (u32::from(r3) << 24);
    state.regs.write(Reg::A2, 0u32);
    state.regs.write(Reg::A0, 0u32);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), expected);
}

#[test]
fn test_aes32dsmi_bs1_rotation() {
    let mut state = initialize_state([Rv32ZkndInstruction::Aes32Dsmi {
        rd: Reg::A2,
        rs1: Reg::A2,
        rs2: Reg::A0,
        bs: Rv32AesBs::B1,
    }]);
    // rs1=0, rs2 byte at shamt=8 is 0x00: INV_SBOX[0x00]=0x52
    // mixed = inv_mix_col_byte(0x52) (same as above, call it M)
    // rd = rol32(M, 8)
    let b: u8 = 0x52;
    let r0 = gmul(b, 0x0e);
    let r1 = gmul(b, 0x09);
    let r2 = gmul(b, 0x0d);
    let r3 = gmul(b, 0x0b);
    let mixed =
        u32::from(r0) | (u32::from(r1) << 8) | (u32::from(r2) << 16) | (u32::from(r3) << 24);
    let expected = mixed.rotate_left(8);
    state.regs.write(Reg::A2, 0u32);
    state.regs.write(Reg::A0, 0u32);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), expected);
}

#[test]
fn test_aes32dsi_and_aes32dsmi_differ() {
    // Same inputs: aes32dsi and aes32dsmi should produce different results because aes32dsmi
    // applies InvMixColumns and aes32dsi does not
    let mut state_dsi = initialize_state([Rv32ZkndInstruction::Aes32Dsi {
        rd: Reg::A2,
        rs1: Reg::A2,
        rs2: Reg::A0,
        bs: Rv32AesBs::B0,
    }]);
    let mut state_dsmi = initialize_state([Rv32ZkndInstruction::Aes32Dsmi {
        rd: Reg::A2,
        rs1: Reg::A2,
        rs2: Reg::A0,
        bs: Rv32AesBs::B0,
    }]);
    let rs1 = 0x1234_5678;
    let rs2 = 0xdeadbeef;
    state_dsi.regs.write(Reg::A2, rs1);
    state_dsi.regs.write(Reg::A0, rs2);
    state_dsmi.regs.write(Reg::A2, rs1);
    state_dsmi.regs.write(Reg::A0, rs2);
    execute(&mut state_dsi).unwrap();
    execute(&mut state_dsmi).unwrap();
    assert_ne!(state_dsi.regs.read(Reg::A2), state_dsmi.regs.read(Reg::A2));
}

#[test]
fn test_aes32dsmi_bs_shifts_are_distinct() {
    // The same rs2 input with different bs values should produce different results.
    let rs2 = 0x0102_0304u32;
    let mut results = [0u32; 4];
    for (i, bs) in [Rv32AesBs::B0, Rv32AesBs::B1, Rv32AesBs::B2, Rv32AesBs::B3]
        .into_iter()
        .enumerate()
    {
        let mut state = initialize_state([Rv32ZkndInstruction::Aes32Dsmi {
            rd: Reg::A2,
            rs1: Reg::A2,
            rs2: Reg::A0,
            bs,
        }]);
        state.regs.write(Reg::A2, 0u32);
        state.regs.write(Reg::A0, rs2);
        execute(&mut state).unwrap();
        results[i] = state.regs.read(Reg::A2);
    }

    // All four results should be distinct since rs2 bytes and their rotations differ
    for i in 0..4 {
        for j in (i + 1)..4 {
            assert_ne!(
                results[i], results[j],
                "bs={i} and bs={j} produced same result"
            );
        }
    }
}
