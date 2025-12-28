use crate::instruction::{GenericInstruction, Rv64Instruction};
use crate::registers::{EReg, Reg};

// Helper to create instruction with opcode
const fn make_r_type(opcode: u32, rd: u8, funct3: u8, rs1: u8, rs2: u8, funct7: u8) -> u32 {
    opcode
        | ((rd as u32) << 7)
        | ((funct3 as u32) << 12)
        | ((rs1 as u32) << 15)
        | ((rs2 as u32) << 20)
        | ((funct7 as u32) << 25)
}

const fn make_i_type(opcode: u32, rd: u8, funct3: u8, rs1: u8, imm: u32) -> u32 {
    opcode
        | ((rd as u32) << 7)
        | ((funct3 as u32) << 12)
        | ((rs1 as u32) << 15)
        | ((imm & 0xfff) << 20)
}

const fn make_s_type(opcode: u32, funct3: u8, rs1: u8, rs2: u8, imm: i32) -> u32 {
    let imm = imm as u32;
    opcode
        | ((imm & 0x1f) << 7)
        | ((funct3 as u32) << 12)
        | ((rs1 as u32) << 15)
        | ((rs2 as u32) << 20)
        | ((imm >> 5) << 25)
}

const fn make_b_type(opcode: u32, funct3: u8, rs1: u8, rs2: u8, imm: i32) -> u32 {
    let imm = imm as u32;
    let imm11 = (imm >> 11) & 1;
    let imm4_1 = (imm >> 1) & 0xf;
    let imm10_5 = (imm >> 5) & 0x3f;
    let imm12 = (imm >> 12) & 1;

    opcode
        | (imm11 << 7)
        | (imm4_1 << 8)
        | ((funct3 as u32) << 12)
        | ((rs1 as u32) << 15)
        | ((rs2 as u32) << 20)
        | (imm10_5 << 25)
        | (imm12 << 31)
}

const fn make_u_type(opcode: u32, rd: u8, imm: u32) -> u32 {
    opcode | ((rd as u32) << 7) | (imm & 0xfffff000)
}

const fn make_j_type(opcode: u32, rd: u8, imm: i32) -> u32 {
    let imm = imm as u32;
    let imm19_12 = (imm >> 12) & 0xff;
    let imm11 = (imm >> 11) & 1;
    let imm10_1 = (imm >> 1) & 0x3ff;
    let imm20 = (imm >> 20) & 1;

    opcode | ((rd as u32) << 7) | (imm19_12 << 12) | (imm11 << 20) | (imm10_1 << 21) | (imm20 << 31)
}

// R-type

#[test]
fn test_add() {
    let inst = make_r_type(0b0110011, 1, 0b000, 2, 3, 0b0000000);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
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
    let inst = make_r_type(0b0110011, 5, 0b000, 6, 7, 0b0100000);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
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
    let inst = make_r_type(0b0110011, 10, 0b001, 11, 12, 0b0000000);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
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
    let inst = make_r_type(0b0110011, 1, 0b010, 2, 3, 0b0000000);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
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
    let inst = make_r_type(0b0110011, 1, 0b011, 2, 3, 0b0000000);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
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
    let inst = make_r_type(0b0110011, 1, 0b100, 2, 3, 0b0000000);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
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
    let inst = make_r_type(0b0110011, 1, 0b101, 2, 3, 0b0000000);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
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
    let inst = make_r_type(0b0110011, 1, 0b101, 2, 3, 0b0100000);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
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
    let inst = make_r_type(0b0110011, 1, 0b110, 2, 3, 0b0000000);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
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
    let inst = make_r_type(0b0110011, 1, 0b111, 2, 3, 0b0000000);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::And {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        }
    );
}

// M extension

#[test]
fn test_mul() {
    let inst = make_r_type(0b0110011, 1, 0b000, 2, 3, 0b0000001);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Mul {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        }
    );
}

