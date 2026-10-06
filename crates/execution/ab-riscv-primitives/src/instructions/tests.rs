use crate::hart::{BasicHart, HartConfig};
use crate::instructions::Instruction;
use crate::instructions::rv64::Rv64Instruction;
use crate::instructions::rv64::a::zaamo::Rv64ZaamoInstruction;
use crate::instructions::rv64::b::Rv64BInstruction;
use crate::instructions::rv64::b::zba::Rv64ZbaInstruction;
use crate::instructions::rv64::b::zbb::{Rv64ZbbInstruction, Rv64ZbbZbkbSharedInstruction};
use crate::instructions::rv64::b::zbs::Rv64ZbsInstruction;
use crate::instructions::rv64::m::Rv64MInstruction;
use crate::instructions::rv64::m::zmmul::Rv64ZmmulInstruction;
use crate::instructions::rv64::zabha::Rv64ZabhaInstruction;
use crate::instructions::rv64::zacas::Rv64ZacasInstruction;
use crate::instructions::test_utils::make_r_type;
use crate::instructions::utils::{I24, I24WithZeroedBits};
use crate::instructions::zicond::ZicondInstruction;
use crate::instructions::zicsr::ZicsrInstruction;
use crate::registers::general_purpose::{Reg, Register};
use ab_riscv_macros::instruction;
use core::fmt;

#[instruction(
    ignore = [
        Ecall,
        CzeroEqz,
        CzeroNez,
        AmocasW,
        AmocasD,
        AmocasQ,
        Rv64ZbsInstruction,
        Rv64ZmmulInstruction,
    ],
    inherit = [
        Rv64Instruction,
        ZicondInstruction,
        Rv64ZabhaInstruction,
        Rv64ZacasInstruction,
        Rv64BInstruction,
        Rv64MInstruction,
    ],
)]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
enum TestIgnoreInstruction<Hart>
where
    Hart: HartConfig, {}

#[instruction]
const impl<Reg, Hart> Instruction for TestIgnoreInstruction<Hart>
where
    Reg: [const] Register<Type = u64>,
    Hart: [const] HartConfig<Reg = Reg>,
{
    const ALIGNMENT: u8 = align_of::<u32>() as u8;

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
impl<Reg, Hart> fmt::Display for TestIgnoreInstruction<Hart>
where
    Reg: fmt::Display + Copy,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

fn implements_extension<E>() -> bool
where
    E: Instruction<Hart = BasicHart<Reg<u64>>>,
{
    TestIgnoreInstruction::<BasicHart<Reg<u64>>>::implements_extension::<E>()
}

#[test]
fn implemented_extensions() {
    assert!(implements_extension::<TestIgnoreInstruction<_>>());
    // Ignoring `ecall` by name doesn't exclude the base ISA
    assert!(implements_extension::<Rv64Instruction<_>>());
    // Ignoring all instructions of `Zicond` by name excludes it
    assert!(!implements_extension::<ZicondInstruction<_>>());
    // Ignoring all own instructions of `Zacas` by name excludes it, even though instructions of
    // `Zaamo` it inherits are present
    assert!(!implements_extension::<Rv64ZacasInstruction<_>>());
    // `amocas.b` and `amocas.h` from `Zabha` are missing without `Zacas`, but `Zabha` is still
    // implemented together with `Zaamo` it inherits
    assert!(implements_extension::<Rv64ZabhaInstruction<_>>());
    assert!(implements_extension::<Rv64ZaamoInstruction<_>>());
    // amocas.b: funct5=00101, funct3=000
    let amocas_b = make_r_type(0b010_1111, 1, 0b000, 2, 3, 0b001_0100);
    assert!(TestIgnoreInstruction::<BasicHart<Reg<u64>>>::try_decode(amocas_b).is_none());
    // Ignoring the whole `Zbs` excludes it together with `B` that inherits it, but not other
    // extensions inherited by `B`
    assert!(!implements_extension::<Rv64ZbsInstruction<_>>());
    assert!(!implements_extension::<Rv64BInstruction<_>>());
    assert!(implements_extension::<Rv64ZbaInstruction<_>>());
    assert!(implements_extension::<Rv64ZbbInstruction<_>>());
    assert!(implements_extension::<Rv64ZbbZbkbSharedInstruction<_>>());
    // Ignoring the whole `Zmmul` excludes `M` that inherits it, even though own instructions of `M`
    // are present
    assert!(!implements_extension::<Rv64ZmmulInstruction<_>>());
    assert!(!implements_extension::<Rv64MInstruction<_>>());
}

/// Ignores `Zabha` as a whole without inheriting it
#[instruction(
    ignore = [Rv64ZabhaInstruction],
    inherit = [Rv64Instruction],
)]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
enum TestIgnoreZabhaInstruction<Hart>
where
    Hart: HartConfig, {}

