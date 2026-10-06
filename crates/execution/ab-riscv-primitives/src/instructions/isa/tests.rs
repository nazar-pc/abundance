use crate::hart::{BasicHart, BasicVectorHart, HartConfig, VectorHartConfig};
use crate::instructions::isa::{MAX_ISA_STRING_LENGTH, isa_string};
use crate::instructions::rv32::Rv32Instruction;
use crate::instructions::rv32::c::zca::Rv32ZcaInstruction;
use crate::instructions::rv32::zce::zcmp::{ZcmpRegister, ZcmpUrlist};
use crate::instructions::rv64::Rv64Instruction;
use crate::instructions::rv64::a::Rv64AInstruction;
use crate::instructions::rv64::a::zaamo::Rv64ZaamoInstruction;
use crate::instructions::rv64::a::zalrsc::Rv64ZalrscInstruction;
use crate::instructions::rv64::b::Rv64BInstruction;
use crate::instructions::rv64::b::zba::Rv64ZbaInstruction;
use crate::instructions::rv64::b::zbb::{Rv64ZbbInstruction, Rv64ZbbZbkbSharedInstruction};
use crate::instructions::rv64::b::zbs::Rv64ZbsInstruction;
use crate::instructions::rv64::c::zca::Rv64ZcaInstruction;
use crate::instructions::rv64::m::Rv64MInstruction;
use crate::instructions::rv64::m::zmmul::Rv64ZmmulInstruction;
use crate::instructions::rv64::zabha::Rv64ZabhaInstruction;
use crate::instructions::rv64::zacas::Rv64ZacasInstruction;
use crate::instructions::rv64::zalasr::Rv64ZalasrInstruction;
use crate::instructions::rv64::zce::zcb::{Rv64ZcbInstruction, Rv64ZcbOnlyInstruction};
use crate::instructions::rv64::zce::zcmp::{Rv64ZcmpInstruction, Rv64ZcmpOnlyInstruction};
use crate::instructions::rv64::zk::zbkb::Rv64ZbkbInstruction;
use crate::instructions::rv64::zk::zbkc::Rv64ZbkcInstruction;
use crate::instructions::rv64::zk::zbkx::Rv64ZbkxInstruction;
use crate::instructions::rv64::zk::zkn::Rv64ZknInstruction;
use crate::instructions::rv64::zk::zkn::zknd::{
    Rv64ZkndInstruction, Rv64ZkndKsRnum, Rv64ZkndZkneSharedInstruction,
};
use crate::instructions::rv64::zk::zkn::zkne::Rv64ZkneInstruction;
use crate::instructions::rv64::zk::zkn::zknh::Rv64ZknhInstruction;
use crate::instructions::utils::{I24, I24WithZeroedBits};
use crate::instructions::v::zvexx::ZveXxInstruction;
use crate::instructions::v::zvexx::arith::ZveXxArithInstruction;
use crate::instructions::v::zvexx::carry::ZveXxCarryInstruction;
use crate::instructions::v::zvexx::config::ZveXxConfigInstruction;
use crate::instructions::v::zvexx::fixed_point::ZveXxFixedPointInstruction;
use crate::instructions::v::zvexx::load::{LoadStoreNreg, Nf, SegVmNf, ZveXxLoadInstruction};
use crate::instructions::v::zvexx::mask::ZveXxMaskInstruction;
use crate::instructions::v::zvexx::muldiv::ZveXxMulDivInstruction;
use crate::instructions::v::zvexx::perm::ZveXxPermInstruction;
use crate::instructions::v::zvexx::reduction::ZveXxReductionInstruction;
use crate::instructions::v::zvexx::store::ZveXxStoreInstruction;
use crate::instructions::v::zvexx::widen_narrow::ZveXxWidenNarrowInstruction;
use crate::instructions::v::{Eew, Elen, V, Vlen};
use crate::instructions::zawrs::ZawrsInstruction;
use crate::instructions::zicond::ZicondInstruction;
use crate::instructions::zicsr::ZicsrInstruction;
use crate::instructions::zifencei::ZifenceiInstruction;
use crate::instructions::zkr::ZkrInstruction;
use crate::instructions::zvbb::ZvbbInstruction;
use crate::instructions::zvbb::zvkb::ZvkbInstruction;
use crate::instructions::zvbc::ZvbcInstruction;
use crate::instructions::{ImplementedExtension, Instruction, IsaExtension};
use crate::registers::general_purpose::{EReg, Reg, Register};
use crate::registers::vector::VReg;
use ab_riscv_macros::instruction;
use core::any::TypeId;
use core::fmt;

