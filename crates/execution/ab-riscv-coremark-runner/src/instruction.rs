use crate::time_csr::{TimeCsrInstruction, TimeCsrState};
use ab_riscv_interpreter::prelude::*;
use ab_riscv_macros::{instruction, instruction_execution};
use ab_riscv_primitives::prelude::*;
use core::fmt;
use core::ops::ControlFlow;

pub(crate) type CoremarkRegister = Reg<u64>;

/// An instruction type used by Coremark runner
#[instruction(
    inherit = [
        Rv64Instruction,
        Rv64MInstruction,
        Rv64BInstruction,
        Rv64ZcaInstruction,
        TimeCsrInstruction,
    ],
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CoremarkInstruction<Hart = BasicHart<CoremarkRegister>>
where
    Hart: HartConfig, {}

#[instruction]
const impl<Reg, Hart> Instruction for CoremarkInstruction<Hart>
where
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

#[instruction]
impl<Reg, Hart> fmt::Display for CoremarkInstruction<Hart>
where
    Reg: Register,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[instruction_execution]
impl<Reg, Hart> ExecutableInstructionOperands for CoremarkInstruction<Hart> where
    Hart: HartConfig<Reg = Reg>
{
}

#[instruction_execution]
impl<Reg, Hart, Env> ExecutableInstructionCsr<Env> for CoremarkInstruction<Hart> where
    Hart: HartConfig<Reg = Reg>
{
}

#[instruction_execution]
impl<Reg, Hart, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
    for CoremarkInstruction<Hart>
where
    Reg: Register,
    Hart: HartConfig<Reg = Reg>,
{
    #[inline(always)]
    fn execute(
        self,
        Rs1Rs2OperandValues {
            rs1_value,
            rs2_value,
        }: Rs1Rs2OperandValues<Reg::Type>,
        regs: &mut Regs,
        env: &mut Env,
        memory: &mut Memory,
        program_counter: &mut PC,
    ) -> ExecutionResult<Reg> {
        ExecutionResult::ContinueNoWrite
    }
}