#[instruction]
const impl<Reg, Hart> Instruction for TestIgnoreZabhaInstruction<Hart>
where
    Reg: [const] Register<Type = u64>,
    Hart: [const] HartConfig<Reg = Reg>,
{
    const ALIGNMENT: u8 = align_of::<u32>() as u8;

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
impl<Reg, Hart> fmt::Display for TestIgnoreZabhaInstruction<Hart>
where
    Reg: fmt::Display + Copy,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[instruction(inherit = [TestIgnoreZabhaInstruction, Rv64ZabhaInstruction])]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
enum TestNestedIgnoreInstruction<Hart>
where
    Hart: HartConfig, {}

#[instruction]
const impl<Reg, Hart> Instruction for TestNestedIgnoreInstruction<Hart>
where
    Reg: [const] Register<Type = u64>,
    Hart: [const] HartConfig<Reg = Reg>,
{
    const ALIGNMENT: u8 = align_of::<u32>() as u8;

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
impl<Reg, Hart> fmt::Display for TestNestedIgnoreInstruction<Hart>
where
    Reg: fmt::Display + Copy,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[test]
fn nested_ignore() {
    // `Zabha` ignored by another inherited instruction set doesn't affect `Zabha` inherited
    // directly, even though `amocas.b` and `amocas.h` from it are missing without `Zacas`
    assert!(
        TestNestedIgnoreInstruction::<BasicHart<Reg<u64>>>::implements_extension::<
            Rv64ZabhaInstruction<_>,
        >()
    );
}

/// `Zacas` without `amocas.q`
#[instruction(
    ignore = [AmocasQ],
    inherit = [Rv64ZacasInstruction],
)]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
enum TestZacasWithoutAmocasQInstruction<Hart>
where
    Hart: HartConfig, {}

#[instruction]
const impl<Reg, Hart> Instruction for TestZacasWithoutAmocasQInstruction<Hart>
where
    Reg: [const] Register<Type = u64>,
    Hart: [const] HartConfig<Reg = Reg>,
{
    const ALIGNMENT: u8 = align_of::<u32>() as u8;

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
impl<Reg, Hart> fmt::Display for TestZacasWithoutAmocasQInstruction<Hart>
where
    Reg: fmt::Display + Copy,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[instruction(inherit = [TestZacasWithoutAmocasQInstruction, Rv64ZabhaInstruction])]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
enum TestZacasZabhaInstruction<Hart>
where
    Hart: HartConfig, {}

#[instruction]
const impl<Reg, Hart> Instruction for TestZacasZabhaInstruction<Hart>
where
    Reg: [const] Register<Type = u64>,
    Hart: [const] HartConfig<Reg = Reg>,
{
    const ALIGNMENT: u8 = align_of::<u32>() as u8;

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
impl<Reg, Hart> fmt::Display for TestZacasZabhaInstruction<Hart>
where
    Reg: fmt::Display + Copy,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[test]
fn enum_condition() {
    // `Zacas` is implemented even without `amocas.q`, which satisfies conditions of `amocas.b` and
    // `amocas.h` from `Zabha`
    assert!(
        TestZacasZabhaInstruction::<BasicHart<Reg<u64>>>::implements_extension::<
            Rv64ZacasInstruction<_>,
        >()
    );
    // amocas.b: funct5=00101, funct3=000
    let instruction = make_r_type(0b010_1111, 1, 0b000, 2, 3, 0b001_0100);
    assert_eq!(
        TestZacasZabhaInstruction::<BasicHart<Reg<u64>>>::try_decode(instruction),
        Some(TestZacasZabhaInstruction::AmocasB {
            rd: Reg::Ra,
            rs1: Reg::Sp,
            rs2: Reg::Gp,
            aq: false,
            rl: false,
        })
    );
}

/// Without own instructions, like `Zkr`
#[instruction(inherit = [ZicsrInstruction])]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
enum TestWithoutOwnInstruction<Hart>
where
    Hart: HartConfig, {}

#[instruction]
const impl<Reg, Hart> Instruction for TestWithoutOwnInstruction<Hart>
where
    Reg: [const] Register<Type = u64>,
    Hart: [const] HartConfig<Reg = Reg>,
{
    const ALIGNMENT: u8 = align_of::<u32>() as u8;

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
impl<Reg, Hart> fmt::Display for TestWithoutOwnInstruction<Hart>
where
    Reg: fmt::Display + Copy,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

/// Defines `mul`, but ignores it
#[instruction(ignore = [Mul])]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
enum TestIgnoredMulInstruction<Hart>
where
    Hart: HartConfig,
{
    Mul {
        rd: Hart::Reg,
        rs1: Hart::Reg,
        rs2: Hart::Reg,
    },
    Foo,
}

#[instruction]
const impl<Reg, Hart> Instruction for TestIgnoredMulInstruction<Hart>
where
    Reg: [const] Register<Type = u64>,
    Hart: [const] HartConfig<Reg = Reg>,
{
    const ALIGNMENT: u8 = align_of::<u32>() as u8;

    type Hart = Hart;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        if instruction == 0 {
            Some(Self::Foo)
        } else {
            None
        }
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

#[instruction]
impl<Reg, Hart> fmt::Display for TestIgnoredMulInstruction<Hart>
where
    Reg: fmt::Display + Copy,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Foo => write!(f, "foo"),
        }
    }
}

/// Instructions with conditions
#[instruction]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
enum TestConditionInstruction<Hart>
where
    Hart: HartConfig,
{
    #[instruction(if = [TestWithoutOwnInstruction])]
    Bar,
    #[instruction(if = [Mul])]
    Baz,
}

#[instruction]
const impl<Reg, Hart> Instruction for TestConditionInstruction<Hart>
where
    Reg: [const] Register<Type = u64>,
    Hart: [const] HartConfig<Reg = Reg>,
{
    const ALIGNMENT: u8 = align_of::<u32>() as u8;

    type Hart = Hart;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        match instruction {
            1 => Some(Self::Bar),
            2 => Some(Self::Baz),
            _ => None,
        }
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

#[instruction]
impl<Reg, Hart> fmt::Display for TestConditionInstruction<Hart>
where
    Reg: fmt::Display + Copy,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bar => write!(f, "bar"),
            Self::Baz => write!(f, "baz"),
        }
    }
}

