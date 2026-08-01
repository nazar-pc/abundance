use crate::instructions::Instruction;
use crate::instructions::zawrs::ZawrsInstruction;
use crate::registers::general_purpose::Reg;

fn make_system(rd: u8, funct3: u8, rs1: u8, imm: u16) -> u32 {
    u32::from(0b111_0011u8)
        | (u32::from(rd) << 7u8)
        | (u32::from(funct3) << 12u8)
        | (u32::from(rs1) << 15u8)
        | (u32::from(imm) << 20u8)
}

#[test]
fn test_wrs_nto() {
    let inst = make_system(0, 0, 0, 0x00d);
    let decoded = ZawrsInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZawrsInstruction::WrsNto {
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_wrs_sto() {
    let inst = make_system(0, 0, 0, 0x01d);
    let decoded = ZawrsInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(
        decoded,
        Some(ZawrsInstruction::WrsSto {
            rs1: Reg::Zero,
            rs2: Reg::Zero,
        })
    );
}

#[test]
fn test_nonzero_rd_returns_none() {
    let inst = make_system(1, 0, 0, 0x00d);
    let decoded = ZawrsInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_nonzero_rs1_returns_none() {
    let inst = make_system(0, 0, 1, 0x00d);
    let decoded = ZawrsInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_wrong_funct3_returns_none() {
    let inst = make_system(0, 1, 0, 0x00d);
    let decoded = ZawrsInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_unknown_imm_returns_none() {
    let inst = make_system(0, 0, 0, 0x000);
    let decoded = ZawrsInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}

#[test]
fn test_wrong_opcode_returns_none() {
    let inst = (make_system(0, 0, 0, 0x00d) & !0b111_1111) | 0b011_0011;
    let decoded = ZawrsInstruction::<Reg<u64>>::try_decode(inst);
    assert_eq!(decoded, None);
}
