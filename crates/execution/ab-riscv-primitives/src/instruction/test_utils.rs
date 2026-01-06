pub(crate) const fn make_r_type(
    opcode: u8,
    rd: u8,
    funct3: u8,
    rs1: u8,
    rs2: u8,
    funct7: u8,
) -> u32 {
    (opcode as u32)
        | ((rd as u32) << 7)
        | ((funct3 as u32) << 12)
        | ((rs1 as u32) << 15)
        | ((rs2 as u32) << 20)
        | ((funct7 as u32) << 25)
}

pub(crate) const fn make_i_type(opcode: u8, rd: u8, funct3: u8, rs1: u8, imm: u32) -> u32 {
    (opcode as u32)
        | ((rd as u32) << 7)
        | ((funct3 as u32) << 12)
        | ((rs1 as u32) << 15)
        | ((imm & 0xfff) << 20)
}

pub(crate) fn make_i_type_with_shamt(
    opcode: u8,
    rd: u8,
    funct3: u8,
    rs1: u8,
    shamt: u8,
    funct6: u8,
) -> u32 {
    (opcode as u32)
        | ((rd as u32) << 7)
        | ((funct3 as u32) << 12)
        | ((rs1 as u32) << 15)
        | ((shamt as u32) << 20)
        | ((funct6 as u32) << 26)
}

pub(crate) const fn make_s_type(opcode: u8, funct3: u8, rs1: u8, rs2: u8, imm: i32) -> u32 {
    let imm = imm.cast_unsigned();
    (opcode as u32)
        | ((imm & 0x1f) << 7)
        | ((funct3 as u32) << 12)
        | ((rs1 as u32) << 15)
        | ((rs2 as u32) << 20)
        | ((imm >> 5) << 25)
}

pub(crate) const fn make_b_type(opcode: u8, funct3: u8, rs1: u8, rs2: u8, imm: i32) -> u32 {
    let imm = imm.cast_unsigned();
    let imm11 = (imm >> 11) & 1;
    let imm4_1 = (imm >> 1) & 0xf;
    let imm10_5 = (imm >> 5) & 0x3f;
    let imm12 = (imm >> 12) & 1;

    (opcode as u32)
        | (imm11 << 7)
        | (imm4_1 << 8)
        | ((funct3 as u32) << 12)
        | ((rs1 as u32) << 15)
        | ((rs2 as u32) << 20)
        | (imm10_5 << 25)
        | (imm12 << 31)
}

pub(crate) const fn make_u_type(opcode: u8, rd: u8, imm: u32) -> u32 {
    (opcode as u32) | ((rd as u32) << 7) | (imm & 0xfffff000)
}

pub(crate) const fn make_j_type(opcode: u8, rd: u8, imm: i32) -> u32 {
    let imm = imm.cast_unsigned();
    let imm19_12 = (imm >> 12) & 0xff;
    let imm11 = (imm >> 11) & 1;
    let imm10_1 = (imm >> 1) & 0x3ff;
    let imm20 = (imm >> 20) & 1;

    (opcode as u32)
        | ((rd as u32) << 7)
        | (imm19_12 << 12)
        | (imm11 << 20)
        | (imm10_1 << 21)
        | (imm20 << 31)
}
