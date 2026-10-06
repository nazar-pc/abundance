extern crate alloc;

use crate::hart::{BasicVectorHart, VectorHartConfig};
use crate::instructions::Instruction;
use crate::instructions::test_utils::TestVectorHart;
use crate::instructions::v::zvexx::ZveXxInstruction;
use crate::instructions::v::{Elen, VRegGroupSize, Vlen, Vlmul, Vsew, Vtype};
use crate::registers::general_purpose::Reg;
use alloc::string::{String, ToString};

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
            ZveXxInstruction::<TestVectorHart>::try_decode(instruction),
            None,
            "{name}"
        );
        assert!(
            ZveXxInstruction::<TestVectorHart>::try_decode(instruction | (4 << v0_field)).is_some(),
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
            ZveXxInstruction::<TestVectorHart>::try_decode(instruction).is_some(),
            "{name}"
        );
    }
}

const VSEWS: [Vsew; 4] = [Vsew::E8, Vsew::E16, Vsew::E32, Vsew::E64];
const VLMULS: [Vlmul; 7] = [
    Vlmul::Mf8,
    Vlmul::Mf4,
    Vlmul::Mf2,
    Vlmul::M1,
    Vlmul::M2,
    Vlmul::M4,
    Vlmul::M8,
];

/// Every valid `vtype` with `vta = vma = false`
fn all_vtypes<Hart>() -> impl Iterator<Item = Vtype<Hart>>
where
    Hart: VectorHartConfig,
{
    VSEWS.into_iter().flat_map(|vsew| {
        VLMULS.into_iter().filter_map(move |vlmul| {
            let raw = u64::from(vlmul.to_bits()) | (u64::from(vsew.to_bits()) << 3);
            Vtype::from_raw::<Reg<u64>>(raw)
        })
    })
}

#[test]
fn vtype_vlmax() {
    fn check<const ELEN: Elen, const VLEN: Vlen>() {
        let mut count = 0;
        for vtype in all_vtypes::<BasicVectorHart<Reg<u64>, ELEN, VLEN>>() {
            count += 1;
            let vlmax = u32::from(vtype.vlmax());
            assert!(vlmax >= 1, "{vtype:?}");
            // Mask registers hold one bit per element in a single register
            assert!(vlmax <= u32::from(VLEN), "{vtype:?}");
            // All elements fit into the register group
            let group_regs = u32::from(vtype.vlmul().register_count().get());
            assert!(
                vlmax * u32::from(vtype.vsew().bytes_width()) <= group_regs * VLEN.bytes(),
                "{vtype:?}"
            );
        }
        assert!(count > 0);
    }

    check::<{ Elen::L64 }, { Vlen::L64 }>();
    check::<{ Elen::L64 }, { Vlen::L128 }>();
    check::<{ Elen::L64 }, { Vlen::L256 }>();
    check::<{ Elen::L64 }, { Vlen::L1024 }>();
    check::<{ Elen::L64 }, { Vlen::L65_536 }>();
    check::<{ Elen::L32 }, { Vlen::L32 }>();
    check::<{ Elen::L32 }, { Vlen::L128 }>();
}

/// Vector load with `vm = 0`, `rs1 = x1` and the given fields
const fn load_masked(nf: u32, mop: u32, rs2: u32, width: u32, vd: u32) -> u32 {
    (nf << 29) | (mop << 26) | (rs2 << 20) | (1 << 15) | (width << 12) | (vd << 7) | 0b000_0111
}

/// Mnemonic of an instruction, the first word of its assembly
fn mnemonic(instruction: ZveXxInstruction<TestVectorHart>) -> String {
    instruction
        .to_string()
        .split(' ')
        .next()
        .unwrap_or_default()
        .to_string()
}

