use ab_riscv_interpreter::rv64::b::zbc::zbc_helpers;
use ab_riscv_interpreter::{
    ExecutableInstruction, ExecutionError, InterpreterState, ProgramCounter,
    SystemInstructionHandler, VirtualMemory,
};
use ab_riscv_macros::{instruction, instruction_execution};
use ab_riscv_primitives::instructions::Instruction;
use ab_riscv_primitives::instructions::rv64::b::zba::Rv64ZbaInstruction;
use ab_riscv_primitives::instructions::rv64::b::zbb::Rv64ZbbInstruction;
use ab_riscv_primitives::instructions::rv64::b::zbc::Rv64ZbcInstruction;
use ab_riscv_primitives::instructions::rv64::b::zbs::Rv64ZbsInstruction;
use ab_riscv_primitives::registers::general_purpose::Register;
use core::fmt;
use core::ops::ControlFlow;

/// B(Zba+Zbb+Zbs)+Zbc
#[instruction(inherit = [Rv64BInstruction, Rv64ZbcInstruction])]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FullRv64BInstruction<Reg> {}

#[instruction]
impl<Reg> const Instruction for FullRv64BInstruction<Reg>
where
    Reg: [const] Register<Type = u64>,
{
    type Reg = Reg;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        None
    }

    #[inline(always)]
    fn alignment() -> u8 {
        size_of::<u32>() as u8
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

#[instruction]
impl<Reg> fmt::Display for FullRv64BInstruction<Reg>
where
    Reg: fmt::Display + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[instruction_execution]
impl<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for FullRv64BInstruction<Reg>
where
    Reg: Register<Type = u64>,
    [(); Reg::N]:,
    Memory: VirtualMemory,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
    InstructionHandler: SystemInstructionHandler<Reg, Memory, PC, CustomError>,
{
    fn execute(
        self,
        state: &mut InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, CustomError>> {
        Ok(ControlFlow::Continue(()))
    }
}
