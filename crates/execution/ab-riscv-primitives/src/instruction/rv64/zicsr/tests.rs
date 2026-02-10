use crate::instruction::Instruction;
use crate::instruction::rv64::zicsr::Rv64ZicsrInstruction;
use crate::instruction::test_utils::make_i_type;
use crate::registers::Reg;

#[test]
fn test_csrrw() {
    let inst = make_i_type(0b1110011, 1, 0b001, 2, 0x305);
    let decoded = Rv64ZicsrInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZicsrInstruction::Csrrw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            csr: 0x305
        })
    );
}

#[test]
fn test_csrrs() {
    let inst = make_i_type(0b1110011, 3, 0b010, 4, 0x341);
    let decoded = Rv64ZicsrInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZicsrInstruction::Csrrs {
            rd: Reg::Gp,
            rs1: Reg::Tp,
            csr: 0x341
        })
    );
}

#[test]
fn test_csrrc() {
    let inst = make_i_type(0b1110011, 5, 0b011, 6, 0x300);
    let decoded = Rv64ZicsrInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZicsrInstruction::Csrrc {
            rd: Reg::T0,
            rs1: Reg::T1,
            csr: 0x300
        })
    );
}

#[test]
fn test_csrrwi() {
    let inst = make_i_type(0b1110011, 7, 0b101, 0b10101, 0x7c0);
    let decoded = Rv64ZicsrInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZicsrInstruction::Csrrwi {
            rd: Reg::T2,
            zimm: 0b10101,
            csr: 0x7c0
        })
    );
}

#[test]
fn test_csrrsi() {
    let inst = make_i_type(0b1110011, 8, 0b110, 0b00001, 0x7c1);
    let decoded = Rv64ZicsrInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZicsrInstruction::Csrrsi {
            rd: Reg::S0,
            zimm: 0b00001,
            csr: 0x7c1
        })
    );
}

#[test]
fn test_csrrci() {
    let inst = make_i_type(0b1110011, 9, 0b111, 0b11111, 0x7c2);
    let decoded = Rv64ZicsrInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZicsrInstruction::Csrrci {
            rd: Reg::S1,
            zimm: 0b11111,
            csr: 0x7c2
        })
    );
}

#[test]
fn test_csrrw_nop_like_encoding() {
    let inst = make_i_type(0b1110011, 0, 0b001, 0, 0x000);
    let decoded = Rv64ZicsrInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZicsrInstruction::Csrrw {
            rd: Reg::Zero,
            rs1: Reg::Zero,
            csr: 0x000
        })
    );
}

#[test]
fn test_csrrwi_nop_like_encoding() {
    let inst = make_i_type(0b1110011, 0, 0b101, 0, 0x000);
    let decoded = Rv64ZicsrInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(Rv64ZicsrInstruction::Csrrwi {
            rd: Reg::Zero,
            zimm: 0,
            csr: 0x000
        })
    );
}

#[test]
fn test_invalid_opcode() {
    let inst = make_i_type(0b0000000, 1, 0b001, 2, 0x305);
    let decoded = Rv64ZicsrInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_invalid_funct3() {
    let inst = make_i_type(0b1110011, 1, 0b000, 2, 0x305);
    let decoded = Rv64ZicsrInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}
