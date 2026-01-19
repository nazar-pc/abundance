use ab_riscv_interpreter::rv64::b::zbc::clmul_internal;
use ab_riscv_interpreter::rv64::{Rv64InterpreterState, Rv64SystemInstructionHandler};
use ab_riscv_interpreter::{ExecutableInstruction, ExecutionError, ProgramCounter, VirtualMemory};
use ab_riscv_macros::{instruction, instruction_execution};
use ab_riscv_primitives::instruction::rv64::Rv64Instruction;
use ab_riscv_primitives::instruction::rv64::b::zba::Rv64ZbaInstruction;
use ab_riscv_primitives::instruction::rv64::b::zbb::Rv64ZbbInstruction;
use ab_riscv_primitives::instruction::rv64::b::zbc::Rv64ZbcInstruction;
use ab_riscv_primitives::instruction::rv64::b::zbs::Rv64ZbsInstruction;
use ab_riscv_primitives::instruction::rv64::m::Rv64MInstruction;
use ab_riscv_primitives::instruction::{BaseInstruction, Instruction};
use ab_riscv_primitives::registers::{EReg, Register};
use core::fmt;
use core::ops::ControlFlow;

/// An instruction type used by contracts (prototype for macro usage)
#[instruction(
    inherit = [
        Rv64Instruction,
        Rv64MInstruction,
        Rv64BInstruction,
        Rv64ZbcInstruction,
    ],
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractInstructionPrototype<Reg> {}

#[instruction]
impl<Reg> const Instruction for ContractInstructionPrototype<Reg>
where
    Reg: [const] Register<Type = u64>,
{
    type Base = Rv64Instruction<Reg>;

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

impl<Reg> const BaseInstruction for ContractInstructionPrototype<Reg>
where
    Reg: [const] Register<Type = u64>,
{
    type Reg = EReg<u64>;

    #[inline(always)]
    fn decode(instruction: u32) -> Self {
        Self::try_decode(instruction).unwrap_or(Self::Invalid(instruction))
    }
}

#[instruction]
impl<Reg> fmt::Display for ContractInstructionPrototype<Reg>
where
    Reg: fmt::Display + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[instruction_execution]
impl<Reg, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        Rv64InterpreterState<Reg, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for ContractInstructionPrototype<Reg>
where
    Reg: Register<Type = u64>,
    [(); Reg::N]:,
    Memory: VirtualMemory,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
    InstructionHandler: Rv64SystemInstructionHandler<Reg, Memory, PC, CustomError>,
{
    #[inline(always)]
    fn execute(
        self,
        state: &mut Rv64InterpreterState<Reg, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, Self, CustomError>> {
        Ok(ControlFlow::Continue(()))
    }
}
