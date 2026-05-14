#![expect(clippy::unusual_byte_groupings, reason = "Test readability")]

use crate::instructions::Instruction;
use crate::instructions::rv32::Rv32Instruction;
use crate::instructions::test_utils::{
    make_b_type, make_i_type, make_j_type, make_r_type, make_s_type, make_u_type,
};
use crate::instructions::utils::{I24, I24WithZeroedBits};
use crate::registers::general_purpose::{EReg, Reg};

// R-type

#[test]
fn test_add() {
    let inst = make_r_type(0b0110011, 1, 0b000, 2, 3, 0b0000000);
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32Instruction::Add {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        }
    );
}

#[test]
fn test_sub() {
    let inst = make_r_type(0b0110011, 5, 0b000, 6, 7, 0b0100000);
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32Instruction::Sub {
            rd: Reg::T0,
            rs1: Reg::T1,
            rs2: Reg::T2
        }
    );
}

#[test]
fn test_sll() {
    let inst = make_r_type(0b0110011, 10, 0b001, 11, 12, 0b0000000);
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32Instruction::Sll {
            rd: Reg::A0,
            rs1: Reg::A1,
            rs2: Reg::A2
        }
    );
}

#[test]
fn test_slt() {
    let inst = make_r_type(0b0110011, 1, 0b010, 2, 3, 0b0000000);
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32Instruction::Slt {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        }
    );
}

#[test]
fn test_sltu() {
    let inst = make_r_type(0b0110011, 1, 0b011, 2, 3, 0b0000000);
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32Instruction::Sltu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        }
    );
}

#[test]
fn test_xor() {
    let inst = make_r_type(0b0110011, 1, 0b100, 2, 3, 0b0000000);
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32Instruction::Xor {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        }
    );
}

#[test]
fn test_srl() {
    let inst = make_r_type(0b0110011, 1, 0b101, 2, 3, 0b0000000);
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32Instruction::Srl {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        }
    );
}

#[test]
fn test_sra() {
    let inst = make_r_type(0b0110011, 1, 0b101, 2, 3, 0b0100000);
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32Instruction::Sra {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        }
    );
}

#[test]
fn test_or() {
    let inst = make_r_type(0b0110011, 1, 0b110, 2, 3, 0b0000000);
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32Instruction::Or {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        }
    );
}

#[test]
fn test_and() {
    let inst = make_r_type(0b0110011, 1, 0b111, 2, 3, 0b0000000);
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32Instruction::And {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        }
    );
}

// I-type

#[test]
fn test_addi() {
    {
        // Positive immediate
        let inst = make_i_type(0b0010011, 1, 0b000, 2, 100);
        let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv32Instruction::Addi {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                imm: 100
            }
        );
    }

    {
        // Negative immediate (-1)
        let inst = make_i_type(0b0010011, 1, 0b000, 2, 0xfff);
        let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv32Instruction::Addi {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                imm: -1
            }
        );
    }

    {
        // Max positive 12-bit signed
        let inst = make_i_type(0b0010011, 1, 0b000, 2, 0x7ff);
        let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv32Instruction::Addi {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                imm: 2047
            }
        );
    }

    {
        // Min negative 12-bit signed
        let inst = make_i_type(0b0010011, 1, 0b000, 2, 0x800);
        let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv32Instruction::Addi {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                imm: -2048
            }
        );
    }
}

#[test]
fn test_slti() {
    let inst = make_i_type(0b0010011, 1, 0b010, 2, 50);
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32Instruction::Slti {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            imm: 50
        }
    );
}

#[test]
fn test_sltiu() {
    let inst = make_i_type(0b0010011, 1, 0b011, 2, 50);
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32Instruction::Sltiu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            imm: 50
        }
    );
}

#[test]
fn test_xori() {
    let inst = make_i_type(0b0010011, 1, 0b100, 2, 0xff);
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32Instruction::Xori {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            imm: 0xff
        }
    );
}

#[test]
fn test_ori() {
    let inst = make_i_type(0b0010011, 1, 0b110, 2, 0xff);
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32Instruction::Ori {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            imm: 0xff
        }
    );
}

