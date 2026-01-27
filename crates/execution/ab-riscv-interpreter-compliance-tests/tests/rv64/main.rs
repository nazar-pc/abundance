#![feature(control_flow_ok)]
#![expect(incomplete_features, reason = "generic_const_exprs")]
// TODO: This feature is not actually used in this crate, but is added as a workaround for
//  https://github.com/rust-lang/rust/issues/139376
#![feature(generic_const_exprs)]

mod utils;

use crate::utils::run_tests;
use ab_riscv_interpreter_compliance_tests::RISCV_ARCH_TEST_REPO_PATH;
use ab_riscv_primitives::instruction::rv64::Rv64Instruction;
use ab_riscv_primitives::instruction::rv64::m::Rv64MInstruction;
use std::path::Path;

#[cfg_attr(miri, ignore)]
#[test]
fn rv64i() {
    run_tests(
        &Path::new(RISCV_ARCH_TEST_REPO_PATH).join("riscv-test-suite/rv64i_m/I/src"),
        |instruction_name, a, b, c| {
            Some(match instruction_name {
                "add" => Rv64Instruction::Add {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    rs2: c.into_reg(),
                },
                "sub" => Rv64Instruction::Sub {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    rs2: c.into_reg(),
                },
                "sll" => Rv64Instruction::Sll {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    rs2: c.into_reg(),
                },
                "slt" => Rv64Instruction::Slt {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    rs2: c.into_reg(),
                },
                "sltu" => Rv64Instruction::Sltu {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    rs2: c.into_reg(),
                },
                "xor" => Rv64Instruction::Xor {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    rs2: c.into_reg(),
                },
                "srl" => Rv64Instruction::Srl {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    rs2: c.into_reg(),
                },
                "sra" => Rv64Instruction::Sra {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    rs2: c.into_reg(),
                },
                "or" => Rv64Instruction::Or {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    rs2: c.into_reg(),
                },
                "and" => Rv64Instruction::And {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    rs2: c.into_reg(),
                },

                "addw" => Rv64Instruction::Addw {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    rs2: c.into_reg(),
                },
                "subw" => Rv64Instruction::Subw {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    rs2: c.into_reg(),
                },
                "sllw" => Rv64Instruction::Sllw {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    rs2: c.into_reg(),
                },
                "srlw" => Rv64Instruction::Srlw {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    rs2: c.into_reg(),
                },
                "sraw" => Rv64Instruction::Sraw {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    rs2: c.into_reg(),
                },

                "addi" => Rv64Instruction::Addi {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    imm: c.into_i16(),
                },
                "slti" => Rv64Instruction::Slti {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    imm: c.into_i16(),
                },
                "sltiu" => Rv64Instruction::Sltiu {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    imm: c.into_i16(),
                },
                "xori" => Rv64Instruction::Xori {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    imm: c.into_i16(),
                },
                "ori" => Rv64Instruction::Ori {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    imm: c.into_i16(),
                },
                "andi" => Rv64Instruction::Andi {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    imm: c.into_i16(),
                },
                "slli" => Rv64Instruction::Slli {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    shamt: c.into_i16().cast_unsigned() as u8,
                },
                "srli" => Rv64Instruction::Srli {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    shamt: c.into_i16().cast_unsigned() as u8,
                },
                "srai" => Rv64Instruction::Srai {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    shamt: c.into_i16().cast_unsigned() as u8,
                },

                "addiw" => Rv64Instruction::Addiw {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    imm: c.into_i16(),
                },
                "slliw" => Rv64Instruction::Slliw {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    shamt: c.into_i16().cast_unsigned() as u8,
                },
                "srliw" => Rv64Instruction::Srliw {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    shamt: c.into_i16().cast_unsigned() as u8,
                },
                "sraiw" => Rv64Instruction::Sraiw {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    shamt: c.into_i16().cast_unsigned() as u8,
                },

                "lb" => Rv64Instruction::Lb {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    imm: c.into_i16(),
                },
                "lh" => Rv64Instruction::Lh {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    imm: c.into_i16(),
                },
                "lw" => Rv64Instruction::Lw {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    imm: c.into_i16(),
                },
                "ld" => Rv64Instruction::Ld {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    imm: c.into_i16(),
                },
                "lbu" => Rv64Instruction::Lbu {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    imm: c.into_i16(),
                },
                "lhu" => Rv64Instruction::Lhu {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    imm: c.into_i16(),
                },
                "lwu" => Rv64Instruction::Lwu {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    imm: c.into_i16(),
                },

                "jalr" => Rv64Instruction::Jalr {
                    rd: a.into_reg(),
                    rs1: b.into_reg(),
                    imm: c.into_i16(),
                },

                "sb" => Rv64Instruction::Sb {
                    rs2: a.into_reg(),
                    rs1: b.into_reg(),
                    imm: c.into_i16(),
                },
                "sh" => Rv64Instruction::Sh {
                    rs2: a.into_reg(),
                    rs1: b.into_reg(),
                    imm: c.into_i16(),
                },
                "sw" => Rv64Instruction::Sw {
                    rs2: a.into_reg(),
                    rs1: b.into_reg(),
                    imm: c.into_i16(),
                },
                "sd" => Rv64Instruction::Sd {
                    rs2: a.into_reg(),
                    rs1: b.into_reg(),
                    imm: c.into_i16(),
                },

                "beq" => Rv64Instruction::Beq {
                    rs1: a.into_reg(),
                    rs2: b.into_reg(),
                    imm: c.into_i32(),
                },
                "bne" => Rv64Instruction::Bne {
                    rs1: a.into_reg(),
                    rs2: b.into_reg(),
                    imm: c.into_i32(),
                },
                "blt" => Rv64Instruction::Blt {
                    rs1: a.into_reg(),
                    rs2: b.into_reg(),
                    imm: c.into_i32(),
                },
                "bge" => Rv64Instruction::Bge {
                    rs1: a.into_reg(),
                    rs2: b.into_reg(),
                    imm: c.into_i32(),
                },
                "bltu" => Rv64Instruction::Bltu {
                    rs1: a.into_reg(),
                    rs2: b.into_reg(),
                    imm: c.into_i32(),
                },
                "bgeu" => Rv64Instruction::Bgeu {
                    rs1: a.into_reg(),
                    rs2: b.into_reg(),
                    imm: c.into_i32(),
                },

                "lui" => Rv64Instruction::Lui {
                    rd: a.into_reg(),
                    imm: b.into_i32(),
                },

                "auipc" => Rv64Instruction::Auipc {
                    rd: a.into_reg(),
                    imm: b.into_i32(),
                },

                "jal" => Rv64Instruction::Jal {
                    rd: a.into_reg(),
                    imm: b.into_i32(),
                },

                // There are no tests for these instructions right now:
                // "fence" => Rv64Instruction::Fence { pred, succ },
                //
                // "ecall" => Rv64Instruction::Ecall,
                // "ebreak" => Rv64Instruction::Ebreak,
                //
                // "unimp" => Rv64Instruction::Unimp,
                _ => {
                    panic!("Unknown instruction {instruction_name}");
                }
            })
        },
    );
}

