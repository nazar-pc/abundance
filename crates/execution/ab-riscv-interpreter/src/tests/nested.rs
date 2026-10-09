//! Path-qualified attributes on items in an inline module.
//!
//! This file intentionally has no top-level `#[instruction]` or `#[instruction_execution]`
//! attributes, so `process_instruction_macros()` must find indented path-qualified ones to process
//! it at all.

use crate::RegisterFile;
use crate::rv64::test_utils::{execute, initialize_state};
use ab_riscv_primitives::prelude::*;

#[expect(
    clippy::inline_modules,
    reason = "Checks that `process_instruction_macros()` finds items in inline modules"
)]
mod inner {
    use crate::{
        ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands,
        ExecutionError, ExecutionResult, FetchInstructionResult, InstructionFetcher,
        OpaqueThreadedExecutionResult, RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands,
        ThreadedExecutableInstruction, ThreadedExecutionResult,
    };
    use ab_riscv_primitives::prelude::*;
    use core::fmt;

    #[ab_riscv_macros::instruction(inherit = [ZicondInstruction])]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(super) enum TestNestedZicond<Hart>
    where
        Hart: HartConfig, {}

    #[ab_riscv_macros::instruction]
    impl<Reg, Hart> Instruction for TestNestedZicond<Hart>
    where
        Reg: Register,
        Hart: HartConfig<Reg = Reg>,
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
    impl<Reg, Hart> fmt::Display for TestNestedZicond<Hart>
    where
        Reg: fmt::Display + Copy,
        Hart: HartConfig<Reg = Reg>,
    {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {}
        }
    }

    #[ab_riscv_macros::instruction_execution]
    impl<Reg, Hart> ExecutableInstructionOperands for TestNestedZicond<Hart>
    where
        Reg: Register,
        Hart: HartConfig<Reg = Reg>,
    {
    }

    #[ab_riscv_macros::instruction_execution]
    impl<Reg, Hart, Env> ExecutableInstructionCsr<Env> for TestNestedZicond<Hart>
    where
        Reg: Register,
        Hart: HartConfig<Reg = Reg>,
    {
    }

    #[ab_riscv_macros::instruction_execution]
    impl<Reg, Hart, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
        for TestNestedZicond<Hart>
    where
        Reg: Register,
        Hart: HartConfig<Reg = Reg>,
        Regs: RegisterFile<Reg>,
    {
        #[inline(always)]
        fn execute(
            self,
            Rs1Rs2OperandValues {
                rs1_value,
                rs2_value,
            }: Rs1Rs2OperandValues<Reg::Type>,
            _regs: &mut Regs,
            _env: &mut Env,
            _memory: &mut Memory,
            _program_counter: &mut PC,
        ) -> ExecutionResult<Reg> {
            ExecutionResult::ContinueNoWrite
        }
    }
}

#[test]
fn nested_path_qualified() {
    // Execution of `czero.eqz` is inherited from `ZicondInstruction`
    let mut state = initialize_state([inner::TestNestedZicond::CzeroEqz {
        rd: Reg::A2,
        rs1: Reg::A0,
        rs2: Reg::A1,
    }]);
    state.regs.write(Reg::A0, 1u64);
    state.regs.write(Reg::A1, 1u64);

    execute(&mut state).unwrap();

    assert_eq!(state.regs.read(Reg::A2), 1);
}