#[test]
fn test_mulh() {
    let inst = make_r_type(0b0110011, 1, 0b001, 2, 3, 0b0000001);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Mulh {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        }
    );
}

#[test]
fn test_mulhsu() {
    let inst = make_r_type(0b0110011, 1, 0b010, 2, 3, 0b0000001);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Mulhsu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        }
    );
}

#[test]
fn test_mulhu() {
    let inst = make_r_type(0b0110011, 1, 0b011, 2, 3, 0b0000001);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Mulhu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        }
    );
}

#[test]
fn test_div() {
    let inst = make_r_type(0b0110011, 1, 0b100, 2, 3, 0b0000001);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Div {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        }
    );
}

#[test]
fn test_divu() {
    let inst = make_r_type(0b0110011, 1, 0b101, 2, 3, 0b0000001);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Divu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        }
    );
}

#[test]
fn test_rem() {
    let inst = make_r_type(0b0110011, 1, 0b110, 2, 3, 0b0000001);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Rem {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        }
    );
}

#[test]
fn test_remu() {
    let inst = make_r_type(0b0110011, 1, 0b111, 2, 3, 0b0000001);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Remu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        }
    );
}

// RV64 R-type W

#[test]
fn test_addw() {
    let inst = make_r_type(0b0111011, 1, 0b000, 2, 3, 0b0000000);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
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
    let inst = make_r_type(0b0111011, 1, 0b000, 2, 3, 0b0100000);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
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
    let inst = make_r_type(0b0111011, 1, 0b001, 2, 3, 0b0000000);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
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
    let inst = make_r_type(0b0111011, 1, 0b101, 2, 3, 0b0000000);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
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
    let inst = make_r_type(0b0111011, 1, 0b101, 2, 3, 0b0100000);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Sraw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        }
    );
}

#[test]
fn test_mulw() {
    let inst = make_r_type(0b0111011, 1, 0b000, 2, 3, 0b0000001);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Mulw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        }
    );
}

#[test]
fn test_divw() {
    let inst = make_r_type(0b0111011, 1, 0b100, 2, 3, 0b0000001);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Divw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        }
    );
}

#[test]
fn test_divuw() {
    let inst = make_r_type(0b0111011, 1, 0b101, 2, 3, 0b0000001);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Divuw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        }
    );
}

#[test]
fn test_remw() {
    let inst = make_r_type(0b0111011, 1, 0b110, 2, 3, 0b0000001);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Remw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp
        }
    );
}

#[test]
fn test_remuw() {
    let inst = make_r_type(0b0111011, 1, 0b111, 2, 3, 0b0000001);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Remuw {
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
        let decoded = Rv64Instruction::<Reg>::decode(inst);
        assert_eq!(
            decoded,
            Rv64Instruction::Addi {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                imm: 100
            }
        );
    }

    {
        // Negative immediate (-1)
        let inst = make_i_type(0b0010011, 1, 0b000, 2, 0xfff);
        let decoded = Rv64Instruction::<Reg>::decode(inst);
        assert_eq!(
            decoded,
            Rv64Instruction::Addi {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                imm: -1
            }
        );
    }

    {
        // Max positive 12-bit signed
        let inst = make_i_type(0b0010011, 1, 0b000, 2, 0x7ff);
        let decoded = Rv64Instruction::<Reg>::decode(inst);
        assert_eq!(
            decoded,
            Rv64Instruction::Addi {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                imm: 2047
            }
        );
    }

    {
        // Min negative 12-bit signed
        let inst = make_i_type(0b0010011, 1, 0b000, 2, 0x800);
        let decoded = Rv64Instruction::<Reg>::decode(inst);
        assert_eq!(
            decoded,
            Rv64Instruction::Addi {
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
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Slti {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            imm: 50
        }
    );
}

#[test]
fn test_sltiu() {
    let inst = make_i_type(0b0010011, 1, 0b011, 2, 50);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Sltiu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            imm: 50
        }
    );
}

