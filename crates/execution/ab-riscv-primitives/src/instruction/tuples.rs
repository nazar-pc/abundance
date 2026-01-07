//! Composition tuples for instructions

use crate::instruction::{GenericBaseInstruction, GenericInstruction};
use core::fmt;

/// Tuple instruction that allows composing a base instruction type with an extension
#[derive(Debug, Copy, Clone)]
pub enum Tuple2Instruction<A, Base> {
    A(A),
    Base(Base),
}

impl<A, Base> fmt::Display for Tuple2Instruction<A, Base>
where
    A: fmt::Display,
    Base: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tuple2Instruction::A(a) => a.fmt(f),
            Tuple2Instruction::Base(b) => b.fmt(f),
        }
    }
}

impl<A, Base> const GenericInstruction for Tuple2Instruction<A, Base>
where
    A: [const] GenericInstruction,
    Base: [const] GenericBaseInstruction,
{
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
    A: [const] GenericInstruction,
    Base: [const] GenericBaseInstruction,
{
    #[inline]
    fn decode(instruction: u32) -> Self {
        if let Some(instruction) = A::try_decode(instruction) {
            Self::A(instruction)
        } else {
            Self::Base(Base::decode(instruction))
        }
    }
}

/// Tuple instruction that allows composing a base instruction type with two extensions
#[derive(Debug, Copy, Clone)]
pub enum Tuple3Instruction<A, B, Base> {
    A(A),
    B(B),
    Base(Base),
}

impl<A, B, Base> fmt::Display for Tuple3Instruction<A, B, Base>
where
    A: fmt::Display,
    B: fmt::Display,
    Base: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tuple3Instruction::A(a) => a.fmt(f),
            Tuple3Instruction::B(b) => b.fmt(f),
            Tuple3Instruction::Base(b) => b.fmt(f),
        }
    }
}

impl<A, B, Base> const GenericInstruction for Tuple3Instruction<A, B, Base>
where
    A: [const] GenericInstruction,
    B: [const] GenericInstruction,
    Base: [const] GenericBaseInstruction,
{
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
    A: [const] GenericInstruction,
    B: [const] GenericInstruction,
    Base: [const] GenericBaseInstruction,
{
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
