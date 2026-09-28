use crate::instructions::Instruction;
use crate::instructions::v::zvexx::ZveXxInstruction;
use crate::instructions::v::{VRegGroupSize, Vlmul};
use crate::registers::general_purpose::Reg;

#[test]
fn widening_register_count_values() {
    // EMUL = 2 * LMUL:
    // Mf8 (1/8) -> 2/8 = 1/4 -> 1 reg
    // Mf4 (1/4) -> 2/4 = 1/2 -> 1 reg
    // Mf2 (1/2) -> 2/2 = 1   -> 1 reg
    // M1 (1)    -> 2/1 = 2   -> 2 regs
    // M2 (2)    -> 4/1 = 4   -> 4 regs
    // M4 (4)    -> 8/1 = 8   -> 8 regs
    // M8 (8)    -> 16/1 = 16 -> None (illegal)
    assert_eq!(
        Vlmul::widening_register_count(Vlmul::Mf8),
        Some(VRegGroupSize::R1)
    );
    assert_eq!(
        Vlmul::widening_register_count(Vlmul::Mf4),
        Some(VRegGroupSize::R1)
    );
    assert_eq!(
        Vlmul::widening_register_count(Vlmul::Mf2),
        Some(VRegGroupSize::R1)
    );
    assert_eq!(
        Vlmul::widening_register_count(Vlmul::M1),
        Some(VRegGroupSize::R2)
    );
    assert_eq!(
        Vlmul::widening_register_count(Vlmul::M2),
        Some(VRegGroupSize::R4)
    );
    assert_eq!(
        Vlmul::widening_register_count(Vlmul::M4),
        Some(VRegGroupSize::R8)
    );
    assert_eq!(Vlmul::widening_register_count(Vlmul::M8), None);
}

/// OP-V encoding with `vm = 0`
const fn op_v_masked(funct6: u32, vs2: u32, vs1: u32, funct3: u32, vd: u32) -> u32 {
    (funct6 << 26) | (vs2 << 20) | (vs1 << 15) | (funct3 << 12) | (vd << 7) | 0b101_0111
}

#[test]
fn masked_v0_data_source_is_reserved() {
    const OPIVV: u32 = 0b000;
    const OPMVV: u32 = 0b010;
    const OPIVX: u32 = 0b100;

    // Bit position of the register field that holds `v0`
    const VS2: u32 = 20;
    const VS1: u32 = 15;
    const VS3: u32 = 7;

    let cases = [
        ("vadd.vv vs2", op_v_masked(0b00_0000, 0, 1, OPIVV, 8), VS2),
        ("vadd.vv vs1", op_v_masked(0b00_0000, 1, 0, OPIVV, 8), VS1),
        ("vadd.vx vs2", op_v_masked(0b00_0000, 0, 1, OPIVX, 8), VS2),
        ("vmseq.vv vs1", op_v_masked(0b01_1000, 1, 0, OPIVV, 8), VS1),
        ("vadc.vvm vs2", op_v_masked(0b01_0000, 0, 1, OPIVV, 8), VS2),
        ("vsaddu.vv vs1", op_v_masked(0b10_0000, 1, 0, OPIVV, 8), VS1),
        ("vmul.vv vs1", op_v_masked(0b10_0101, 1, 0, OPMVV, 8), VS1),
        ("vwaddu.vv vs1", op_v_masked(0b11_0000, 1, 0, OPMVV, 8), VS1),
        ("vnsrl.wv vs2", op_v_masked(0b10_1100, 0, 1, OPIVV, 8), VS2),
        (
            "vzext.vf2 vs2",
            op_v_masked(0b01_0010, 0, 0b0_0110, OPMVV, 8),
            VS2,
        ),
        (
            "vredsum.vs vs1",
            op_v_masked(0b00_0000, 1, 0, OPMVV, 8),
            VS1,
        ),
        (
            "vslideup.vx vs2",
            op_v_masked(0b00_1110, 0, 1, OPIVX, 8),
            VS2,
        ),
        (
            "vrgather.vv vs1",
            op_v_masked(0b00_1100, 1, 0, OPIVV, 8),
            VS1,
        ),
        (
            "vmerge.vvm vs1",
            op_v_masked(0b01_0111, 1, 0, OPIVV, 8),
            VS1,
        ),
        // `vluxei8.v v8, (x1), v0, v0.t`
        (
            "vluxei8.v vs2",
            (0b01 << 26) | (1 << 15) | (8 << 7) | 0b000_0111,
            VS2,
        ),
        // `vse8.v v0, (x1), v0.t`
        ("vse8.v vs3", (1 << 15) | 0b010_0111, VS3),
        // `vsuxei8.v v8, (x1), v0, v0.t`
        (
            "vsuxei8.v vs2",
            (0b01 << 26) | (1 << 15) | (8 << 7) | 0b010_0111,
            VS2,
        ),
    ];
    for (name, instruction, v0_field) in cases {
        assert_eq!(
            ZveXxInstruction::<Reg<u64>>::try_decode(instruction),
            None,
            "{name}"
        );
        assert!(
            ZveXxInstruction::<Reg<u64>>::try_decode(instruction | (4 << v0_field)).is_some(),
            "{name}"
        );
    }

    // Sources that are masks themselves have EEW=1 as well
    for (name, instruction) in [
        ("vcpop.m", op_v_masked(0b01_0000, 0, 0b1_0000, OPMVV, 1)),
        ("vfirst.m", op_v_masked(0b01_0000, 0, 0b1_0001, OPMVV, 1)),
        ("vmsbf.m", op_v_masked(0b01_0100, 0, 0b0_0001, OPMVV, 8)),
        ("viota.m", op_v_masked(0b01_0100, 0, 0b1_0000, OPMVV, 8)),
    ] {
        assert!(
            ZveXxInstruction::<Reg<u64>>::try_decode(instruction).is_some(),
            "{name}"
        );
    }
}