#[test]
fn test_xori() {
    let inst = make_i_type(0b0010011, 1, 0b100, 2, 0xff);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Xori {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            imm: 0xff
        }
    );
}

#[test]
fn test_ori() {
    let inst = make_i_type(0b0010011, 1, 0b110, 2, 0xff);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Ori {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            imm: 0xff
        }
    );
}

#[test]
fn test_andi() {
    let inst = make_i_type(0b0010011, 1, 0b111, 2, 0xff);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Andi {
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
        let decoded = Rv64Instruction::<Reg>::decode(inst);
        assert_eq!(
            decoded,
            Rv64Instruction::Slli {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                shamt: 10
            }
        );
    }

    {
        // Mid shift (bit 5 set) - tests 6-bit shamt handling
        let inst = make_i_type(0b0010011, 1, 0b001, 2, 32);
        let decoded = Rv64Instruction::<Reg>::decode(inst);
        assert_eq!(
            decoded,
            Rv64Instruction::Slli {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                shamt: 32
            },
            "SLLI with shamt=32 should decode correctly"
        );
    }

    {
        // Max shift - all 6 bits set (tests funct6 is checked correctly)
        let inst = make_i_type(0b0010011, 1, 0b001, 2, 63);
        let decoded = Rv64Instruction::<Reg>::decode(inst);
        assert_eq!(
            decoded,
            Rv64Instruction::Slli {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                shamt: 63
            },
            "SLLI with shamt=63 should decode correctly (tests funct6 handling)"
        );
    }

    {
        // Invalid: bit 26 set (would pass with a buggy `funct7 & 0b111_1100` check)
        // This specifically tests that funct6 must be exactly 0b000000
        let shamt = 10u32;
        let inst = 0b0010011 | (1 << 7) | (0b001 << 12) | (2 << 15) | (shamt << 20) | (1 << 26);
        let decoded = Rv64Instruction::<Reg>::decode(inst);
        assert!(
            matches!(decoded, Rv64Instruction::Invalid(_)),
            "SLLI with bit 26 set should be invalid (catches funct7 & 0b111_1100 bug)"
        );
    }

    {
        // Invalid: bit 27 set
        let shamt = 10u32;
        let inst = 0b0010011 | (1 << 7) | (0b001 << 12) | (2 << 15) | (shamt << 20) | (1 << 27);
        let decoded = Rv64Instruction::<Reg>::decode(inst);
        assert!(
            matches!(decoded, Rv64Instruction::Invalid(_)),
            "SLLI with bit 27 set should be invalid"
        );
    }

    {
        // Invalid: multiple funct6 bits set
        let shamt = 10u32;
        let inst =
            0b0010011 | (1 << 7) | (0b001 << 12) | (2 << 15) | (shamt << 20) | (0b010000 << 26);
        let decoded = Rv64Instruction::<Reg>::decode(inst);
        assert!(
            matches!(decoded, Rv64Instruction::Invalid(_)),
            "SLLI with funct6=0b010000 (SRAI's funct6) should be invalid"
        );
    }
}