#[test]
fn test_andi() {
    let inst = make_i_type(0b0010011, 1, 0b111, 2, 0xff);
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32Instruction::Andi {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            imm: 0xff
        }
    );
}

#[test]
fn test_slli() {
    {
        // Basic shift
        let inst = make_i_type(0b0010011, 1, 0b001, 2, 10);
        let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv32Instruction::Slli {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                shamt: 10
            }
        );
    }

    {
        // Max shift (shamt=31) - all 5 bits set
        let inst = make_i_type(0b0010011, 1, 0b001, 2, 31);
        let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv32Instruction::Slli {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                shamt: 31
            }
        );
    }

    {
        // Invalid: bit 25 set (funct7 != 0b0000000) - would be valid in RV64 but not RV32
        let shamt = 10u32;
        let inst = 0b0010011 | (1 << 7) | (0b001 << 12) | (2 << 15) | (shamt << 20) | (1 << 25);
        let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst);
        assert!(
            decoded.is_none(),
            "SLLI with bit 25 set should be invalid in RV32 (funct7 != 0)"
        );
    }

    {
        // Invalid: bit 30 set (funct7 = 0b0100000, which is SRAI's funct7)
        let shamt = 10u32;
        let inst =
            0b0010011 | (1 << 7) | (0b001 << 12) | (2 << 15) | (shamt << 20) | (0b0100000 << 25);
        let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst);
        assert!(
            decoded.is_none(),
            "SLLI with funct7=0b0100000 should be invalid"
        );
    }
}

#[test]
fn test_srli() {
    {
        // Basic shift
        let inst = make_i_type(0b0010011, 1, 0b101, 2, 10);
        let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv32Instruction::Srli {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                shamt: 10
            }
        );
    }

    {
        // Max shift (shamt=31)
        let inst = make_i_type(0b0010011, 1, 0b101, 2, 31);
        let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv32Instruction::Srli {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                shamt: 31
            }
        );
    }

    {
        // Invalid: bit 25 set (funct7 != 0b0000000)
        let shamt = 10u32;
        let inst = 0b0010011 | (1 << 7) | (0b101 << 12) | (2 << 15) | (shamt << 20) | (1 << 25);
        let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst);
        assert!(
            decoded.is_none(),
            "SRLI with bit 25 set should be invalid in RV32"
        );
    }
}

#[test]
fn test_srai() {
    {
        // Basic shift with correct funct7
        let shamt = 10u32;
        let inst =
            0b0010011 | (1 << 7) | (0b101 << 12) | (2 << 15) | (shamt << 20) | (0b0100000 << 25);
        let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv32Instruction::Srai {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                shamt: 10
            }
        );
    }

    {
        // Max shift (shamt=31)
        let shamt = 31u32;
        let inst =
            0b0010011 | (1 << 7) | (0b101 << 12) | (2 << 15) | (shamt << 20) | (0b0100000 << 25);
        let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv32Instruction::Srai {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                shamt: 31
            }
        );
    }

    {
        // Without SRAI's funct7 bit, this is SRLI
        let shamt = 10u32;
        let inst = 0b0010011 | (1 << 7) | (0b101 << 12) | (2 << 15) | (shamt << 20);
        let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv32Instruction::Srli {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                shamt: 10
            },
            "Without SRAI funct7, this is SRLI"
        );
    }

    {
        // Invalid: extra bits set in funct7
        let shamt = 10u32;
        let inst =
            0b0010011 | (1 << 7) | (0b101 << 12) | (2 << 15) | (shamt << 20) | (0b0100001 << 25);
        let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst);
        assert!(
            decoded.is_none(),
            "SRAI with extra funct7 bits should be invalid"
        );
    }
}

// Loads (I-type)

#[test]
fn test_lb() {
    let inst = make_i_type(0b0000011, 1, 0b000, 2, 100);
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32Instruction::Lb {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            imm: 100
        }
    );
}

#[test]
fn test_lh() {
    let inst = make_i_type(0b0000011, 1, 0b001, 2, 100);
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32Instruction::Lh {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            imm: 100
        }
    );
}

