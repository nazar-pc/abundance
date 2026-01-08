//! Composition tuples for instructions

use crate::instruction::{GenericBaseInstruction, GenericInstruction};
use core::fmt;

/// Tuple instruction that allows composing a base instruction type with an extension.
///
/// NOTE: All instructions in a tuple must use the same associated register type or else the
/// compiler will produce a lot of very confusing errors.
#[derive(Debug, Copy, Clone)]
pub enum Tuple2Instruction<A, Base>
where
    A: GenericInstruction<Base = Base>,
    Base: GenericBaseInstruction,
{
    A(A),
    Base(Base),
}

impl<A, Base> fmt::Display for Tuple2Instruction<A, Base>
where
    A: GenericInstruction<Base = Base>,
    Base: GenericBaseInstruction,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tuple2Instruction::A(a) => fmt::Display::fmt(a, f),
            Tuple2Instruction::Base(b) => fmt::Display::fmt(b, f),
        }
    }
}

impl<A, Base> const GenericInstruction for Tuple2Instruction<A, Base>
where
    A: [const] GenericInstruction<Base = Base>,
    Base: [const] GenericBaseInstruction,
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
    fn size(&self) -> usize {
        match self {
            Tuple2Instruction::A(a) => a.size(),
            Tuple2Instruction::Base(b) => b.size(),
        }
    }
}

impl<A, Base> const GenericBaseInstruction for Tuple2Instruction<A, Base>
where
    A: [const] GenericInstruction<Base = Base>,
    Base: [const] GenericBaseInstruction,
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
    A: GenericInstruction<Base = Base>,
    B: GenericInstruction<Base = Base>,
    Base: GenericBaseInstruction,
{
    A(A),
    B(B),
    Base(Base),
}

impl<A, B, Base> fmt::Display for Tuple3Instruction<A, B, Base>
where
    A: GenericInstruction<Base = Base>,
    B: GenericInstruction<Base = Base>,
    Base: GenericBaseInstruction,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tuple3Instruction::A(a) => fmt::Display::fmt(a, f),
            Tuple3Instruction::B(b) => fmt::Display::fmt(b, f),
            Tuple3Instruction::Base(b) => fmt::Display::fmt(b, f),
        }
    }
}

impl<A, B, Base> const GenericInstruction for Tuple3Instruction<A, B, Base>
where
    A: [const] GenericInstruction<Base = Base>,
    B: [const] GenericInstruction<Base = Base>,
    Base: [const] GenericBaseInstruction,
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
    fn size(&self) -> usize {
        match self {
            Tuple3Instruction::A(a) => a.size(),
            Tuple3Instruction::B(b) => b.size(),
            Tuple3Instruction::Base(b) => b.size(),
        }
    }
}

impl<A, B, Base> const GenericBaseInstruction for Tuple3Instruction<A, B, Base>
where
    A: [const] GenericInstruction<Base = Base>,
    B: [const] GenericInstruction<Base = Base>,
    Base: [const] GenericBaseInstruction,
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
