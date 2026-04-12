use crate::rv32::test_utils::{execute, initialize_state};
use crate::rv32::zk::zkn::zknd::rv32_zknd_helpers::gmul;
use ab_riscv_primitives::instructions::rv32::zk::zkn::zknd::Rv32AesBs;
use ab_riscv_primitives::instructions::rv32::zk::zkn::zkne::Rv32ZkneInstruction;
use ab_riscv_primitives::registers::general_purpose::Reg;

// aes32esi
//
// Semantics: rd = rs1 ^ rol32(SBOX[(rs2 >> (bs*8)) & 0xff] as u32, bs*8)
// Note: rs1 == rd (destructive encoding); the interpreter reads rs1 before writing rd,
// so the aliasing is not observable at the instruction level.

#[test]
fn test_aes32esi_zero_rs1_bs0() {
    let mut state = initialize_state([Rv32ZkneInstruction::Aes32Esi {
        rd: Reg::A2,
        rs1: Reg::A2,
        rs2: Reg::A0,
        bs: Rv32AesBs::B0,
    }]);
    // rs1=0, rs2=0x00: SBOX[0x00]=0x63, shamt=0 → rol32(0x63, 0)=0x00000063
    // rd = 0 ^ 0x63 = 0x63
    state.regs.write(Reg::A2, 0u32);
    state.regs.write(Reg::A0, 0u32);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), 0x63u32);
}

#[test]
fn test_aes32esi_zero_rs1_bs1() {
    let mut state = initialize_state([Rv32ZkneInstruction::Aes32Esi {
        rd: Reg::A2,
        rs1: Reg::A2,
        rs2: Reg::A0,
        bs: Rv32AesBs::B1,
    }]);
    // rs1=0, rs2=0x0000_0100: byte at shamt=8 is 0x01, SBOX[0x01]=0x7c
    // rol32(0x7c, 8) = 0x00007c00
    // rd = 0 ^ 0x00007c00 = 0x00007c00
    state.regs.write(Reg::A2, 0u32);
    state.regs.write(Reg::A0, 0x0000_0100u32);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), 0x0000_7c00u32);
}

#[test]
fn test_aes32esi_zero_rs1_bs2() {
    let mut state = initialize_state([Rv32ZkneInstruction::Aes32Esi {
        rd: Reg::A2,
        rs1: Reg::A2,
        rs2: Reg::A0,
        bs: Rv32AesBs::B2,
    }]);
    // rs1=0, rs2=0x00ff_0000: byte at shamt=16 is 0xff, SBOX[0xff]=0x16
    // rol32(0x16, 16) = 0x0016_0000
    // rd = 0 ^ 0x0016_0000 = 0x0016_0000
    state.regs.write(Reg::A2, 0u32);
    state.regs.write(Reg::A0, 0x00ff_0000u32);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), 0x0016_0000u32);
}

#[test]
fn test_aes32esi_zero_rs1_bs3() {
    let mut state = initialize_state([Rv32ZkneInstruction::Aes32Esi {
        rd: Reg::A2,
        rs1: Reg::A2,
        rs2: Reg::A0,
        bs: Rv32AesBs::B3,
    }]);
    // rs1=0, rs2=0x0100_0000: byte at shamt=24 is 0x01, SBOX[0x01]=0x7c
    // rol32(0x7c, 24) = 0x7c00_0000
    // rd = 0 ^ 0x7c00_0000 = 0x7c00_0000
    state.regs.write(Reg::A2, 0u32);
    state.regs.write(Reg::A0, 0x0100_0000u32);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), 0x7c00_0000u32);
}

#[test]
fn test_aes32esi_nonzero_rs1_xors() {
    let mut state = initialize_state([Rv32ZkneInstruction::Aes32Esi {
        rd: Reg::A2,
        rs1: Reg::A2,
        rs2: Reg::A0,
        bs: Rv32AesBs::B0,
    }]);
    // rs1=0xffff_ffff, rs2=0x00: SBOX[0x00]=0x63, shamt=0 → 0x63
    // rd = 0xffff_ffff ^ 0x63 = 0xffff_ff9c
    state.regs.write(Reg::A2, 0xffff_ffffu32);
    state.regs.write(Reg::A0, 0u32);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), 0xffff_ff9cu32);
}