#[test]
fn test_lw() {
    {
        // Positive offset
        let inst = make_i_type(0b0000011, 1, 0b010, 2, 100);
        let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv32Instruction::Lw {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                imm: 100
            }
        );
    }

    {
        // Negative offset (-4)
        let inst = make_i_type(0b0000011, 1, 0b010, 2, 0xffc);
        let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv32Instruction::Lw {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                imm: -4
            }
        );
    }
}

#[test]
fn test_lbu() {
    let inst = make_i_type(0b0000011, 1, 0b100, 2, 100);
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32Instruction::Lbu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            imm: 100
        }
    );
}

#[test]
fn test_lhu() {
    let inst = make_i_type(0b0000011, 1, 0b101, 2, 100);
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32Instruction::Lhu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            imm: 100
        }
    );
}

// RV32 does not have LWU or LD - verify they don't accidentally decode

#[test]
fn test_no_ld() {
    // funct3=0b011 on load opcode is LD in RV64, must be invalid in RV32
    let inst = make_i_type(0b0000011, 1, 0b011, 2, 100);
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst);
    assert!(
        decoded.is_none(),
        "LD (funct3=0b011) must not decode in RV32"
    );
}

#[test]
fn test_no_lwu() {
    // funct3=0b110 on load opcode is LWU in RV64, must be invalid in RV32
    let inst = make_i_type(0b0000011, 1, 0b110, 2, 100);
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst);
    assert!(
        decoded.is_none(),
        "LWU (funct3=0b110) must not decode in RV32"
    );
}

// Jalr (I-type)

#[test]
fn test_jalr() {
    let inst = make_i_type(0b1100111, 1, 0b000, 2, 100);
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32Instruction::Jalr {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            imm: 100
        }
    );
}

// S-type

#[test]
fn test_sb() {
    let inst = make_s_type(0b0100011, 0b000, 2, 3, 100);
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32Instruction::Sb {
            rs2: Reg::Gp,
            rs1: Reg::Sp,
            imm: 100
        }
    );
}

#[test]
fn test_sh() {
    let inst = make_s_type(0b0100011, 0b001, 2, 3, 100);
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32Instruction::Sh {
            rs2: Reg::Gp,
            rs1: Reg::Sp,
            imm: 100
        }
    );
}

#[test]
fn test_sw() {
    {
        // Positive offset
        let inst = make_s_type(0b0100011, 0b010, 2, 3, 100);
        let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv32Instruction::Sw {
                rs2: Reg::Gp,
                rs1: Reg::Sp,
                imm: 100
            }
        );
    }

    {
        // Negative offset
        let inst = make_s_type(0b0100011, 0b010, 2, 3, -8);
        let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv32Instruction::Sw {
                rs2: Reg::Gp,
                rs1: Reg::Sp,
                imm: -8
            }
        );
    }
}

// RV32 does not have SD - verify it doesn't accidentally decode

#[test]
fn test_no_sd() {
    // funct3=0b011 on store opcode is SD in RV64, must be invalid in RV32
    let inst = make_s_type(0b0100011, 0b011, 2, 3, 0);
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst);
    assert!(
        decoded.is_none(),
        "SD (funct3=0b011) must not decode in RV32"
    );
}

// B-type

#[test]
fn test_beq() {
    {
        // Positive offset
        let inst = make_b_type(0b1100011, 0b000, 1, 2, 0x100);
        let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv32Instruction::Beq {
                rs1: Reg::Ra,
                rs2: Reg::Sp,
                imm: I24::from_i32(0x100)
            }
        );
    }

    {
        // Negative offset
        let inst = make_b_type(0b1100011, 0b000, 1, 2, -8);
        let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv32Instruction::Beq {
                rs1: Reg::Ra,
                rs2: Reg::Sp,
                imm: I24::from_i32(-8)
            }
        );
    }
}

#[test]
fn test_bne() {
    let inst = make_b_type(0b1100011, 0b001, 1, 2, 0x100);
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32Instruction::Bne {
            rs1: Reg::Ra,
            rs2: Reg::Sp,
            imm: I24::from_i32(0x100)
        }
    );
}