#[instruction(
    inherit = [Rv64Instruction, Rv64MInstruction],
)]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
enum TestMInstruction<Hart>
where
    Hart: HartConfig, {}

#[instruction]
const impl<Reg, Hart> Instruction for TestMInstruction<Hart>
where
    Reg: [const] Register<Type = u64>,
    Hart: [const] HartConfig<Reg = Reg>,
{
    const OWN_ISA_EXTENSIONS: &'static [IsaExtension] = &[];

    const ALIGNMENT: u8 = align_of::<u16>() as u8;

    type Hart = Hart;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        None
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

#[instruction]
impl<Reg, Hart> fmt::Display for TestMInstruction<Hart>
where
    Reg: Register,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[instruction(
    inherit = [
        Rv64Instruction,
        Rv64MInstruction,
        Rv64AInstruction,
        Rv64BInstruction,
        ZicsrInstruction,
        ZifenceiInstruction,
        Rv64ZcbInstruction,
        Rv64ZcmpInstruction,
    ],
)]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
enum TestScalarInstruction<Hart>
where
    Hart: HartConfig, {}

#[instruction]
const impl<Reg, Hart> Instruction for TestScalarInstruction<Hart>
where
    Reg: [const] Register<Type = u64>,
    Hart: [const] HartConfig<Reg = Reg>,
{
    const OWN_ISA_EXTENSIONS: &'static [IsaExtension] = &[];

    const ALIGNMENT: u8 = align_of::<u16>() as u8;

    type Hart = Hart;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        None
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

#[instruction]
impl<Reg, Hart> fmt::Display for TestScalarInstruction<Hart>
where
    Reg: Register,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[instruction(
    inherit = [
        Rv64Instruction,
        Rv64ZacasInstruction,
        Rv64ZabhaInstruction,
        Rv64ZalasrInstruction,
        ZawrsInstruction,
        ZicondInstruction,
        ZkrInstruction,
        Rv64ZknInstruction,
    ],
)]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
enum TestMiscInstruction<Hart>
where
    Hart: HartConfig, {}

#[instruction]
const impl<Reg, Hart> Instruction for TestMiscInstruction<Hart>
where
    Reg: [const] Register<Type = u64>,
    Hart: [const] HartConfig<Reg = Reg>,
{
    const OWN_ISA_EXTENSIONS: &'static [IsaExtension] = &[];

    const ALIGNMENT: u8 = align_of::<u16>() as u8;

    type Hart = Hart;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        None
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

#[instruction]
impl<Reg, Hart> fmt::Display for TestMiscInstruction<Hart>
where
    Reg: Register,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[instruction(
    inherit = [
        Rv64Instruction,
        Rv64ZaamoInstruction,
        Rv64ZalrscInstruction,
        Rv64ZbaInstruction,
        Rv64ZbbInstruction,
        Rv64ZbsInstruction,
        Rv64ZbkbInstruction,
        Rv64ZbkcInstruction,
        Rv64ZbkxInstruction,
        Rv64ZkneInstruction,
        Rv64ZkndInstruction,
        Rv64ZknhInstruction,
        Rv64ZcaInstruction,
    ],
)]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
enum TestSeparateInstruction<Hart>
where
    Hart: HartConfig, {}

#[instruction]
const impl<Reg, Hart> Instruction for TestSeparateInstruction<Hart>
where
    Reg: [const] Register<Type = u64>,
    Hart: [const] HartConfig<Reg = Reg>,
{
    const OWN_ISA_EXTENSIONS: &'static [IsaExtension] = &[];

    const ALIGNMENT: u8 = align_of::<u16>() as u8;

    type Hart = Hart;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        None
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

#[instruction]
impl<Reg, Hart> fmt::Display for TestSeparateInstruction<Hart>
where
    Reg: Register,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[instruction(
    inherit = [Rv32Instruction, Rv32ZcaInstruction],
)]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
enum TestRv32ZcaInstruction<Hart>
where
    Hart: HartConfig, {}

#[instruction]
const impl<Reg, Hart> Instruction for TestRv32ZcaInstruction<Hart>
where
    Reg: [const] Register<Type = u32>,
    Hart: [const] HartConfig<Reg = Reg>,
{
    const OWN_ISA_EXTENSIONS: &'static [IsaExtension] = &[];

    const ALIGNMENT: u8 = align_of::<u16>() as u8;

    type Hart = Hart;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        None
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

#[instruction]
impl<Reg, Hart> fmt::Display for TestRv32ZcaInstruction<Hart>
where
    Reg: Register,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[instruction(
    inherit = [Rv64Instruction, ZveXxInstruction, ZvbbInstruction, ZvbcInstruction],
)]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
enum TestVectorInstruction<Hart>
where
    Hart: HartConfig, {}

#[instruction]
const impl<Reg, Hart> Instruction for TestVectorInstruction<Hart>
where
    Reg: [const] Register<Type = u64>,
    Hart: [const] VectorHartConfig<Reg = Reg>,
{
    const OWN_ISA_EXTENSIONS: &'static [IsaExtension] = &[];

    const ALIGNMENT: u8 = align_of::<u16>() as u8;

    type Hart = Hart;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        None
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

#[instruction]
impl<Reg, Hart> fmt::Display for TestVectorInstruction<Hart>
where
    Reg: Register,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[instruction(
    inherit = [Rv64Instruction, ZveXxInstruction, ZvbbInstruction],
)]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
enum TestZve32xInstruction<Hart>
where
    Hart: HartConfig, {}

#[instruction]
const impl<Reg, Hart> Instruction for TestZve32xInstruction<Hart>
where
    Reg: [const] Register<Type = u64>,
    Hart: [const] VectorHartConfig<Reg = Reg>,
{
    const OWN_ISA_EXTENSIONS: &'static [IsaExtension] = &[];

    const ALIGNMENT: u8 = align_of::<u16>() as u8;

    type Hart = Hart;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        None
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

#[instruction]
impl<Reg, Hart> fmt::Display for TestZve32xInstruction<Hart>
where
    Reg: Register,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

type Elen64Vlen256Hart = BasicVectorHart<Reg<u64>, { Elen::L64 }, { Vlen::L256 }>;
type Elen32Vlen32Hart = BasicVectorHart<Reg<u64>, { Elen::L32 }, { Vlen::L32 }>;

/// Extension with version `1.0`
const fn extension(name: &'static str) -> IsaExtension {
    IsaExtension::new(name, 1, 0)
}

/// Implemented extension with given own ISA extensions
const fn implemented(own_isa_extensions: &'static [IsaExtension]) -> ImplementedExtension {
    ImplementedExtension {
        type_id: TypeId::of::<()>(),
        own_isa_extensions,
    }
}

fn assert_isa_string(xlen: u8, implemented_extensions: &[ImplementedExtension], expected: &str) {
    assert_eq!(
        isa_string(xlen, implemented_extensions).unsize().as_str(),
        expected
    );
}

#[test]
fn canonical_order() {
    assert_isa_string(
        64,
        &[implemented(
            const {
                &[
                    extension("zvl32b"),
                    extension("zbb"),
                    extension("sstc"),
                    extension("zicsr"),
                    extension("zvl128b"),
                    extension("xcustom"),
                    extension("m"),
                    extension("zmmul"),
                    extension("zba"),
                    extension("p"),
                    extension("zifencei"),
                    extension("i"),
                    extension("v"),
                    extension("zve32x"),
                ]
            },
        )],
        "rv64i1p0_m1p0_p1p0_v1p0_zicsr1p0_zifencei1p0_zmmul1p0_zba1p0_zbb1p0_zve32x1p0_\
        zvl128b1p0_zvl32b1p0_sstc1p0_xcustom1p0",
    );
}

#[test]
fn duplicates() {
    assert_isa_string(
        64,
        &[
            implemented(const { &[IsaExtension::new("i", 2, 1)] }),
            implemented(const { &[IsaExtension::new("zicsr", 2, 0)] }),
            implemented(
                const {
                    &[
                        IsaExtension::new("i", 2, 1),
                        IsaExtension::new("zicsr", 2, 0),
                        IsaExtension::new("m", 2, 0),
                    ]
                },
            ),
            implemented(const { &[] }),
        ],
        "rv64i2p1_m2p0_zicsr2p0",
    );
}

#[test]
fn versions() {
    // Different versions of the same extension are all listed, ordered by version
    assert_isa_string(
        64,
        &[implemented(
            const {
                &[
                    IsaExtension::new("i", 2, 1),
                    IsaExtension::new("m", 2, 0),
                    IsaExtension::new("zmmul", 1, 0),
                    IsaExtension::new("xcustom", 123, 45),
                    IsaExtension::new("m", 1, 0),
                ]
            },
        )],
        "rv64i2p1_m1p0_m2p0_zmmul1p0_xcustom123p45",
    );
}

#[test]
fn without_base_isa() {
    assert_isa_string(32, &[], "");
    assert_isa_string(
        64,
        &[implemented(
            const { &[extension("zmmul"), IsaExtension::new("m", 2, 0)] },
        )],
        "m2p0_zmmul1p0",
    );
}

#[test]
#[should_panic(expected = "ISA string is too long")]
fn too_long() {
    const NAME: &str = match str::from_utf8(&[b'x'; MAX_ISA_STRING_LENGTH]) {
        Ok(name) => name,
        Err(_) => unreachable!(),
    };
    assert_isa_string(
        64,
        &[implemented(const { &[extension("i"), extension(NAME)] })],
        "",
    );
}

#[test]
fn without_base_isa_string() {
    assert_eq!(
        <Rv64MInstruction<BasicHart<Reg<u64>>> as Instruction>::ISA_STRING,
        "m2p0_zmmul1p0"
    );
    assert_eq!(
        <Rv64AInstruction<BasicHart<Reg<u64>>> as Instruction>::ISA_STRING,
        "a2p1_zaamo1p0_zalrsc1p0"
    );
    assert_eq!(
        <Rv64ZbbZbkbSharedInstruction<BasicHart<Reg<u64>>> as Instruction>::ISA_STRING,
        ""
    );
}

// Expected ISA strings below are produced by LLVM for the same set of extensions, except that LLVM
// also adds extensions whose parts are all present (like `c` for `zca`)

#[test]
fn base_isa_string() {
    assert_eq!(
        <Rv32Instruction<BasicHart<Reg<u32>>> as Instruction>::ISA_STRING,
        "rv32i2p1"
    );
    assert_eq!(
        <Rv32Instruction<BasicHart<EReg<u32>>> as Instruction>::ISA_STRING,
        "rv32e2p0"
    );
    assert_eq!(
        <Rv64Instruction<BasicHart<Reg<u64>>> as Instruction>::ISA_STRING,
        "rv64i2p1"
    );
    assert_eq!(
        <Rv64Instruction<BasicHart<EReg<u64>>> as Instruction>::ISA_STRING,
        "rv64e2p0"
    );
}

#[test]
fn implied_extensions() {
    // `M` inherits `Zmmul`
    assert_eq!(
        <TestMInstruction<BasicHart<Reg<u64>>> as Instruction>::ISA_STRING,
        "rv64i2p1_m2p0_zmmul1p0"
    );
    assert_eq!(
        <TestScalarInstruction<BasicHart<Reg<u64>>> as Instruction>::ISA_STRING,
        "rv64i2p1_m2p0_a2p1_b1p0_zicsr2p0_zifencei2p0_zmmul1p0_zaamo1p0_zalrsc1p0_zca1p0_\
        zcb1p0_zcmp1p0_zba1p0_zbb1p0_zbs1p0"
    );
    // `Zkr` inherits `Zicsr` (LLVM needs `+zicsr` explicitly for this)
    assert_eq!(
        <TestMiscInstruction<BasicHart<Reg<u64>>> as Instruction>::ISA_STRING,
        "rv64i2p1_zicond1p0_zicsr2p0_zaamo1p0_zabha1p0_zacas1p0_zalasr1p0_zawrs1p0_zbkb1p0_zbkc1p0_\
        zbkx1p0_zkn1p0_zknd1p0_zkne1p0_zknh1p0_zkr1p0"
    );
}

#[test]
fn separate_extensions_isa_string() {
    // Extensions are not combined from their parts inherited separately, like `a`, `b`, `c` and
    // `zkn`
    assert_eq!(
        <TestSeparateInstruction<BasicHart<Reg<u64>>> as Instruction>::ISA_STRING,
        "rv64i2p1_zaamo1p0_zalrsc1p0_zca1p0_zba1p0_zbb1p0_zbkb1p0_zbkc1p0_zbkx1p0_zbs1p0_zknd1p0_\
        zkne1p0_zknh1p0"
    );
    assert_eq!(
        <TestRv32ZcaInstruction<BasicHart<Reg<u32>>> as Instruction>::ISA_STRING,
        "rv32i2p1_zca1p0"
    );
}

#[test]
fn vector_isa_string() {
    assert_eq!(
        <TestVectorInstruction<Elen64Vlen256Hart> as Instruction>::ISA_STRING,
        "rv64i2p1_zicsr2p0_zvbb1p0_zvbc1p0_zve32x1p0_zve64x1p0_zvkb1p0_zvl128b1p0_zvl256b1p0_\
        zvl32b1p0_zvl64b1p0"
    );
    assert_eq!(
        <TestZve32xInstruction<Elen32Vlen32Hart> as Instruction>::ISA_STRING,
        "rv64i2p1_zicsr2p0_zvbb1p0_zve32x1p0_zvkb1p0_zvl32b1p0"
    );
}
