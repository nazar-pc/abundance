//! Path-qualified attributes on items in an inline module.
//!
//! This file intentionally has no top-level `#[instruction]` attributes, so
//! `process_instruction_macros()` must find indented path-qualified ones to process it at all.

use crate::hart::BasicHart;
use crate::instructions::Instruction;
use crate::registers::general_purpose::Reg;

#[expect(
    clippy::inline_modules,
    reason = "Checks that `process_instruction_macros()` finds items in inline modules"
)]
mod inner {
    use crate::hart::HartConfig;
    use crate::instructions::tests::TestIgnoredMulInstruction;
    use crate::instructions::{ImplementedExtension, Instruction, IsaExtension};
    use crate::registers::general_purpose::Register;
    use core::fmt;

    /// Instructions with conditions
    #[ab_riscv_macros::instruction]
    #[derive(Debug, Clone, Copy)]
    #[derive_const(PartialEq, Eq)]
    enum TestNestedConditionInstruction<Hart>
    where
        Hart: HartConfig,
    {
        #[ab_riscv_macros::instruction(if = [Foo])]
        Qux,
        #[ab_riscv_macros::instruction(if = [Mul])]
        Quux,
    }

    #[ab_riscv_macros::instruction]
    const impl<Reg, Hart> Instruction for TestNestedConditionInstruction<Hart>
    where
        Reg: [const] Register<Type = u64>,
        Hart: [const] HartConfig<Reg = Reg>,
    {
        const OWN_ISA_EXTENSIONS: &'static [IsaExtension] = &[];

        const ALIGNMENT: u8 = align_of::<u32>() as u8;

        type Hart = Hart;

        #[inline(always)]
        fn try_decode(instruction: u32) -> Option<Self> {
            match instruction {
                1 => Some(Self::Qux),
                2 => Some(Self::Quux),
                _ => None,
            }
        }

        #[inline(always)]
        fn size(&self) -> u8 {
            size_of::<u32>() as u8
        }
    }

    #[ab_riscv_macros::instruction]
    impl<Reg, Hart> fmt::Display for TestNestedConditionInstruction<Hart>
    where
        Reg: fmt::Display + Copy,
        Hart: HartConfig<Reg = Reg>,
    {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::Qux => write!(f, "qux"),
                Self::Quux => write!(f, "quux"),
            }
        }
    }

    #[ab_riscv_macros::instruction(
        inherit = [TestIgnoredMulInstruction, TestNestedConditionInstruction],
    )]
    #[derive(Debug, Clone, Copy)]
    #[derive_const(PartialEq, Eq)]
    pub(super) enum TestNestedInstruction<Hart>
    where
        Hart: HartConfig, {}

    #[ab_riscv_macros::instruction]
    const impl<Reg, Hart> Instruction for TestNestedInstruction<Hart>
    where
        Reg: [const] Register<Type = u64>,
        Hart: [const] HartConfig<Reg = Reg>,
    {
        const OWN_ISA_EXTENSIONS: &'static [IsaExtension] = &[];

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

    #[ab_riscv_macros::instruction]
    impl<Reg, Hart> fmt::Display for TestNestedInstruction<Hart>
    where
        Reg: fmt::Display + Copy,
        Hart: HartConfig<Reg = Reg>,
    {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {}
        }
    }
}

#[test]
fn nested_path_qualified() {
    type TestNestedInstruction = inner::TestNestedInstruction<BasicHart<Reg<u64>>>;

    // Decoding of `foo` is inherited from `TestIgnoredMulInstruction`
    assert!(matches!(
        TestNestedInstruction::try_decode(0),
        Some(TestNestedInstruction::Foo { .. })
    ));
    // `foo` is present, so `qux` is too
    assert!(matches!(
        TestNestedInstruction::try_decode(1),
        Some(TestNestedInstruction::Qux { .. })
    ));
    // `mul` is ignored, so `quux` is not present
    assert!(TestNestedInstruction::try_decode(2).is_none());
}