#[test]
fn test_blt() {
    let inst = make_b_type(0b1100011, 0b100, 1, 2, 0x100);
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32Instruction::Blt {
            rs1: Reg::Ra,
            rs2: Reg::Sp,
            imm: I24::from_i32(0x100)
        }
    );
}

#[test]
fn test_bge() {
    let inst = make_b_type(0b1100011, 0b101, 1, 2, 0x100);
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32Instruction::Bge {
            rs1: Reg::Ra,
            rs2: Reg::Sp,
            imm: I24::from_i32(0x100)
        }
    );
}

#[test]
fn test_bltu() {
    let inst = make_b_type(0b1100011, 0b110, 1, 2, 0x100);
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32Instruction::Bltu {
            rs1: Reg::Ra,
            rs2: Reg::Sp,
            imm: I24::from_i32(0x100)
        }
    );
}

#[test]
fn test_bgeu() {
    let inst = make_b_type(0b1100011, 0b111, 1, 2, 0x100);
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32Instruction::Bgeu {
            rs1: Reg::Ra,
            rs2: Reg::Sp,
            imm: I24::from_i32(0x100)
        }
    );
}

// Lui (U-type)

#[test]
fn test_lui() {
    let inst = make_u_type(0b0110111, 1, 0x12345000);
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32Instruction::Lui {
            rd: Reg::Ra,
            imm: I24WithZeroedBits::from_i32(0x12345000)
        }
    );
}

// Auipc (U-type)

#[test]
fn test_auipc() {
    let inst = make_u_type(0b0010111, 1, 0x12345000);
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32Instruction::Auipc {
            rd: Reg::Ra,
            imm: I24WithZeroedBits::from_i32(0x12345000)
        }
    );
}

// Jal (J-type)

#[test]
fn test_jal() {
    {
        // Positive offset
        let inst = make_j_type(0b1101111, 1, 0x1000);
        let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv32Instruction::Jal {
                rd: Reg::Ra,
                imm: I24::from_i32(0x1000)
            }
        );
    }

    {
        // Negative offset
        let inst = make_j_type(0b1101111, 1, -0x1000);
        let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv32Instruction::Jal {
                rd: Reg::Ra,
                imm: I24::from_i32(-0x1000)
            }
        );
    }
}

// Fence

#[test]
fn test_fence_valid() {
    // Common full memory fence (fence iorw,iorw): pred=0xf, succ=0xf, fm=0
    let inst = 0x0ff0_000f_u32;
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(decoded, Rv32Instruction::Fence { pred: 15, succ: 15 });

    let inst = 0b0001111_u32 | (3_u32 << 24) | (3_u32 << 20);
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(decoded, Rv32Instruction::Fence { pred: 3, succ: 3 });
}

#[test]
fn test_fence_invalid() {
    // Non-zero fm (reserved, must be 0)
    let inst = 0x10f0_000f_u32;
    assert!(Rv32Instruction::<Reg<u32>>::try_decode(inst).is_none());

    // FENCE.I (funct3=1) - we explicitly reject it
    let inst = 0x0000_100f_u32;
    assert!(Rv32Instruction::<Reg<u32>>::try_decode(inst).is_none());

    // rd != 0
    let inst = 0x0ff0_100f_u32;
    assert!(Rv32Instruction::<Reg<u32>>::try_decode(inst).is_none());

    // rs1 != 0
    let inst = 0x0ff8_000f_u32;
    assert!(Rv32Instruction::<Reg<u32>>::try_decode(inst).is_none());

    // Wrong funct3 (not 0, not 1)
    let inst = 0x0ff0_200f_u32;
    assert!(Rv32Instruction::<Reg<u32>>::try_decode(inst).is_none());
}

#[test]
fn test_fence_tso() {
    // Canonical encoding: 0x8330000f
    let inst = 0x8330_000f_u32;
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(decoded, Rv32Instruction::FenceTso);
}