#[cfg_attr(miri, ignore)]
#[test]
fn rv64m() {
    run_tests(
        &Path::new(RISCV_ARCH_TEST_REPO_PATH).join("riscv-test-suite/rv64i_m/M/src"),
        |instruction_name, rd, rs1, rs2| {
            let rd = rd.into_reg();
            let rs1 = rs1.into_reg();
            let rs2 = rs2.into_reg();

            Some(match instruction_name {
                "mul" => Rv64MInstruction::Mul { rd, rs1, rs2 },
                "mulh" => Rv64MInstruction::Mulh { rd, rs1, rs2 },
                "mulhsu" => Rv64MInstruction::Mulhsu { rd, rs1, rs2 },
                "mulhu" => Rv64MInstruction::Mulhu { rd, rs1, rs2 },
                "div" => Rv64MInstruction::Div { rd, rs1, rs2 },
                "divu" => Rv64MInstruction::Divu { rd, rs1, rs2 },
                "rem" => Rv64MInstruction::Rem { rd, rs1, rs2 },
                "remu" => Rv64MInstruction::Remu { rd, rs1, rs2 },
                "mulw" => Rv64MInstruction::Mulw { rd, rs1, rs2 },
                "divw" => Rv64MInstruction::Divw { rd, rs1, rs2 },
                "divuw" => Rv64MInstruction::Divuw { rd, rs1, rs2 },
                "remw" => Rv64MInstruction::Remw { rd, rs1, rs2 },
                "remuw" => Rv64MInstruction::Remuw { rd, rs1, rs2 },
                _ => {
                    panic!("Unknown instruction {instruction_name}");
                }
            })
        },
    );
}

