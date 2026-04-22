use ab_riscv_interpreter::prelude::*;
use ab_riscv_macros::{instruction, instruction_execution};
use ab_riscv_primitives::prelude::*;
use core::fmt;
use core::ops::ControlFlow;

/// All instructions supported by the interpreter for RV32I base ISA
pub(crate) type AbundanceRv32IMaxInstruction = AbundanceRv32IMaxInstructionPrototype<Reg<u32>>;

/// All instructions supported by the interpreter for RV32I base ISA
#[instruction(
    inherit = [
        Rv32Instruction,
        Rv32BInstruction,
        Rv32MInstruction,
        Rv32ZbcInstruction,
        Rv32ZcaInstruction,
        Rv32ZcbInstruction,
        Rv32ZcmpInstruction,
        Rv32ZknInstruction,
        ZicondInstruction,
        ZicsrInstruction,
        Zve64xInstruction,
        MachineModePlaceholder,
    ],
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AbundanceRv32IMaxInstructionPrototype<Reg> {}

#[instruction]
impl<Reg> const Instruction for AbundanceRv32IMaxInstructionPrototype<Reg>
where
    Reg: [const] Register<Type = u32>,
{
    type Reg = Reg;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        None
    }

    #[inline(always)]
    fn alignment() -> u8 {
        align_of::<u32>() as u8
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

#[instruction]
impl<Reg> fmt::Display for AbundanceRv32IMaxInstructionPrototype<Reg>
where
    Reg: Register,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        InterpreterState<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for AbundanceRv32IMaxInstructionPrototype<Reg>
where
    Reg: Register<Type = u32>,
    Memory: VirtualMemory,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
    InstructionHandler: SystemInstructionHandler<Reg, Regs, Memory, PC, CustomError>,
{
    fn execute(
        self,
        state: &mut InterpreterState<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, CustomError>> {
        Ok(ControlFlow::Continue(()))
    }
}