#[test]
fn test_fence_tso_invalid_variants() {
    // fm=8 but pred != 0b0011
    let inst = 0x8230_000f_u32;
    assert!(Rv32Instruction::<Reg<u32>>::try_decode(inst).is_none());

    // fm=8 but succ != 0b0011
    let inst = 0x8310_000f_u32;
    assert!(Rv32Instruction::<Reg<u32>>::try_decode(inst).is_none());

    // fm=1 - reserved
    let inst = 0x10f0_000f_u32;
    assert!(Rv32Instruction::<Reg<u32>>::try_decode(inst).is_none());

    // fm=15 - reserved
    let inst = 0xfff0_000f_u32;
    assert!(Rv32Instruction::<Reg<u32>>::try_decode(inst).is_none());
}

// System instructions

#[test]
fn test_ecall() {
    let inst = 0b000000000000_00000_000_00000_1110011u32;
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(decoded, Rv32Instruction::Ecall);
}

#[test]
fn test_ebreak() {
    let inst = 0b000000000001_00000_000_00000_1110011u32;
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(decoded, Rv32Instruction::Ebreak);
}

// Unimplemented/illegal

#[test]
fn test_unimp() {
    let inst = 0xc0001073u32;
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(decoded, Rv32Instruction::Unimp);
}

// Invalid instructions

#[test]
fn test_invalid() {
    {
        // Invalid opcode
        let inst = 0b1111111u32;
        let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst);
        assert!(decoded.is_none());
    }

    {
        // Invalid R-type funct7
        let inst = make_r_type(0b0110011, 1, 0b000, 2, 3, 0b1111111);
        let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst);
        assert!(decoded.is_none());
    }
}

// RV32E Variant Tests

#[test]
fn test_rv32e() {
    {
        // Valid RV32E instruction
        let inst = make_r_type(0b0110011, 1, 0b000, 2, 3, 0b0000000);
        let decoded = Rv32Instruction::<EReg<u32>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv32Instruction::Add {
                rd: EReg::Ra,
                rs1: EReg::Sp,
                rs2: EReg::Gp
            }
        );
    }

    {
        // Max valid register (15/A5) in RV32E
        let inst = make_r_type(0b0110011, 15, 0b000, 14, 13, 0b0000000);
        let decoded = Rv32Instruction::<EReg<u32>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv32Instruction::Add {
                rd: EReg::A5,
                rs1: EReg::A4,
                rs2: EReg::A3
            }
        );
    }

    {
        // Invalid register (16 doesn't exist in RV32E)
        let inst = make_r_type(0b0110011, 16, 0b000, 2, 3, 0b0000000);
        let decoded = Rv32Instruction::<EReg<u32>>::try_decode(inst);
        assert!(decoded.is_none());
    }
}

// Edge Cases

#[test]
fn test_zero_register() {
    let inst = make_r_type(0b0110011, 0, 0b000, 0, 0, 0b0000000);
    let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv32Instruction::Add {
            rd: Reg::Zero,
            rs1: Reg::Zero,
            rs2: Reg::Zero
        }
    );
}

#[test]
fn test_all_registers_rv32i() {
    for reg_num in 0..32 {
        let inst = make_r_type(0b0110011, reg_num, 0b000, 1, 2, 0b0000000);
        let decoded = Rv32Instruction::<Reg<u32>>::try_decode(inst).unwrap();
        assert!(
            matches!(decoded, Rv32Instruction::Add { .. }),
            "Register {} should be valid for RV32I",
            reg_num
        );
    }
}

#[test]
fn test_all_registers_rv32e() {
    // Valid registers (0-15)
    for reg_num in 0..16 {
        let inst = make_r_type(0b0110011, reg_num, 0b000, 1, 2, 0b0000000);
        let decoded = Rv32Instruction::<EReg<u32>>::try_decode(inst).unwrap();
        assert!(
            matches!(decoded, Rv32Instruction::Add { .. }),
            "Register {} should be valid for RV32E",
            reg_num
        );
    }

    // Invalid registers (16-31)
    for reg_num in 16..32 {
        let inst = make_r_type(0b0110011, reg_num, 0b000, 1, 2, 0b0000000);
        let decoded = Rv32Instruction::<EReg<u32>>::try_decode(inst);
        assert!(
            decoded.is_none(),
            "Register {} should be invalid for RV32E",
            reg_num
        );
    }
}
