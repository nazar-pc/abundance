use crate::instructions::Instruction;
use crate::instructions::zifencei::ZifenceiInstruction;
use crate::registers::general_purpose::Reg;

#[test]
fn test_fence_i() {
    let inst = 0x0000_100f_u32;
    let decoded = ZifenceiInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZifenceiInstruction::FenceI {
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

/// Per spec, `rd`/`rs1`/`imm` are reserved fields for `fence.i` and must be ignored by
/// implementations rather than checked - so a non-zero encoding must still decode as `fence.i`,
/// not be rejected as illegal.
#[test]
fn test_fence_i_reserved_fields_still_decodes() {
    // rd=1, rs1=2, imm=0x7ff, funct3=1
    let inst = 0x7ff1_108f_u32;
    let decoded = ZifenceiInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZifenceiInstruction::FenceI {
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_wrong_funct3_returns_none() {
    let inst = 0x0000_200f_u32;
    let decoded = ZifenceiInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_wrong_opcode_returns_none() {
    let inst = 0x0000_1013_u32;
    let decoded = ZifenceiInstruction::<Reg<u32>>::try_decode(inst);
    assert_eq!(decoded, None);
}