#[test]
fn test_srli() {
    {
        // Basic shift
        let inst = make_i_type(0b0010011, 1, 0b101, 2, 10);
        let decoded = Rv64Instruction::<Reg>::decode(inst);
        assert_eq!(
            decoded,
            Rv64Instruction::Srli {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                shamt: 10
            }
        );
    }

    {
        // Mid shift (bit 5 set) - tests 6-bit shamt handling
        let inst = make_i_type(0b0010011, 1, 0b101, 2, 32);
        let decoded = Rv64Instruction::<Reg>::decode(inst);
        assert_eq!(
            decoded,
            Rv64Instruction::Srli {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                shamt: 32
            },
            "SRLI with shamt=32 should decode correctly"
        );
    }

    {
        // Max shift - tests funct6 is checked correctly
        let inst = make_i_type(0b0010011, 1, 0b101, 2, 63);
        let decoded = Rv64Instruction::<Reg>::decode(inst);
        assert_eq!(
            decoded,
            Rv64Instruction::Srli {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                shamt: 63
            },
            "SRLI with shamt=63 should decode correctly (tests funct6 handling)"
        );
    }

    {
        // Invalid: bit 26 set (funct6 = 0b000001)
        let shamt = 10u32;
        let inst = 0b0010011 | (1 << 7) | (0b101 << 12) | (2 << 15) | (shamt << 20) | (1 << 26);
        let decoded = Rv64Instruction::<Reg>::decode(inst);
        assert!(
            matches!(decoded, Rv64Instruction::Invalid(_)),
            "SRLI with funct6=0b000001 should be invalid"
        );
    }

    {
        // Invalid: bits 26 and 27 set (funct6 = 0b000011)
        let shamt = 10u32;
        let inst = 0b0010011 | (1 << 7) | (0b101 << 12) | (2 << 15) | (shamt << 20) | (0b11 << 26);
        let decoded = Rv64Instruction::<Reg>::decode(inst);
        assert!(
            matches!(decoded, Rv64Instruction::Invalid(_)),
            "SRLI with funct6=0b000011 should be invalid"
        );
    }

    {
        // Invalid: bit 31 set (funct6 = 0b100000)
        let shamt = 10u32;
        let inst = 0b0010011 | (1 << 7) | (0b101 << 12) | (2 << 15) | (shamt << 20) | (1u32 << 31);
        let decoded = Rv64Instruction::<Reg>::decode(inst);
        assert!(
            matches!(decoded, Rv64Instruction::Invalid(_)),
            "SRLI with funct6=0b100000 should be invalid"
        );
    }
}

#[test]
fn test_srai() {
    {
        // Basic shift with correct funct6
        let shamt = 10u32;
        let inst =
            0b0010011 | (1 << 7) | (0b101 << 12) | (2 << 15) | (shamt << 20) | (0b010000 << 26);
        let decoded = Rv64Instruction::<Reg>::decode(inst);
        assert_eq!(
            decoded,
            Rv64Instruction::Srai {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                shamt: 10
            }
        );
    }

    {
        // Mid shift (bit 5 set) - tests 6-bit shamt handling
        let shamt = 32u32;
        let inst =
            0b0010011 | (1 << 7) | (0b101 << 12) | (2 << 15) | (shamt << 20) | (0b010000 << 26);
        let decoded = Rv64Instruction::<Reg>::decode(inst);
        assert_eq!(
            decoded,
            Rv64Instruction::Srai {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                shamt: 32
            },
            "SRAI with shamt=32 should decode correctly"
        );
    }

    {
        // Max shift - tests funct6 is checked correctly
        let shamt = 63u32;
        let inst =
            0b0010011 | (1 << 7) | (0b101 << 12) | (2 << 15) | (shamt << 20) | (0b010000 << 26);
        let decoded = Rv64Instruction::<Reg>::decode(inst);
        assert_eq!(
            decoded,
            Rv64Instruction::Srai {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                shamt: 63
            },
            "SRAI with shamt=63 should decode correctly (tests funct6 handling)"
        );
    }

    {
        // Invalid: bit 26 not set (this is SRLI, not SRAI)
        let shamt = 10u32;
        let inst = 0b0010011 | (1 << 7) | (0b101 << 12) | (2 << 15) | (shamt << 20);
        let decoded = Rv64Instruction::<Reg>::decode(inst);
        assert_eq!(
            decoded,
            Rv64Instruction::Srli {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                shamt: 10
            },
            "Without funct6 bit 4, this is SRLI"
        );
    }

    {
        // Invalid: extra bits set in funct6
        let shamt = 10u32;
        let inst =
            0b0010011 | (1 << 7) | (0b101 << 12) | (2 << 15) | (shamt << 20) | (0b010001 << 26);
        let decoded = Rv64Instruction::<Reg>::decode(inst);
        assert!(
            matches!(decoded, Rv64Instruction::Invalid(_)),
            "SRAI with extra funct6 bits should be invalid"
        );
    }

    {
        // Invalid: wrong funct6 pattern
        let shamt = 10u32;
        let inst =
            0b0010011 | (1 << 7) | (0b101 << 12) | (2 << 15) | (shamt << 20) | (0b100000 << 26);
        let decoded = Rv64Instruction::<Reg>::decode(inst);
        assert!(
            matches!(decoded, Rv64Instruction::Invalid(_)),
            "SRAI with funct6=0b100000 should be invalid"
        );
    }
}