#[test]
fn masked_v0_destination_is_reserved() {
    const OPIVV: u32 = 0b000;
    const OPMVV: u32 = 0b010;
    const OPIVI: u32 = 0b011;
    const OPIVX: u32 = 0b100;
    const OPMVX: u32 = 0b110;

    // Every instruction whose destination may not overlap the mask it reads, all with `vd = v0`
    // and `vm = 0`
    let reserved = [
        // arith
        ("vadd.vv", op_v_masked(0b00_0000, 4, 1, OPIVV, 0)),
        ("vadd.vi", op_v_masked(0b00_0000, 4, 1, OPIVI, 0)),
        ("vadd.vx", op_v_masked(0b00_0000, 4, 1, OPIVX, 0)),
        ("vsub.vv", op_v_masked(0b00_0010, 4, 1, OPIVV, 0)),
        ("vsub.vx", op_v_masked(0b00_0010, 4, 1, OPIVX, 0)),
        ("vrsub.vi", op_v_masked(0b00_0011, 4, 1, OPIVI, 0)),
        ("vrsub.vx", op_v_masked(0b00_0011, 4, 1, OPIVX, 0)),
        ("vminu.vv", op_v_masked(0b00_0100, 4, 1, OPIVV, 0)),
        ("vminu.vx", op_v_masked(0b00_0100, 4, 1, OPIVX, 0)),
        ("vmin.vv", op_v_masked(0b00_0101, 4, 1, OPIVV, 0)),
        ("vmin.vx", op_v_masked(0b00_0101, 4, 1, OPIVX, 0)),
        ("vmaxu.vv", op_v_masked(0b00_0110, 4, 1, OPIVV, 0)),
        ("vmaxu.vx", op_v_masked(0b00_0110, 4, 1, OPIVX, 0)),
        ("vmax.vv", op_v_masked(0b00_0111, 4, 1, OPIVV, 0)),
        ("vmax.vx", op_v_masked(0b00_0111, 4, 1, OPIVX, 0)),
        ("vand.vv", op_v_masked(0b00_1001, 4, 1, OPIVV, 0)),
        ("vand.vi", op_v_masked(0b00_1001, 4, 1, OPIVI, 0)),
        ("vand.vx", op_v_masked(0b00_1001, 4, 1, OPIVX, 0)),
        ("vor.vv", op_v_masked(0b00_1010, 4, 1, OPIVV, 0)),
        ("vor.vi", op_v_masked(0b00_1010, 4, 1, OPIVI, 0)),
        ("vor.vx", op_v_masked(0b00_1010, 4, 1, OPIVX, 0)),
        ("vxor.vv", op_v_masked(0b00_1011, 4, 1, OPIVV, 0)),
        ("vxor.vi", op_v_masked(0b00_1011, 4, 1, OPIVI, 0)),
        ("vxor.vx", op_v_masked(0b00_1011, 4, 1, OPIVX, 0)),
        ("vsll.vv", op_v_masked(0b100_101, 4, 1, OPIVV, 0)),
        ("vsll.vi", op_v_masked(0b100_101, 4, 1, OPIVI, 0)),
        ("vsll.vx", op_v_masked(0b100_101, 4, 1, OPIVX, 0)),
        ("vsrl.vv", op_v_masked(0b101_000, 4, 1, OPIVV, 0)),
        ("vsrl.vi", op_v_masked(0b101_000, 4, 1, OPIVI, 0)),
        ("vsrl.vx", op_v_masked(0b101_000, 4, 1, OPIVX, 0)),
        ("vsra.vv", op_v_masked(0b101_001, 4, 1, OPIVV, 0)),
        ("vsra.vi", op_v_masked(0b101_001, 4, 1, OPIVI, 0)),
        ("vsra.vx", op_v_masked(0b101_001, 4, 1, OPIVX, 0)),
        // carry
        ("vadc.vvm", op_v_masked(0b01_0000, 4, 1, OPIVV, 0)),
        ("vadc.vim", op_v_masked(0b01_0000, 4, 1, OPIVI, 0)),
        ("vadc.vxm", op_v_masked(0b01_0000, 4, 1, OPIVX, 0)),
        ("vsbc.vvm", op_v_masked(0b01_0010, 4, 1, OPIVV, 0)),
        ("vsbc.vxm", op_v_masked(0b01_0010, 4, 1, OPIVX, 0)),
        // muldiv
        ("vdivu.vv", op_v_masked(0b100_000, 4, 1, OPMVV, 0)),
        ("vdivu.vx", op_v_masked(0b100_000, 4, 1, OPMVX, 0)),
        ("vdiv.vv", op_v_masked(0b100_001, 4, 1, OPMVV, 0)),
        ("vdiv.vx", op_v_masked(0b100_001, 4, 1, OPMVX, 0)),
        ("vremu.vv", op_v_masked(0b100_010, 4, 1, OPMVV, 0)),
        ("vremu.vx", op_v_masked(0b100_010, 4, 1, OPMVX, 0)),
        ("vrem.vv", op_v_masked(0b100_011, 4, 1, OPMVV, 0)),
        ("vrem.vx", op_v_masked(0b100_011, 4, 1, OPMVX, 0)),
        ("vmulhu.vv", op_v_masked(0b100_100, 4, 1, OPMVV, 0)),
        ("vmulhu.vx", op_v_masked(0b100_100, 4, 1, OPMVX, 0)),
        ("vmul.vv", op_v_masked(0b100_101, 4, 1, OPMVV, 0)),
        ("vmul.vx", op_v_masked(0b100_101, 4, 1, OPMVX, 0)),
        ("vmulhsu.vv", op_v_masked(0b100_110, 4, 1, OPMVV, 0)),
        ("vmulhsu.vx", op_v_masked(0b100_110, 4, 1, OPMVX, 0)),
        ("vmulh.vv", op_v_masked(0b100_111, 4, 1, OPMVV, 0)),
        ("vmulh.vx", op_v_masked(0b100_111, 4, 1, OPMVX, 0)),
        ("vmadd.vv", op_v_masked(0b101_001, 4, 1, OPMVV, 0)),
        ("vmadd.vx", op_v_masked(0b101_001, 4, 1, OPMVX, 0)),
        ("vnmsub.vv", op_v_masked(0b101_011, 4, 1, OPMVV, 0)),
        ("vnmsub.vx", op_v_masked(0b101_011, 4, 1, OPMVX, 0)),
        ("vmacc.vv", op_v_masked(0b101_101, 4, 1, OPMVV, 0)),
        ("vmacc.vx", op_v_masked(0b101_101, 4, 1, OPMVX, 0)),
        ("vnmsac.vv", op_v_masked(0b101_111, 4, 1, OPMVV, 0)),
        ("vnmsac.vx", op_v_masked(0b101_111, 4, 1, OPMVX, 0)),
        ("vwmulu.vv", op_v_masked(0b111_000, 4, 1, OPMVV, 0)),
        ("vwmulu.vx", op_v_masked(0b111_000, 4, 1, OPMVX, 0)),
        ("vwmulsu.vv", op_v_masked(0b111_010, 4, 1, OPMVV, 0)),
        ("vwmulsu.vx", op_v_masked(0b111_010, 4, 1, OPMVX, 0)),
        ("vwmul.vv", op_v_masked(0b111_011, 4, 1, OPMVV, 0)),
        ("vwmul.vx", op_v_masked(0b111_011, 4, 1, OPMVX, 0)),
        ("vwmaccu.vv", op_v_masked(0b111_100, 4, 1, OPMVV, 0)),
        ("vwmaccu.vx", op_v_masked(0b111_100, 4, 1, OPMVX, 0)),
        ("vwmacc.vv", op_v_masked(0b111_101, 4, 1, OPMVV, 0)),
        ("vwmacc.vx", op_v_masked(0b111_101, 4, 1, OPMVX, 0)),
        ("vwmaccus.vx", op_v_masked(0b111_110, 4, 1, OPMVX, 0)),
        ("vwmaccsu.vv", op_v_masked(0b111_111, 4, 1, OPMVV, 0)),
        ("vwmaccsu.vx", op_v_masked(0b111_111, 4, 1, OPMVX, 0)),
        // widen narrow
        ("vzext.vf8", op_v_masked(0b01_0010, 4, 0b0_0010, OPMVV, 0)),
        ("vsext.vf8", op_v_masked(0b01_0010, 4, 0b0_0011, OPMVV, 0)),
        ("vzext.vf4", op_v_masked(0b01_0010, 4, 0b0_0100, OPMVV, 0)),
        ("vsext.vf4", op_v_masked(0b01_0010, 4, 0b0_0101, OPMVV, 0)),
        ("vzext.vf2", op_v_masked(0b01_0010, 4, 0b0_0110, OPMVV, 0)),
        ("vsext.vf2", op_v_masked(0b01_0010, 4, 0b0_0111, OPMVV, 0)),
        ("vnsrl.wv", op_v_masked(0b101_100, 4, 1, OPIVV, 0)),
        ("vnsrl.wi", op_v_masked(0b101_100, 4, 1, OPIVI, 0)),
        ("vnsrl.wx", op_v_masked(0b101_100, 4, 1, OPIVX, 0)),
        ("vnsra.wv", op_v_masked(0b101_101, 4, 1, OPIVV, 0)),
        ("vnsra.wi", op_v_masked(0b101_101, 4, 1, OPIVI, 0)),
        ("vnsra.wx", op_v_masked(0b101_101, 4, 1, OPIVX, 0)),
        ("vwaddu.vv", op_v_masked(0b110_000, 4, 1, OPMVV, 0)),
        ("vwaddu.vx", op_v_masked(0b110_000, 4, 1, OPMVX, 0)),
        ("vwadd.vv", op_v_masked(0b110_001, 4, 1, OPMVV, 0)),
        ("vwadd.vx", op_v_masked(0b110_001, 4, 1, OPMVX, 0)),
        ("vwsubu.vv", op_v_masked(0b110_010, 4, 1, OPMVV, 0)),
        ("vwsubu.vx", op_v_masked(0b110_010, 4, 1, OPMVX, 0)),
        ("vwsub.vv", op_v_masked(0b110_011, 4, 1, OPMVV, 0)),
        ("vwsub.vx", op_v_masked(0b110_011, 4, 1, OPMVX, 0)),
        ("vwaddu.wv", op_v_masked(0b110_100, 4, 1, OPMVV, 0)),
        ("vwaddu.wx", op_v_masked(0b110_100, 4, 1, OPMVX, 0)),
        ("vwadd.wv", op_v_masked(0b110_101, 4, 1, OPMVV, 0)),
        ("vwadd.wx", op_v_masked(0b110_101, 4, 1, OPMVX, 0)),
        ("vwsubu.wv", op_v_masked(0b110_110, 4, 1, OPMVV, 0)),
        ("vwsubu.wx", op_v_masked(0b110_110, 4, 1, OPMVX, 0)),
        ("vwsub.wv", op_v_masked(0b110_111, 4, 1, OPMVV, 0)),
        ("vwsub.wx", op_v_masked(0b110_111, 4, 1, OPMVX, 0)),
        // fixed point
        ("vaaddu.vv", op_v_masked(0b00_1000, 4, 1, OPMVV, 0)),
        ("vaaddu.vx", op_v_masked(0b00_1000, 4, 1, OPMVX, 0)),
        ("vaadd.vv", op_v_masked(0b00_1001, 4, 1, OPMVV, 0)),
        ("vaadd.vx", op_v_masked(0b00_1001, 4, 1, OPMVX, 0)),
        ("vasubu.vv", op_v_masked(0b00_1010, 4, 1, OPMVV, 0)),
        ("vasubu.vx", op_v_masked(0b00_1010, 4, 1, OPMVX, 0)),
        ("vasub.vv", op_v_masked(0b00_1011, 4, 1, OPMVV, 0)),
        ("vasub.vx", op_v_masked(0b00_1011, 4, 1, OPMVX, 0)),
        ("vsaddu.vv", op_v_masked(0b100_000, 4, 1, OPIVV, 0)),
        ("vsaddu.vi", op_v_masked(0b100_000, 4, 1, OPIVI, 0)),
        ("vsaddu.vx", op_v_masked(0b100_000, 4, 1, OPIVX, 0)),
        ("vsadd.vv", op_v_masked(0b100_001, 4, 1, OPIVV, 0)),
        ("vsadd.vi", op_v_masked(0b100_001, 4, 1, OPIVI, 0)),
        ("vsadd.vx", op_v_masked(0b100_001, 4, 1, OPIVX, 0)),
        ("vssubu.vv", op_v_masked(0b100_010, 4, 1, OPIVV, 0)),
        ("vssubu.vx", op_v_masked(0b100_010, 4, 1, OPIVX, 0)),
        ("vssub.vv", op_v_masked(0b100_011, 4, 1, OPIVV, 0)),
        ("vssub.vx", op_v_masked(0b100_011, 4, 1, OPIVX, 0)),
        ("vsmul.vv", op_v_masked(0b100_111, 4, 1, OPIVV, 0)),
        ("vsmul.vx", op_v_masked(0b100_111, 4, 1, OPIVX, 0)),
        ("vssrl.vv", op_v_masked(0b101_010, 4, 1, OPIVV, 0)),
        ("vssrl.vi", op_v_masked(0b101_010, 4, 1, OPIVI, 0)),
        ("vssrl.vx", op_v_masked(0b101_010, 4, 1, OPIVX, 0)),
        ("vssra.vv", op_v_masked(0b101_011, 4, 1, OPIVV, 0)),
        ("vssra.vi", op_v_masked(0b101_011, 4, 1, OPIVI, 0)),
        ("vssra.vx", op_v_masked(0b101_011, 4, 1, OPIVX, 0)),
        ("vnclipu.wv", op_v_masked(0b101_110, 4, 1, OPIVV, 0)),
        ("vnclipu.wi", op_v_masked(0b101_110, 4, 1, OPIVI, 0)),
        ("vnclipu.wx", op_v_masked(0b101_110, 4, 1, OPIVX, 0)),
        ("vnclip.wv", op_v_masked(0b101_111, 4, 1, OPIVV, 0)),
        ("vnclip.wi", op_v_masked(0b101_111, 4, 1, OPIVI, 0)),
        ("vnclip.wx", op_v_masked(0b101_111, 4, 1, OPIVX, 0)),
        // mask
        ("vmsbf.m", op_v_masked(0b01_0100, 4, 0b0_0001, OPMVV, 0)),
        ("vmsof.m", op_v_masked(0b01_0100, 4, 0b0_0010, OPMVV, 0)),
        ("vmsif.m", op_v_masked(0b01_0100, 4, 0b0_0011, OPMVV, 0)),
        ("viota.m", op_v_masked(0b01_0100, 4, 0b1_0000, OPMVV, 0)),
        ("vid.v", op_v_masked(0b01_0100, 0, 0b1_0001, OPMVV, 0)),
        // perm
        ("vrgather.vv", op_v_masked(0b00_1100, 4, 1, OPIVV, 0)),
        ("vrgather.vi", op_v_masked(0b00_1100, 4, 1, OPIVI, 0)),
        ("vrgather.vx", op_v_masked(0b00_1100, 4, 1, OPIVX, 0)),
        ("vrgatherei16.vv", op_v_masked(0b00_1110, 4, 1, OPIVV, 0)),
        ("vslideup.vi", op_v_masked(0b00_1110, 4, 1, OPIVI, 0)),
        ("vslideup.vx", op_v_masked(0b00_1110, 4, 1, OPIVX, 0)),
        ("vslide1up.vx", op_v_masked(0b00_1110, 4, 1, OPMVX, 0)),
        ("vslidedown.vi", op_v_masked(0b00_1111, 4, 1, OPIVI, 0)),
        ("vslidedown.vx", op_v_masked(0b00_1111, 4, 1, OPIVX, 0)),
        ("vslide1down.vx", op_v_masked(0b00_1111, 4, 1, OPMVX, 0)),
        ("vmerge.vvm", op_v_masked(0b01_0111, 4, 1, OPIVV, 0)),
        ("vmerge.vim", op_v_masked(0b01_0111, 4, 1, OPIVI, 0)),
        ("vmerge.vxm", op_v_masked(0b01_0111, 4, 1, OPIVX, 0)),
        // load
        ("vle8.v", load_masked(0, 0b00, 0, 0b000, 0)),
        ("vle8ff.v", load_masked(0, 0b00, 16, 0b000, 0)),
        ("vluxei8.v", load_masked(0, 0b01, 1, 0b000, 0)),
        ("vlse8.v", load_masked(0, 0b10, 0, 0b000, 0)),
        ("vloxei8.v", load_masked(0, 0b11, 1, 0b000, 0)),
        ("vlseg2e8.v", load_masked(1, 0b00, 0, 0b000, 0)),
        ("vlseg2e8ff.v", load_masked(1, 0b00, 16, 0b000, 0)),
        ("vluxseg2ei8.v", load_masked(1, 0b01, 1, 0b000, 0)),
        ("vlsseg2e8.v", load_masked(1, 0b10, 0, 0b000, 0)),
        ("vloxseg2ei8.v", load_masked(1, 0b11, 1, 0b000, 0)),
    ];
    for (name, instruction) in reserved {
        assert_eq!(
            ZveXxInstruction::<TestVectorHart>::try_decode(instruction),
            None,
            "{name}"
        );
        // Same instruction with `vd = v8`
        let instruction = ZveXxInstruction::<TestVectorHart>::try_decode(instruction | (8 << 7));
        assert_eq!(instruction.map(mnemonic).as_deref(), Some(name));
    }

    // Destinations that are masks or scalars may overlap the mask, all with `vd = v0` and `vm = 0`
    let allowed = [
        ("vredsum.vs", op_v_masked(0b00_0000, 4, 1, OPMVV, 0)),
        ("vredand.vs", op_v_masked(0b00_0001, 4, 1, OPMVV, 0)),
        ("vredor.vs", op_v_masked(0b00_0010, 4, 1, OPMVV, 0)),
        ("vredxor.vs", op_v_masked(0b00_0011, 4, 1, OPMVV, 0)),
        ("vredminu.vs", op_v_masked(0b00_0100, 4, 1, OPMVV, 0)),
        ("vredmin.vs", op_v_masked(0b00_0101, 4, 1, OPMVV, 0)),
        ("vredmaxu.vs", op_v_masked(0b00_0110, 4, 1, OPMVV, 0)),
        ("vredmax.vs", op_v_masked(0b00_0111, 4, 1, OPMVV, 0)),
        ("vcpop.m", op_v_masked(0b01_0000, 4, 0b1_0000, OPMVV, 0)),
        ("vfirst.m", op_v_masked(0b01_0000, 4, 0b1_0001, OPMVV, 0)),
        ("vmadc.vvm", op_v_masked(0b01_0001, 4, 1, OPIVV, 0)),
        ("vmadc.vim", op_v_masked(0b01_0001, 4, 1, OPIVI, 0)),
        ("vmadc.vxm", op_v_masked(0b01_0001, 4, 1, OPIVX, 0)),
        ("vmsbc.vvm", op_v_masked(0b01_0011, 4, 1, OPIVV, 0)),
        ("vmsbc.vxm", op_v_masked(0b01_0011, 4, 1, OPIVX, 0)),
        ("vmseq.vv", op_v_masked(0b01_1000, 4, 1, OPIVV, 0)),
        ("vmseq.vi", op_v_masked(0b01_1000, 4, 1, OPIVI, 0)),
        ("vmseq.vx", op_v_masked(0b01_1000, 4, 1, OPIVX, 0)),
        ("vmsne.vv", op_v_masked(0b01_1001, 4, 1, OPIVV, 0)),
        ("vmsne.vi", op_v_masked(0b01_1001, 4, 1, OPIVI, 0)),
        ("vmsne.vx", op_v_masked(0b01_1001, 4, 1, OPIVX, 0)),
        ("vmsltu.vv", op_v_masked(0b01_1010, 4, 1, OPIVV, 0)),
        ("vmsltu.vx", op_v_masked(0b01_1010, 4, 1, OPIVX, 0)),
        ("vmslt.vv", op_v_masked(0b01_1011, 4, 1, OPIVV, 0)),
        ("vmslt.vx", op_v_masked(0b01_1011, 4, 1, OPIVX, 0)),
        ("vmsleu.vv", op_v_masked(0b01_1100, 4, 1, OPIVV, 0)),
        ("vmsleu.vi", op_v_masked(0b01_1100, 4, 1, OPIVI, 0)),
        ("vmsleu.vx", op_v_masked(0b01_1100, 4, 1, OPIVX, 0)),
        ("vmsle.vv", op_v_masked(0b01_1101, 4, 1, OPIVV, 0)),
        ("vmsle.vi", op_v_masked(0b01_1101, 4, 1, OPIVI, 0)),
        ("vmsle.vx", op_v_masked(0b01_1101, 4, 1, OPIVX, 0)),
        ("vmsgtu.vi", op_v_masked(0b01_1110, 4, 1, OPIVI, 0)),
        ("vmsgtu.vx", op_v_masked(0b01_1110, 4, 1, OPIVX, 0)),
        ("vmsgt.vi", op_v_masked(0b01_1111, 4, 1, OPIVI, 0)),
        ("vmsgt.vx", op_v_masked(0b01_1111, 4, 1, OPIVX, 0)),
        ("vwredsumu.vs", op_v_masked(0b110_000, 4, 1, OPIVV, 0)),
        ("vwredsum.vs", op_v_masked(0b110_001, 4, 1, OPIVV, 0)),
    ];
    for (name, instruction) in allowed {
        let instruction = ZveXxInstruction::<TestVectorHart>::try_decode(instruction);
        assert_eq!(instruction.map(mnemonic).as_deref(), Some(name));
    }
}
