pub(crate) const fn make_r_type(
    opcode: u8,
    rd: u8,
    funct3: u8,
    rs1: u8,
    rs2: u8,
    funct7: u8,
) -> u32 {
    u32::from(opcode)
        | (u32::from(rd) << 7u8)
        | (u32::from(funct3) << 12u8)
        | (u32::from(rs1) << 15u8)
        | (u32::from(rs2) << 20u8)
        | (u32::from(funct7) << 25u8)
}

pub(crate) const fn make_i_type(opcode: u8, rd: u8, funct3: u8, rs1: u8, imm: u32) -> u32 {
    u32::from(opcode)
        | (u32::from(rd) << 7u8)
        | (u32::from(funct3) << 12u8)
        | (u32::from(rs1) << 15u8)
        | ((imm & 0xfff) << 20u8)
}

pub(crate) fn make_i_type_with_shamt(
    opcode: u8,
    rd: u8,
    funct3: u8,
    rs1: u8,
    shamt: u8,
    funct6: u8,
) -> u32 {
    u32::from(opcode)
        | (u32::from(rd) << 7u8)
        | (u32::from(funct3) << 12u8)
        | (u32::from(rs1) << 15u8)
        | (u32::from(shamt) << 20u8)
        | (u32::from(funct6) << 26u8)
}

pub(crate) const fn make_s_type(opcode: u8, funct3: u8, rs1: u8, rs2: u8, imm: i32) -> u32 {
    let imm = imm.cast_unsigned();
    u32::from(opcode)
        | ((imm & 0x1f) << 7u8)
        | (u32::from(funct3) << 12u8)
        | (u32::from(rs1) << 15u8)
        | (u32::from(rs2) << 20u8)
        | ((imm >> 5u8) << 25u8)
}

pub(crate) const fn make_b_type(opcode: u8, funct3: u8, rs1: u8, rs2: u8, imm: i32) -> u32 {
    let imm = imm.cast_unsigned();
    let imm11 = (imm >> 11u8) & 1;
    let imm4_1 = (imm >> 1u8) & 0xf;
    let imm10_5 = (imm >> 5u8) & 0x3f;
    let imm12 = (imm >> 12u8) & 1;

    u32::from(opcode)
        | (imm11 << 7u8)
        | (imm4_1 << 8u8)
        | (u32::from(funct3) << 12u8)
        | (u32::from(rs1) << 15u8)
        | (u32::from(rs2) << 20u8)
        | (imm10_5 << 25u8)
        | (imm12 << 31u8)
}

pub(crate) const fn make_u_type(opcode: u8, rd: u8, imm: u32) -> u32 {
    u32::from(opcode) | (u32::from(rd) << 7u8) | (imm & 0xffff_f000)
}

pub(crate) const fn make_j_type(opcode: u8, rd: u8, imm: i32) -> u32 {
    let imm = imm.cast_unsigned();
    let imm19_12 = (imm >> 12u8) & 0xff;
    let imm11 = (imm >> 11u8) & 1;
    let imm10_1 = (imm >> 1u8) & 0x3ff;
    let imm20 = (imm >> 20u8) & 1;

    u32::from(opcode)
        | (u32::from(rd) << 7u8)
        | (imm19_12 << 12u8)
        | (imm11 << 20u8)
        | (imm10_1 << 21u8)
        | (imm20 << 31u8)
}