// RV64 I-type W

#[test]
fn test_addiw() {
    let inst = make_i_type(0b0011011, 1, 0b000, 2, 100);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Addiw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            imm: 100
        }
    );
}

#[test]
fn test_slliw() {
    let inst = make_i_type(0b0011011, 1, 0b001, 2, 10);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Slliw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            shamt: 10
        }
    );
}

#[test]
fn test_srliw() {
    let inst = make_i_type(0b0011011, 1, 0b101, 2, 10);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Srliw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            shamt: 10
        }
    );
}

#[test]
fn test_sraiw() {
    let inst = make_i_type(0b0011011, 1, 0b101, 2, 10 | (0b0100000 << 5));
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Sraiw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            shamt: 10
        }
    );
}

// Loads (I-type)

#[test]
fn test_lb() {
    let inst = make_i_type(0b0000011, 1, 0b000, 2, 100);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Lb {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            imm: 100
        }
    );
}

#[test]
fn test_lh() {
    let inst = make_i_type(0b0000011, 1, 0b001, 2, 100);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Lh {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            imm: 100
        }
    );
}

#[test]
fn test_lw() {
    let inst = make_i_type(0b0000011, 1, 0b010, 2, 100);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Lw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            imm: 100
        }
    );
}

#[test]
fn test_ld() {
    {
        // Positive offset
        let inst = make_i_type(0b0000011, 1, 0b011, 2, 100);
        let decoded = Rv64Instruction::<Reg>::decode(inst);
        assert_eq!(
            decoded,
            Rv64Instruction::Ld {
                rd: Reg::Ra,
                rs1: Reg::Sp,
                imm: 100
            }
        );
    }

    {
        // Negative offset (-4)
        let inst = make_i_type(0b0000011, 1, 0b011, 2, 0xffc);
        let decoded = Rv64Instruction::<Reg>::decode(inst);
        assert_eq!(
            decoded,
            Rv64Instruction::Ld {
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
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Lbu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            imm: 100
        }
    );
}

#[test]
fn test_lhu() {
    let inst = make_i_type(0b0000011, 1, 0b101, 2, 100);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Lhu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            imm: 100
        }
    );
}

#[test]
fn test_lwu() {
    let inst = make_i_type(0b0000011, 1, 0b110, 2, 100);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Lwu {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            imm: 100
        }
    );
}

// Jalr (I-type)

#[test]
fn test_jalr() {
    let inst = make_i_type(0b1100111, 1, 0b000, 2, 100);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Jalr {
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
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Sb {
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            imm: 100
        }
    );
}

#[test]
fn test_sh() {
    let inst = make_s_type(0b0100011, 0b001, 2, 3, 100);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Sh {
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            imm: 100
        }
    );
}

#[test]
fn test_sw() {
    let inst = make_s_type(0b0100011, 0b010, 2, 3, 100);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Sw {
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            imm: 100
        }
    );
}

