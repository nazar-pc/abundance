#![expect(clippy::unusual_byte_groupings, reason = "Test readability")]

use crate::instructions::Instruction;
use crate::instructions::rv64::Rv64Instruction;
use crate::instructions::test_utils::{
    make_b_type, make_i_type, make_j_type, make_r_type, make_s_type, make_u_type,
};
use crate::instructions::utils::{I24, I24WithZeroedBits};
use crate::registers::general_purpose::{EReg, Reg};

// R-type

#[test]
fn test_add() {
    let inst = make_r_type(0b011_0011, 1, 0b000, 2, 3, 0b000_0000);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Add {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        }
    );
}

#[test]
fn test_sub() {
    let inst = make_r_type(0b011_0011, 5, 0b000, 6, 7, 0b010_0000);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Sub {
            rd: Reg::T0,
            rs1: Reg::T1,
            rs2: Reg::T2
        }
    );
}

#[test]
fn test_sll() {
    let inst = make_r_type(0b011_0011, 10, 0b001, 11, 12, 0b000_0000);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Sll {
            rd: Reg::A0,
            rs1: Reg::A1,
            rs2: Reg::A2
        }
    );
}

#[test]
fn test_slt() {
    let inst = make_r_type(0b011_0011, 1, 0b010, 2, 3, 0b000_0000);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Slt {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        }
    );
}

#[test]
fn test_sltu() {
    let inst = make_r_type(0b011_0011, 1, 0b011, 2, 3, 0b000_0000);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Sltu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        }
    );
}

#[test]
fn test_xor() {
    let inst = make_r_type(0b011_0011, 1, 0b100, 2, 3, 0b000_0000);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Xor {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        }
    );
}

#[test]
fn test_srl() {
    let inst = make_r_type(0b011_0011, 1, 0b101, 2, 3, 0b000_0000);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Srl {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        }
    );
}

#[test]
fn test_sra() {
    let inst = make_r_type(0b011_0011, 1, 0b101, 2, 3, 0b010_0000);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Sra {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        }
    );
}

#[test]
fn test_or() {
    let inst = make_r_type(0b011_0011, 1, 0b110, 2, 3, 0b000_0000);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Or {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        }
    );
}

#[test]
fn test_and() {
    let inst = make_r_type(0b011_0011, 1, 0b111, 2, 3, 0b000_0000);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::And {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        }
    );
}

// RV64 R-type W

#[test]
fn test_addw() {
    let inst = make_r_type(0b011_1011, 1, 0b000, 2, 3, 0b000_0000);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Addw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        }
    );
}

#[test]
fn test_subw() {
    let inst = make_r_type(0b011_1011, 1, 0b000, 2, 3, 0b010_0000);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Subw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        }
    );
}

#[test]
fn test_sllw() {
    let inst = make_r_type(0b011_1011, 1, 0b001, 2, 3, 0b000_0000);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Sllw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        }
    );
}

#[test]
fn test_srlw() {
    let inst = make_r_type(0b011_1011, 1, 0b101, 2, 3, 0b000_0000);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Srlw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        }
    );
}

#[test]
fn test_sraw() {
    let inst = make_r_type(0b011_1011, 1, 0b101, 2, 3, 0b010_0000);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Sraw {
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
        let inst = make_i_type(0b001_0011, 1, 0b000, 2, 100);
        let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv64Instruction::Addi {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                imm: 100,
                rs2: Reg::Zero,
            }
        );
    }

    {
        // Negative immediate (-1)
        let inst = make_i_type(0b001_0011, 1, 0b000, 2, 0xfff);
        let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv64Instruction::Addi {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                imm: -1,
                rs2: Reg::Zero,
            }
        );
    }

    {
        // Max positive 12-bit signed
        let inst = make_i_type(0b001_0011, 1, 0b000, 2, 0x7ff);
        let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv64Instruction::Addi {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                imm: 2047,
                rs2: Reg::Zero,
            }
        );
    }

    {
        // Min negative 12-bit signed
        let inst = make_i_type(0b001_0011, 1, 0b000, 2, 0x800);
        let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv64Instruction::Addi {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                imm: -2048,
                rs2: Reg::Zero,
            }
        );
    }
}