#[cfg_attr(miri, ignore)]
#[test]
fn rv64b() {
    // TODO: test vectors for B extension are mostly broken:
    //  https://github.com/riscv-non-isa/riscv-arch-test/issues/860
    // if true {
    //     return;
    // }
    // run_tests(
    //     &Path::new(RISCV_ARCH_TEST_REPO_PATH).join("riscv-test-suite/rv64i_m/B/src"),
    //     |instruction_name, rd, rs1, rs2_shamt| {
    //         let shamt = rs2_shamt.into_u8();
    //         let rd = rd.into_reg();
    //         let rs1 = rs1.into_reg();
    //         let rs2 = Reg::from_bits(rs2_shamt.into_u8()).unwrap();
    //
    //         Some(match instruction_name {
    //             // Zba
    //             "add.uw" => FullRv64BInstruction::AddUw { rd, rs1, rs2 },
    //             "sh1add" => FullRv64BInstruction::Sh1add { rd, rs1, rs2 },
    //             "sh1add.uw" => FullRv64BInstruction::Sh1addUw { rd, rs1, rs2 },
    //             "sh2add" => FullRv64BInstruction::Sh2add { rd, rs1, rs2 },
    //             "sh2add.uw" => FullRv64BInstruction::Sh2addUw { rd, rs1, rs2 },
    //             "sh3add" => FullRv64BInstruction::Sh3add { rd, rs1, rs2 },
    //             "sh3add.uw" => FullRv64BInstruction::Sh3addUw { rd, rs1, rs2 },
    //             "slli.uw" => FullRv64BInstruction::SlliUw { rd, rs1, shamt },
    //             // Zbb
    //             "andn" => FullRv64BInstruction::Andn { rd, rs1, rs2 },
    //             "orn" => FullRv64BInstruction::Orn { rd, rs1, rs2 },
    //             "xnor" => FullRv64BInstruction::Xnor { rd, rs1, rs2 },
    //             "clz" => FullRv64BInstruction::Clz { rd, rs1 },
    //             "clzw" => FullRv64BInstruction::Clzw { rd, rs1 },
    //             "ctz" => FullRv64BInstruction::Ctz { rd, rs1 },
    //             "ctzw" => FullRv64BInstruction::Ctzw { rd, rs1 },
    //             "cpop" => FullRv64BInstruction::Cpop { rd, rs1 },
    //             "cpopw" => FullRv64BInstruction::Cpopw { rd, rs1 },
    //             "max" => FullRv64BInstruction::Max { rd, rs1, rs2 },
    //             "maxu" => FullRv64BInstruction::Maxu { rd, rs1, rs2 },
    //             "min" => FullRv64BInstruction::Min { rd, rs1, rs2 },
    //             "minu" => FullRv64BInstruction::Minu { rd, rs1, rs2 },
    //             "sext.b" => FullRv64BInstruction::Sextb { rd, rs1 },
    //             "sext.h" => FullRv64BInstruction::Sexth { rd, rs1 },
    //             "zext.h" => FullRv64BInstruction::Zexth { rd, rs1 },
    //             "rol" => FullRv64BInstruction::Rol { rd, rs1, rs2 },
    //             "rolw" => FullRv64BInstruction::Rolw { rd, rs1, rs2 },
    //             "ror" => FullRv64BInstruction::Ror { rd, rs1, rs2 },
    //             "rori" => FullRv64BInstruction::Rori { rd, rs1, shamt },
    //             "roriw" => FullRv64BInstruction::Roriw { rd, rs1, shamt },
    //             "rorw" => FullRv64BInstruction::Rorw { rd, rs1, rs2 },
    //             "orc.b" => FullRv64BInstruction::Orcb { rd, rs1 },
    //             "rev8" => FullRv64BInstruction::Rev8 { rd, rs1 },
    //             // Zbc
    //             "clmul" => FullRv64BInstruction::Clmul { rd, rs1, rs2 },
    //             "clmulh" => FullRv64BInstruction::Clmulh { rd, rs1, rs2 },
    //             "clmulr" => FullRv64BInstruction::Clmulr { rd, rs1, rs2 },
    //             // Zbs
    //             "bset" => FullRv64BInstruction::Bset { rd, rs1, rs2 },
    //             "bseti" => FullRv64BInstruction::Bseti { rd, rs1, shamt },
    //             "bclr" => FullRv64BInstruction::Bclr { rd, rs1, rs2 },
    //             "bclri" => FullRv64BInstruction::Bclri { rd, rs1, shamt },
    //             "binv" => FullRv64BInstruction::Binv { rd, rs1, rs2 },
    //             "binvi" => FullRv64BInstruction::Binvi { rd, rs1, shamt },
    //             "bext" => FullRv64BInstruction::Bext { rd, rs1, rs2 },
    //             "bexti" => FullRv64BInstruction::Bexti { rd, rs1, shamt },
    //             _ => {
    //                 panic!("Unknown instruction {instruction_name}");
    //             }
    //         })
    //     },
    // );
}

// TODO: Zknh extension uses completely different pattern in assembly files that is not currently
// supported #[cfg_attr(miri, ignore)]
// #[test]
// fn rv64zk() {
//     run_tests(
//         &Path::new(RISCV_ARCH_TEST_REPO_PATH).join("riscv-test-suite/rv64i_m/K/src"),
//         |instruction_name, rd, rs1, _| {
//             let rd = rd.into_reg();
//             let rs1 = rs1.into_reg();
//
//             Some(match instruction_name {
//                 // Zknh
//                 "sha256sig0" => Rv64ZknhInstruction::Sha256Sig0 { rd, rs1 },
//                 "sha256sig1" => Rv64ZknhInstruction::Sha256Sig1 { rd, rs1 },
//                 "sha256sum0" => Rv64ZknhInstruction::Sha256Sum0 { rd, rs1 },
//                 "sha256sum1" => Rv64ZknhInstruction::Sha256Sum1 { rd, rs1 },
//                 "sha512sig0" => Rv64ZknhInstruction::Sha512Sig0 { rd, rs1 },
//                 "sha512sig1" => Rv64ZknhInstruction::Sha512Sig1 { rd, rs1 },
//                 "sha512sum0" => Rv64ZknhInstruction::Sha512Sum0 { rd, rs1 },
//                 "sha512sum1" => Rv64ZknhInstruction::Sha512Sum1 { rd, rs1 },
//                 _ => {
//                     // Other instructions are not supported yet
//                     return None;
//                 }
//             })
//         },
//     );
// }