#[test]
fn test_sd() {
    {
        // Positive offset
        let inst = make_s_type(0b0100011, 0b011, 2, 3, 100);
        let decoded = Rv64Instruction::<Reg>::decode(inst);
        assert_eq!(
            decoded,
            Rv64Instruction::Sd {
                rs1: Reg::Sp,
                rs2: Reg::Gp,
                imm: 100
            }
        );
    }

    {
        // Negative offset
        let inst = make_s_type(0b0100011, 0b011, 2, 3, -8);
        let decoded = Rv64Instruction::<Reg>::decode(inst);
        assert_eq!(
            decoded,
            Rv64Instruction::Sd {
                rs1: Reg::Sp,
                rs2: Reg::Gp,
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
        let inst = make_b_type(0b1100011, 0b000, 1, 2, 0x100);
        let decoded = Rv64Instruction::<Reg>::decode(inst);
        assert_eq!(
            decoded,
            Rv64Instruction::Beq {
                rs1: Reg::Ra,
                rs2: Reg::Sp,
                imm: 0x100
            }
        );
    }

    {
        // Negative offset
        let inst = make_b_type(0b1100011, 0b000, 1, 2, -8);
        let decoded = Rv64Instruction::<Reg>::decode(inst);
        assert_eq!(
            decoded,
            Rv64Instruction::Beq {
                rs1: Reg::Ra,
                rs2: Reg::Sp,
                imm: -8
            }
        );
    }
}

#[test]
fn test_bne() {
    let inst = make_b_type(0b1100011, 0b001, 1, 2, 0x100);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Bne {
            rs1: Reg::Ra,
            rs2: Reg::Sp,
            imm: 0x100
        }
    );
}

#[test]
fn test_blt() {
    let inst = make_b_type(0b1100011, 0b100, 1, 2, 0x100);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Blt {
            rs1: Reg::Ra,
            rs2: Reg::Sp,
            imm: 0x100
        }
    );
}

#[test]
fn test_bge() {
    let inst = make_b_type(0b1100011, 0b101, 1, 2, 0x100);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Bge {
            rs1: Reg::Ra,
            rs2: Reg::Sp,
            imm: 0x100
        }
    );
}

#[test]
fn test_bltu() {
    let inst = make_b_type(0b1100011, 0b110, 1, 2, 0x100);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Bltu {
            rs1: Reg::Ra,
            rs2: Reg::Sp,
            imm: 0x100
        }
    );
}

#[test]
fn test_bgeu() {
    let inst = make_b_type(0b1100011, 0b111, 1, 2, 0x100);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Bgeu {
            rs1: Reg::Ra,
            rs2: Reg::Sp,
            imm: 0x100
        }
    );
}

// Lui (U-type)

#[test]
fn test_lui() {
    let inst = make_u_type(0b0110111, 1, 0x12345000);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Lui {
            rd: Reg::Ra,
            imm: 0x12345000u32 as i32
        }
    );
}

// Auipc (U-type)

#[test]
fn test_auipc() {
    let inst = make_u_type(0b0010111, 1, 0x12345000);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Auipc {
            rd: Reg::Ra,
            imm: 0x12345000u32 as i32
        }
    );
}

// Jal (J-type)

#[test]
fn test_jal() {
    {
        // Positive offset
        let inst = make_j_type(0b1101111, 1, 0x1000);
        let decoded = Rv64Instruction::<Reg>::decode(inst);
        assert_eq!(
            decoded,
            Rv64Instruction::Jal {
                rd: Reg::Ra,
                imm: 0x1000
            }
        );
    }

    {
        // Negative offset
        let inst = make_j_type(0b1101111, 1, -0x1000);
        let decoded = Rv64Instruction::<Reg>::decode(inst);
        assert_eq!(
            decoded,
            Rv64Instruction::Jal {
                rd: Reg::Ra,
                imm: -0x1000
            }
        );
    }
}

// Fence (I-type like, simplified for EM)