#[test]
fn test_slti() {
    let inst = make_i_type(0b001_0011, 1, 0b010, 2, 50);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Slti {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            imm: 50,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_sltiu() {
    let inst = make_i_type(0b001_0011, 1, 0b011, 2, 50);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Sltiu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            imm: 50,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_xori() {
    let inst = make_i_type(0b001_0011, 1, 0b100, 2, 0xff);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Xori {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            imm: 0xff,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_ori() {
    let inst = make_i_type(0b001_0011, 1, 0b110, 2, 0xff);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Ori {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            imm: 0xff,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_andi() {
    let inst = make_i_type(0b001_0011, 1, 0b111, 2, 0xff);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Andi {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            imm: 0xff,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_slli() {
    {
        // Basic shift
        let inst = make_i_type(0b001_0011, 1, 0b001, 2, 10);
        let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv64Instruction::Slli {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                shamt: 10,
                rs2: Reg::Zero,
            }
        );
    }

    {
        // Mid shift (bit 5 set) - tests 6-bit shamt handling
        let inst = make_i_type(0b001_0011, 1, 0b001, 2, 32);
        let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv64Instruction::Slli {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                shamt: 32,
                rs2: Reg::Zero,
            },
            "SLLI with shamt=32 should decode correctly"
        );
    }

    {
        // Max shift - all 6 bits set (tests funct6 is checked correctly)
        let inst = make_i_type(0b001_0011, 1, 0b001, 2, 63);
        let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv64Instruction::Slli {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                shamt: 63,
                rs2: Reg::Zero,
            },
            "SLLI with shamt=63 should decode correctly (tests funct6 handling)"
        );
    }

    {
        // Invalid: bit 26 set (would pass with a buggy `funct7 & 0b111_1100` check)
        // This specifically tests that funct6 must be exactly 0b000000
        let shamt = 10u32;
        let inst =
            0b001_0011 | (1 << 7u8) | (0b001 << 12u8) | (2 << 15u8) | (shamt << 20u8) | (1 << 26u8);
        let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst);
        assert!(
            decoded.is_none(),
            "SLLI with bit 26 set should be invalid (catches funct7 & 0b111_1100 bug)"
        );
    }

    {
        // Invalid: bit 27 set
        let shamt = 10u32;
        let inst =
            0b001_0011 | (1 << 7u8) | (0b001 << 12u8) | (2 << 15u8) | (shamt << 20u8) | (1 << 27u8);
        let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst);
        assert!(decoded.is_none(), "SLLI with bit 27 set should be invalid");
    }

    {
        // Invalid: multiple funct6 bits set
        let shamt = 10u32;
        let inst = 0b001_0011
            | (1 << 7u8)
            | (0b001 << 12u8)
            | (2 << 15u8)
            | (shamt << 20u8)
            | (0b01_0000 << 26u8);
        let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst);
        assert!(
            decoded.is_none(),
            "SLLI with funct6=0b01_0000 (SRAI's funct6) should be invalid"
        );
    }
}

#[test]
fn test_srli() {
    {
        // Basic shift
        let inst = make_i_type(0b001_0011, 1, 0b101, 2, 10);
        let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv64Instruction::Srli {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                shamt: 10,
                rs2: Reg::Zero,
            }
        );
    }

    {
        // Mid shift (bit 5 set) - tests 6-bit shamt handling
        let inst = make_i_type(0b001_0011, 1, 0b101, 2, 32);
        let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv64Instruction::Srli {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                shamt: 32,
                rs2: Reg::Zero,
            },
            "SRLI with shamt=32 should decode correctly"
        );
    }

    {
        // Max shift - tests funct6 is checked correctly
        let inst = make_i_type(0b001_0011, 1, 0b101, 2, 63);
        let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv64Instruction::Srli {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                shamt: 63,
                rs2: Reg::Zero,
            },
            "SRLI with shamt=63 should decode correctly (tests funct6 handling)"
        );
    }

    {
        // Invalid: bit 26 set (funct6 = 0b000001)
        let shamt = 10u32;
        let inst =
            0b001_0011 | (1 << 7u8) | (0b101 << 12u8) | (2 << 15u8) | (shamt << 20u8) | (1 << 26u8);
        let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst);
        assert!(
            decoded.is_none(),
            "SRLI with funct6=0b000001 should be invalid"
        );
    }

    {
        // Invalid: bits 26 and 27 set (funct6 = 0b000011)
        let shamt = 10u32;
        let inst = 0b001_0011
            | (1 << 7u8)
            | (0b101 << 12u8)
            | (2 << 15u8)
            | (shamt << 20u8)
            | (0b11 << 26u8);
        let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst);
        assert!(
            decoded.is_none(),
            "SRLI with funct6=0b000011 should be invalid"
        );
    }

    {
        // Invalid: bit 31 set (funct6 = 0b10_0000)
        let shamt = 10u32;
        let inst = 0b001_0011
            | (1 << 7u8)
            | (0b101 << 12u8)
            | (2 << 15u8)
            | (shamt << 20u8)
            | (1u32 << 31u8);
        let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst);
        assert!(
            decoded.is_none(),
            "SRLI with funct6=0b10_0000 should be invalid"
        );
    }
}