#[instruction(inherit = [TestConditionInstruction, ZicsrInstruction])]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
enum TestZicsrConditionInstruction<Hart>
where
    Hart: HartConfig, {}

#[instruction]
const impl<Reg, Hart> Instruction for TestZicsrConditionInstruction<Hart>
where
    Reg: [const] Register<Type = u64>,
    Hart: [const] HartConfig<Reg = Reg>,
{
    const ALIGNMENT: u8 = align_of::<u32>() as u8;

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
impl<Reg, Hart> fmt::Display for TestZicsrConditionInstruction<Hart>
where
    Reg: fmt::Display + Copy,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[instruction(inherit = [TestConditionInstruction, TestWithoutOwnInstruction])]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
enum TestWithoutOwnConditionInstruction<Hart>
where
    Hart: HartConfig, {}

#[instruction]
const impl<Reg, Hart> Instruction for TestWithoutOwnConditionInstruction<Hart>
where
    Reg: [const] Register<Type = u64>,
    Hart: [const] HartConfig<Reg = Reg>,
{
    const ALIGNMENT: u8 = align_of::<u32>() as u8;

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
impl<Reg, Hart> fmt::Display for TestWithoutOwnConditionInstruction<Hart>
where
    Reg: fmt::Display + Copy,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[instruction(inherit = [TestConditionInstruction, TestIgnoredMulInstruction])]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
enum TestIgnoredMulConditionInstruction<Hart>
where
    Hart: HartConfig, {}

#[instruction]
const impl<Reg, Hart> Instruction for TestIgnoredMulConditionInstruction<Hart>
where
    Reg: [const] Register<Type = u64>,
    Hart: [const] HartConfig<Reg = Reg>,
{
    const ALIGNMENT: u8 = align_of::<u32>() as u8;

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
impl<Reg, Hart> fmt::Display for TestIgnoredMulConditionInstruction<Hart>
where
    Reg: fmt::Display + Copy,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[test]
fn enum_without_own_instructions_condition() {
    // An enum without own instructions must be inherited to satisfy the condition, enums it
    // inherits are not sufficient
    assert!(TestZicsrConditionInstruction::<BasicHart<Reg<u64>>>::try_decode(1).is_none());
    assert!(TestWithoutOwnConditionInstruction::<BasicHart<Reg<u64>>>::try_decode(1).is_some());
}

#[test]
fn ignored_own_instruction_condition() {
    // `mul` ignored by an inherited enum doesn't satisfy the condition
    assert!(TestIgnoredMulConditionInstruction::<BasicHart<Reg<u64>>>::try_decode(2).is_none());
}