#[test]
fn test_fence() {
    // FENCE instruction format:
    // fm(4) | pred(4) | succ(4) | rs1(5) | funct3(3) | rd(5) | opcode(7)
    // Bits 28-31: fm, Bits 24-27: pred, Bits 20-23: succ
    let fm = 0u32;
    let pred = 3u32;
    let succ = 3u32;
    let inst = 0b0001111 | (fm << 28) | (pred << 24) | (succ << 20);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(
        decoded,
        Rv64Instruction::Fence {
            pred: 3,
            succ: 3,
            fm: 0
        }
    );
}

// System instructions

#[test]
fn test_ecall() {
    #[expect(clippy::unusual_byte_groupings)]
    let inst = 0b000000000000_00000_000_00000_1110011u32;
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(decoded, Rv64Instruction::Ecall);
}

#[test]
fn test_ebreak() {
    #[expect(clippy::unusual_byte_groupings)]
    let inst = 0b000000000001_00000_000_00000_1110011u32;
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(decoded, Rv64Instruction::Ebreak);
}

// Unimplemented/illegal

#[test]
fn test_unimp() {
    // Standard unimp encoding
    let inst = 0xc0001073u32;
    let decoded = Rv64Instruction::<Reg>::decode(inst);
    assert_eq!(decoded, Rv64Instruction::Unimp);
}

// Invalid instructions

#[test]
fn test_invalid() {
    {
        // Invalid opcode
        let inst = 0b1111111u32;
        let decoded = Rv64Instruction::<Reg>::decode(inst);
        assert_eq!(decoded, Rv64Instruction::Invalid(inst));
    }

    {
        // Invalid R-type funct7
        let inst = make_r_type(0b0110011, 1, 0b000, 2, 3, 0b1111111);
        let decoded = Rv64Instruction::<Reg>::decode(inst);
        assert_eq!(decoded, Rv64Instruction::Invalid(inst));
    }
}

// RV64E Variant Tests

#[test]
fn test_rv64e() {
    {
        // Valid RV64E instruction
        let inst = make_r_type(0b0110011, 1, 0b000, 2, 3, 0b0000000);
        let decoded = Rv64Instruction::<EReg>::decode(inst);
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
        let inst = make_r_type(0b0110011, 15, 0b000, 14, 13, 0b0000000);
        let decoded = Rv64Instruction::<EReg>::decode(inst);
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
        let inst = make_r_type(0b0110011, 16, 0b000, 2, 3, 0b0000000);
        let decoded = Rv64Instruction::<EReg>::decode(inst);
        assert_eq!(decoded, Rv64Instruction::Invalid(inst));
    }
}

// Edge Cases

#[test]
fn test_zero_register() {
    let inst = make_r_type(0b0110011, 0, 0b000, 0, 0, 0b0000000);
    let decoded = Rv64Instruction::<Reg>::decode(inst);
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
        let inst = make_r_type(0b0110011, reg_num, 0b000, 1, 2, 0b0000000);
        let decoded = Rv64Instruction::<Reg>::decode(inst);
        assert!(
            matches!(decoded, Rv64Instruction::Add { .. }),
            "Register {} should be valid for RV64I",
            reg_num
        );
    }
}

#[test]
fn test_all_registers_rv64e() {
    // Valid registers (0-15)
    for reg_num in 0..16 {
        let inst = make_r_type(0b0110011, reg_num, 0b000, 1, 2, 0b0000000);
        let decoded = Rv64Instruction::<EReg>::decode(inst);
        assert!(
            matches!(decoded, Rv64Instruction::Add { .. }),
            "Register {} should be valid for RV64E",
            reg_num
        );
    }

    // Invalid registers (16-31)
    for reg_num in 16..32 {
        let inst = make_r_type(0b0110011, reg_num, 0b000, 1, 2, 0b0000000);
        let decoded = Rv64Instruction::<EReg>::decode(inst);
        assert!(
            matches!(decoded, Rv64Instruction::Invalid(_)),
            "Register {} should be invalid for RV64E",
            reg_num
        );
    }
}
