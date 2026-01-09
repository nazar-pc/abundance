//! Composition tuples for instructions

use crate::instruction::{BaseInstruction, Instruction};
use core::fmt;

/// Tuple instruction that allows composing a base instruction type with an extension.
///
/// NOTE: All instructions in a tuple must use the same associated register type or else the
/// compiler will produce a lot of very confusing errors.
#[derive(Debug, Copy, Clone)]
pub enum Tuple2Instruction<A, Base>
where
    A: Instruction<Base = Base>,
    Base: BaseInstruction,
{
    A(A),
    Base(Base),
}

impl<A, Base> fmt::Display for Tuple2Instruction<A, Base>
where
    A: Instruction<Base = Base>,
    Base: BaseInstruction,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tuple2Instruction::A(a) => fmt::Display::fmt(a, f),
            Tuple2Instruction::Base(b) => fmt::Display::fmt(b, f),
        }
    }
}

impl<A, Base> const Instruction for Tuple2Instruction<A, Base>
where
    A: [const] Instruction<Base = Base>,
    Base: [const] BaseInstruction,
{
    type Base = Base;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        if let Some(instruction) = A::try_decode(instruction) {
            Some(Self::A(instruction))
        } else {
            Some(Self::Base(Base::try_decode(instruction)?))
        }
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        match self {
            Tuple2Instruction::A(a) => a.size(),
            Tuple2Instruction::Base(b) => b.size(),
        }
    }
}

impl<A, Base> const BaseInstruction for Tuple2Instruction<A, Base>
where
    A: [const] Instruction<Base = Base>,
    Base: [const] BaseInstruction,
{
    type Reg = Base::Reg;

    #[inline(always)]
    fn from_base(base: Base) -> Self {
        Self::Base(base)
    }

    #[inline]
    fn decode(instruction: u32) -> Self {
        if let Some(instruction) = A::try_decode(instruction) {
            Self::A(instruction)
        } else {
            Self::Base(Base::decode(instruction))
        }
    }
}

/// Tuple instruction that allows composing a base instruction type with two extensions.
///
/// NOTE: All instructions in a tuple must use the same associated register type or else the
/// compiler will produce a lot of very confusing errors.
#[derive(Debug, Copy, Clone)]
pub enum Tuple3Instruction<A, B, Base>
where
    A: Instruction<Base = Base>,
    B: Instruction<Base = Base>,
    Base: BaseInstruction,
{
    A(A),
    B(B),
    Base(Base),
}

impl<A, B, Base> fmt::Display for Tuple3Instruction<A, B, Base>
where
    A: Instruction<Base = Base>,
    B: Instruction<Base = Base>,
    Base: BaseInstruction,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tuple3Instruction::A(a) => fmt::Display::fmt(a, f),
            Tuple3Instruction::B(b) => fmt::Display::fmt(b, f),
            Tuple3Instruction::Base(b) => fmt::Display::fmt(b, f),
        }
    }
}

impl<A, B, Base> const Instruction for Tuple3Instruction<A, B, Base>
where
    A: [const] Instruction<Base = Base>,
    B: [const] Instruction<Base = Base>,
    Base: [const] BaseInstruction,
{
    type Base = Base;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        if let Some(instruction) = A::try_decode(instruction) {
            Some(Self::A(instruction))
        } else if let Some(instruction) = B::try_decode(instruction) {
            Some(Self::B(instruction))
        } else {
            Some(Self::Base(Base::try_decode(instruction)?))
        }
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        match self {
            Tuple3Instruction::A(a) => a.size(),
            Tuple3Instruction::B(b) => b.size(),
            Tuple3Instruction::Base(b) => b.size(),
        }
    }
}

impl<A, B, Base> const BaseInstruction for Tuple3Instruction<A, B, Base>
where
    A: [const] Instruction<Base = Base>,
    B: [const] Instruction<Base = Base>,
    Base: [const] BaseInstruction,
{
    type Reg = Base::Reg;

    #[inline(always)]
    fn from_base(base: Base) -> Self {
        Self::Base(base)
    }

    #[inline]
    fn decode(instruction: u32) -> Self {
        if let Some(instruction) = A::try_decode(instruction) {
            Self::A(instruction)
        } else if let Some(instruction) = B::try_decode(instruction) {
            Self::B(instruction)
        } else {
            Self::Base(Base::decode(instruction))
        }
    }
}