#[test]
fn test_aes32esi_arg_swap_differs() {
    // aes32esi with rs2=v0 vs rs2=v1 should differ for non-symmetric inputs
    let v0 = 0x0011_2233u32;
    let v1 = 0x8899_aabbu32;
    let mut state1 = initialize_state([Rv32ZkneInstruction::Aes32Esi {
        rd: Reg::A2,
        rs1: Reg::A2,
        rs2: Reg::A0,
        bs: Rv32AesBs::B0,
    }]);
    let mut state2 = initialize_state([Rv32ZkneInstruction::Aes32Esi {
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

// aes32esmi
//
// Semantics: rd = rs1 ^ rol32(mix_col_byte(SBOX[(rs2>>(bs*8))&0xff]), bs*8)

#[test]
fn test_aes32esmi_zero_rs1_bs0() {
    let mut state = initialize_state([Rv32ZkneInstruction::Aes32Esmi {
        rd: Reg::A2,
        rs1: Reg::A2,
        rs2: Reg::A0,
        bs: Rv32AesBs::B0,
    }]);
    // rs1=0, rs2=0x00: SBOX[0x00]=0x63
    // mix_col_byte(0x63):
    //   r0 = gmul(0x63, 0x02), r1 = 0x63, r2 = 0x63, r3 = gmul(0x63, 0x03)
    // expected computed via the same gmul the soft backend uses.
    let b: u8 = 0x63;
    let r0 = gmul(b, 0x02);
    let r1 = b;
    let r2 = b;
    let r3 = gmul(b, 0x03);
    let expected =
        u32::from(r0) | (u32::from(r1) << 8) | (u32::from(r2) << 16) | (u32::from(r3) << 24);
    state.regs.write(Reg::A2, 0u32);
    state.regs.write(Reg::A0, 0u32);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), expected);
}

#[test]
fn test_aes32esmi_bs1_rotation() {
    let mut state = initialize_state([Rv32ZkneInstruction::Aes32Esmi {
        rd: Reg::A2,
        rs1: Reg::A2,
        rs2: Reg::A0,
        bs: Rv32AesBs::B1,
    }]);
    // rs1=0, rs2 byte at shamt=8 is 0x00: SBOX[0x00]=0x63
    // mixed = mix_col_byte(0x63) (same as above, call it M)
    // rd = rol32(M, 8)
    let b: u8 = 0x63;
    let r0 = gmul(b, 0x02);
    let r1 = b;
    let r2 = b;
    let r3 = gmul(b, 0x03);
    let mixed =
        u32::from(r0) | (u32::from(r1) << 8) | (u32::from(r2) << 16) | (u32::from(r3) << 24);
    let expected = mixed.rotate_left(8);
    state.regs.write(Reg::A2, 0u32);
    state.regs.write(Reg::A0, 0u32);
    execute(&mut state).unwrap();
    assert_eq!(state.regs.read(Reg::A2), expected);
}

#[test]
fn test_aes32esi_and_aes32esmi_differ() {
    // Same inputs: aes32esi and aes32esmi should produce different results because aes32esmi
    // applies MixColumns and aes32esi does not
    let mut state_esi = initialize_state([Rv32ZkneInstruction::Aes32Esi {
        rd: Reg::A2,
        rs1: Reg::A2,
        rs2: Reg::A0,
        bs: Rv32AesBs::B0,
    }]);
    let mut state_esmi = initialize_state([Rv32ZkneInstruction::Aes32Esmi {
        rd: Reg::A2,
        rs1: Reg::A2,
        rs2: Reg::A0,
        bs: Rv32AesBs::B0,
    }]);
    let rs1 = 0x1234_5678;
    let rs2 = 0xdeadbeef;
    state_esi.regs.write(Reg::A2, rs1);
    state_esi.regs.write(Reg::A0, rs2);
    state_esmi.regs.write(Reg::A2, rs1);
    state_esmi.regs.write(Reg::A0, rs2);
    execute(&mut state_esi).unwrap();
    execute(&mut state_esmi).unwrap();
    assert_ne!(state_esi.regs.read(Reg::A2), state_esmi.regs.read(Reg::A2));
}

#[test]
fn test_aes32esmi_bs_shifts_are_distinct() {
    // The same rs2 input with different bs values should produce different results.
    let rs2 = 0x0102_0304u32;
    let mut results = [0u32; 4];
    for (i, bs) in [Rv32AesBs::B0, Rv32AesBs::B1, Rv32AesBs::B2, Rv32AesBs::B3]
        .into_iter()
        .enumerate()
    {
        let mut state = initialize_state([Rv32ZkneInstruction::Aes32Esmi {
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
