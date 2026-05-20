use crate::instructions::Instruction;
use crate::instructions::test_utils::make_i_type;
use crate::instructions::zicsr::ZicsrInstruction;
use crate::registers::general_purpose::Reg;

#[test]
fn test_csrrw() {
    let inst = make_i_type(0b111_0011, 1, 0b001, 2, 0x305);
    let decoded = ZicsrInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZicsrInstruction::Csrrw {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            csr_index: 0x305,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_csrrs() {
    let inst = make_i_type(0b111_0011, 3, 0b010, 4, 0x341);
    let decoded = ZicsrInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZicsrInstruction::Csrrs {
            rd: Reg::Gp,
            rs1: Reg::Tp,
            csr_index: 0x341,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_csrrc() {
    let inst = make_i_type(0b111_0011, 5, 0b011, 6, 0x300);
    let decoded = ZicsrInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZicsrInstruction::Csrrc {
            rd: Reg::T0,
            rs1: Reg::T1,
            csr_index: 0x300,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_csrrwi() {
    let inst = make_i_type(0b111_0011, 7, 0b101, 0b10101, 0x7c0);
    let decoded = ZicsrInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZicsrInstruction::Csrrwi {
            rd: Reg::T2,
            zimm: 0b10101,
            csr_index: 0x7c0,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_csrrsi() {
    let inst = make_i_type(0b111_0011, 8, 0b110, 0b00001, 0x7c1);
    let decoded = ZicsrInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZicsrInstruction::Csrrsi {
            rd: Reg::S0,
            zimm: 0b00001,
            csr_index: 0x7c1,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_csrrci() {
    let inst = make_i_type(0b111_0011, 9, 0b111, 0b11111, 0x7c2);
    let decoded = ZicsrInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZicsrInstruction::Csrrci {
            rd: Reg::S1,
            zimm: 0b11111,
            csr_index: 0x7c2,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_csrrw_nop_like_encoding() {
    let inst = make_i_type(0b111_0011, 0, 0b001, 0, 0x000);
    let decoded = ZicsrInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZicsrInstruction::Csrrw {
            rd: Reg::Zero,
            rs1: Reg::Zero,
            csr_index: 0x000,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_csrrwi_nop_like_encoding() {
    let inst = make_i_type(0b111_0011, 0, 0b101, 0, 0x000);
    let decoded = ZicsrInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZicsrInstruction::Csrrwi {
            rd: Reg::Zero,
            zimm: 0,
            csr_index: 0x000,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_invalid_opcode() {
    let inst = make_i_type(0b000_0000, 1, 0b001, 2, 0x305);
    let decoded = ZicsrInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_invalid_funct3() {
    let inst = make_i_type(0b111_0011, 1, 0b000, 2, 0x305);
    let decoded = ZicsrInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}