#[test]
fn test_srai() {
    {
        // Basic shift with correct funct6
        let shamt = 10u32;
        let inst = 0b001_0011
            | (1 << 7u8)
            | (0b101 << 12u8)
            | (2 << 15u8)
            | (shamt << 20u8)
            | (0b01_0000 << 26u8);
        let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv64Instruction::Srai {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                shamt: 10,
                rs2: Reg::Zero,
            }
        );
    }

    {
        // Mid shift (bit 5 set) - tests 6-bit shamt handling
        let shamt = 32u32;
        let inst = 0b001_0011
            | (1 << 7u8)
            | (0b101 << 12u8)
            | (2 << 15u8)
            | (shamt << 20u8)
            | (0b01_0000 << 26u8);
        let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv64Instruction::Srai {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                shamt: 32,
                rs2: Reg::Zero,
            },
            "SRAI with shamt=32 should decode correctly"
        );
    }

    {
        // Max shift - tests funct6 is checked correctly
        let shamt = 63u32;
        let inst = 0b001_0011
            | (1 << 7u8)
            | (0b101 << 12u8)
            | (2 << 15u8)
            | (shamt << 20u8)
            | (0b01_0000 << 26u8);
        let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv64Instruction::Srai {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                shamt: 63,
                rs2: Reg::Zero,
            },
            "SRAI with shamt=63 should decode correctly (tests funct6 handling)"
        );
    }

    {
        // Invalid: bit 26 not set (this is SRLI, not SRAI)
        let shamt = 10u32;
        let inst = 0b001_0011 | (1 << 7u8) | (0b101 << 12u8) | (2 << 15u8) | (shamt << 20u8);
        let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv64Instruction::Srli {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                shamt: 10,
                rs2: Reg::Zero,
            },
            "Without funct6 bit 4, this is SRLI"
        );
    }

    {
        // Invalid: extra bits set in funct6
        let shamt = 10u32;
        let inst = 0b001_0011
            | (1 << 7u8)
            | (0b101 << 12u8)
            | (2 << 15u8)
            | (shamt << 20u8)
            | (0b01_0001 << 26u8);
        let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst);
        assert!(
            decoded.is_none(),
            "SRAI with extra funct6 bits should be invalid"
        );
    }

    {
        // Invalid: wrong funct6 pattern
        let shamt = 10u32;
        let inst = 0b001_0011
            | (1 << 7u8)
            | (0b101 << 12u8)
            | (2 << 15u8)
            | (shamt << 20u8)
            | (0b10_0000 << 26u8);
        let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst);
        assert!(
            decoded.is_none(),
            "SRAI with funct6=0b100000 should be invalid"
        );
    }
}

// RV64 I-type W

