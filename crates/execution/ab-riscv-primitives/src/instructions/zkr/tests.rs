use crate::instructions::Instruction;
use crate::instructions::zkr::{SEED_CSR_INDEX, ZkrInstruction};
use crate::registers::general_purpose::Reg;

#[test]
fn seed_csr_index_matches_specification() {
    // Zkr's `seed` CSR is a custom, unprivileged (bits `[9:8] = 0b00`) read/write CSR at `0x015`
    assert_eq!(SEED_CSR_INDEX, 0x015);
}

fn make_system(rd: u8, funct3: u8, rs1: u8, csr_index: u16) -> u32 {
    u32::from(0b111_0011u8)
        | (u32::from(rd) << 7u8)
        | (u32::from(funct3) << 12u8)
        | (u32::from(rs1) << 15u8)
        | (u32::from(csr_index) << 20u8)
}

/// `Zkr` defines no instructions of its own - it only adds the `seed` CSR, which is accessed
/// through `Zicsr` instructions (inherited as a dependency). Decoding a `csrrw` targeting `seed`
/// must therefore succeed exactly as it would for `ZicsrInstruction` itself.
#[test]
fn decodes_csrrw_targeting_seed() {
    let inst = make_system(10, 0b001, 11, SEED_CSR_INDEX);
    let decoded = ZkrInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZkrInstruction::Csrrw {
            rd: Reg::A0,
            rs1: Reg::A1,
            rs2: Reg::Zero,
            csr_index: SEED_CSR_INDEX,
        })
    );
}

#[test]
fn decodes_csrrs_with_arbitrary_csr_index() {
    // Decoding itself does not know or care which CSR index is targeted - that is resolved at
    // execution time
    let inst = make_system(5, 0b010, 0, 0x300);
    let decoded = ZkrInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZkrInstruction::Csrrs {
            rd: Reg::T0,
            rs1: Reg::Zero,
            rs2: Reg::Zero,
            csr_index: 0x300,
        })
    );
}

#[test]
fn try_decode_rejects_non_system_opcode() {
    let inst = make_system(10, 0b001, 11, SEED_CSR_INDEX) & !0b111_1111 | 0b011_0011;
    assert_eq!(ZkrInstruction::<Reg<u32>>::try_decode(inst), None);
}

#[test]
fn try_decode_rejects_unknown_funct3() {
    let inst = make_system(10, 0b100, 11, SEED_CSR_INDEX);
    assert_eq!(ZkrInstruction::<Reg<u32>>::try_decode(inst), None);
}