#[test]
fn test_addiw() {
    let inst = make_i_type(0b001_1011, 1, 0b000, 2, 100);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Addiw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            imm: 100,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_slliw() {
    let inst = make_i_type(0b001_1011, 1, 0b001, 2, 10);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Slliw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            shamt: 10,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_srliw() {
    let inst = make_i_type(0b001_1011, 1, 0b101, 2, 10);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Srliw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            shamt: 10,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_sraiw() {
    let inst = make_i_type(0b001_1011, 1, 0b101, 2, 10 | (0b010_0000 << 5u8));
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Sraiw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            shamt: 10,
            rs2: Reg::Zero,
        }
    );
}

// Loads (I-type)

#[test]
fn test_lb() {
    let inst = make_i_type(0b000_0011, 1, 0b000, 2, 100);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Lb {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            imm: 100,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_lh() {
    let inst = make_i_type(0b000_0011, 1, 0b001, 2, 100);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Lh {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            imm: 100,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_lw() {
    let inst = make_i_type(0b000_0011, 1, 0b010, 2, 100);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Lw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            imm: 100,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_ld() {
    {
        // Positive offset
        let inst = make_i_type(0b000_0011, 1, 0b011, 2, 100);
        let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv64Instruction::Ld {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                imm: 100,
                rs2: Reg::Zero,
            }
        );
    }

    {
        // Negative offset (-4)
        let inst = make_i_type(0b000_0011, 1, 0b011, 2, 0xffc);
        let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv64Instruction::Ld {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                imm: -4,
                rs2: Reg::Zero,
            }
        );
    }
}

#[test]
fn test_lbu() {
    let inst = make_i_type(0b000_0011, 1, 0b100, 2, 100);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Lbu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            imm: 100,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_lhu() {
    let inst = make_i_type(0b000_0011, 1, 0b101, 2, 100);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Lhu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            imm: 100,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_lwu() {
    let inst = make_i_type(0b000_0011, 1, 0b110, 2, 100);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Lwu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            imm: 100,
            rs2: Reg::Zero,
        }
    );
}

// Jalr (I-type)

#[test]
fn test_jalr() {
    let inst = make_i_type(0b110_0111, 1, 0b000, 2, 100);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Jalr {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            imm: 100,
            rs2: Reg::Zero,
        }
    );
}

// S-type

#[test]
fn test_sb() {
    let inst = make_s_type(0b010_0011, 0b000, 2, 3, 100);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Sb {
            rs2: Reg::Gp,
            rs1: Reg::Sp,
            imm: 100
        }
    );
}

#[test]
fn test_sh() {
    let inst = make_s_type(0b010_0011, 0b001, 2, 3, 100);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Sh {
            rs2: Reg::Gp,
            rs1: Reg::Sp,
            imm: 100
        }
    );
}

#[test]
fn test_sw() {
    let inst = make_s_type(0b010_0011, 0b010, 2, 3, 100);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Sw {
            rs2: Reg::Gp,
            rs1: Reg::Sp,
            imm: 100
        }
    );
}

#[test]
fn test_sd() {
    {
        // Positive offset
        let inst = make_s_type(0b010_0011, 0b011, 2, 3, 100);
        let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv64Instruction::Sd {
                rs2: Reg::Gp,
                rs1: Reg::Sp,
                imm: 100
            }
        );
    }

    {
        // Negative offset
        let inst = make_s_type(0b010_0011, 0b011, 2, 3, -8);
        let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv64Instruction::Sd {
                rs2: Reg::Gp,
                rs1: Reg::Sp,
                imm: -8
            }
        );
    }
}

// B-type

#[test]
fn test_beq() {
    {
        // Positive offset
        let inst = make_b_type(0b110_0011, 0b000, 1, 2, 0x100);
        let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv64Instruction::Beq {
                rs1: Reg::Ra,
                rs2: Reg::Sp,
                imm: I24::from_i32(0x100)
            }
        );
    }

    {
        // Negative offset
        let inst = make_b_type(0b110_0011, 0b000, 1, 2, -8);
        let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv64Instruction::Beq {
                rs1: Reg::Ra,
                rs2: Reg::Sp,
                imm: I24::from_i32(-8)
            }
        );
    }
}

#[test]
fn test_bne() {
    let inst = make_b_type(0b110_0011, 0b001, 1, 2, 0x100);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Bne {
            rs1: Reg::Ra,
            rs2: Reg::Sp,
            imm: I24::from_i32(0x100)
        }
    );
}

#[test]
fn test_blt() {
    let inst = make_b_type(0b110_0011, 0b100, 1, 2, 0x100);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Blt {
            rs1: Reg::Ra,
            rs2: Reg::Sp,
            imm: I24::from_i32(0x100)
        }
    );
}

#[test]
fn test_bge() {
    let inst = make_b_type(0b110_0011, 0b101, 1, 2, 0x100);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Bge {
            rs1: Reg::Ra,
            rs2: Reg::Sp,
            imm: I24::from_i32(0x100)
        }
    );
}

#[test]
fn test_bltu() {
    let inst = make_b_type(0b110_0011, 0b110, 1, 2, 0x100);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Bltu {
            rs1: Reg::Ra,
            rs2: Reg::Sp,
            imm: I24::from_i32(0x100)
        }
    );
}

#[test]
fn test_bgeu() {
    let inst = make_b_type(0b110_0011, 0b111, 1, 2, 0x100);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Bgeu {
            rs1: Reg::Ra,
            rs2: Reg::Sp,
            imm: I24::from_i32(0x100)
        }
    );
}

// Lui (U-type)

#[test]
fn test_lui() {
    let inst = make_u_type(0b011_0111, 1, 0x1234_5000);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Lui {
            rd: Reg::Ra,
            imm: I24WithZeroedBits::from_i32(0x1234_5000),
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

// Auipc (U-type)

#[test]
fn test_auipc() {
    let inst = make_u_type(0b001_0111, 1, 0x1234_5000);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Auipc {
            rd: Reg::Ra,
            imm: I24WithZeroedBits::from_i32(0x1234_5000),
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

// Jal (J-type)

#[test]
fn test_jal() {
    {
        // Positive offset
        let inst = make_j_type(0b110_1111, 1, 0x1000);
        let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv64Instruction::Jal {
                rd: Reg::Ra,
                imm: I24::from_i32(0x1000),
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            }
        );
    }

    {
        // Negative offset
        let inst = make_j_type(0b110_1111, 1, -0x1000);
        let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv64Instruction::Jal {
                rd: Reg::Ra,
                imm: I24::from_i32(-0x1000),
                rs1: Reg::Zero,
                rs2: Reg::Zero,
            }
        );
    }
}

// Fence (I-type like, simplified for EM)

#[test]
fn test_fence_valid() {
    // Common full memory fence (fence iorw,iorw): pred=0xf, succ=0xf, fm=0
    let inst = 0x0ff0_000f_u32;
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Fence {
            pred: 15,
            succ: 15,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );

    // Original test case (custom pred/succ, fm=0 implicit)
    let inst = 0b000_1111_u32 | (3_u32 << 24u8) | (3_u32 << 20u8);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Fence {
            pred: 3,
            succ: 3,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_fence_invalid() {
    // Non-zero fm (reserved, must be 0)
    // fm=1
    let inst = 0x10f0_000f_u32;
    assert!(Rv64Instruction::<Reg<u64>>::try_decode(inst).is_none());

    // FENCE.I (funct3=1) - we explicitly reject it
    let inst = 0x0000_100f_u32;
    assert!(Rv64Instruction::<Reg<u64>>::try_decode(inst).is_none());

    // rd != 0
    // rd=1 (example)
    let inst = 0x0ff0_100f_u32;
    assert!(Rv64Instruction::<Reg<u64>>::try_decode(inst).is_none());

    // rs1 != 0
    // rs1=1 (example)
    let inst = 0x0ff8_000f_u32;
    assert!(Rv64Instruction::<Reg<u64>>::try_decode(inst).is_none());

    // Wrong funct3 for memory fence (not 0, not 1)
    // funct3=2 (example)
    let inst = 0x0ff0_200f_u32;
    assert!(Rv64Instruction::<Reg<u64>>::try_decode(inst).is_none());
}

#[test]
fn test_fence_tso() {
    // Canonical encoding: 0x8330000f
    let inst = 0x8330_000f_u32;
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::FenceTso {
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_fence_tso_invalid_variants() {
    // fm=8 but pred != 0b0011 - reserved
    let inst = 0x8230_000f_u32;
    assert!(Rv64Instruction::<Reg<u64>>::try_decode(inst).is_none());

    // fm=8 but succ != 0b0011 - reserved
    let inst = 0x8310_000f_u32;
    assert!(Rv64Instruction::<Reg<u64>>::try_decode(inst).is_none());

    // fm=1 - still reserved
    let inst = 0x10f0_000f_u32;
    assert!(Rv64Instruction::<Reg<u64>>::try_decode(inst).is_none());

    // fm=15 - reserved
    let inst = 0xfff0_000f_u32;
    assert!(Rv64Instruction::<Reg<u64>>::try_decode(inst).is_none());
}

// System instructions

#[test]
fn test_ecall() {
    let inst = 0b000000000000_00000_000_00000_1110011_u32;
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Ecall {
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

#[test]
fn test_ebreak() {
    let inst = 0b000000000001_00000_000_00000_1110011_u32;
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Ebreak {
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

// Unimplemented/illegal

#[test]
fn test_unimp() {
    // Standard unimp encoding
    let inst = 0xc000_1073_u32;
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Unimp {
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        }
    );
}

// Invalid instructions

#[test]
fn test_invalid() {
    {
        // Invalid opcode
        let inst = 0b111_1111_u32;
        let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst);
        assert!(decoded.is_none());
    }

    {
        // Invalid R-type funct7
        let inst = make_r_type(0b011_0011, 1, 0b000, 2, 3, 0b111_1111);
        let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst);
        assert!(decoded.is_none());
    }
}

// RV64E Variant Tests

#[test]
fn test_rv64e() {
    {
        // Valid RV64E instruction
        let inst = make_r_type(0b011_0011, 1, 0b000, 2, 3, 0b000_0000);
        let decoded = Rv64Instruction::<EReg<u64>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv64Instruction::Add {
                rd: EReg::Ra,
                rs1: EReg::Sp,
                rs2: EReg::Gp
            }
        );
    }

    {
        // Max valid register (15/A5) in RV64E
        let inst = make_r_type(0b011_0011, 15, 0b000, 14, 13, 0b000_0000);
        let decoded = Rv64Instruction::<EReg<u64>>::try_decode(inst).unwrap();
        assert_eq!(
            decoded,
            Rv64Instruction::Add {
                rd: EReg::A5,
                rs1: EReg::A4,
                rs2: EReg::A3
            }
        );
    }

    {
        // Invalid register (16 doesn't exist in RV64E)
        let inst = make_r_type(0b011_0011, 16, 0b000, 2, 3, 0b000_0000);
        let decoded = Rv64Instruction::<EReg<u64>>::try_decode(inst);
        assert!(decoded.is_none());
    }
}

// Edge Cases

#[test]
fn test_zero_register() {
    let inst = make_r_type(0b011_0011, 0, 0b000, 0, 0, 0b000_0000);
    let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
    assert_eq!(
        decoded,
        Rv64Instruction::Add {
            rd: Reg::Zero,
            rs1: Reg::Zero,
            rs2: Reg::Zero
        }
    );
}

#[test]
fn test_all_registers_rv64i() {
    for reg_num in 0..32 {
        let inst = make_r_type(0b011_0011, reg_num, 0b000, 1, 2, 0b000_0000);
        let decoded = Rv64Instruction::<Reg<u64>>::try_decode(inst).unwrap();
        assert!(
            matches!(decoded, Rv64Instruction::Add { .. }),
            "Register {reg_num} should be valid for RV64I"
        );
    }
}

#[test]
fn test_all_registers_rv64e() {
    // Valid registers (0-15)
    for reg_num in 0..16 {
        let inst = make_r_type(0b011_0011, reg_num, 0b000, 1, 2, 0b000_0000);
        let decoded = Rv64Instruction::<EReg<u64>>::try_decode(inst).unwrap();
        assert!(
            matches!(decoded, Rv64Instruction::Add { .. }),
            "Register {reg_num} should be valid for RV64E"
        );
    }

    // Invalid registers (16-31)
    for reg_num in 16..32 {
        let inst = make_r_type(0b011_0011, reg_num, 0b000, 1, 2, 0b000_0000);
        let decoded = Rv64Instruction::<EReg<u64>>::try_decode(inst);
        assert!(
            decoded.is_none(),
            "Register {reg_num} should be invalid for RV64E"
        );
    }
}
