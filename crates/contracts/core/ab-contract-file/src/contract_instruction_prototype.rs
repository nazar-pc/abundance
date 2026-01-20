use ab_riscv_interpreter::rv64::b::zbc::clmul_internal;
use ab_riscv_interpreter::rv64::{Rv64InterpreterState, Rv64SystemInstructionHandler};
use ab_riscv_interpreter::{ExecutableInstruction, ExecutionError, ProgramCounter, VirtualMemory};
use ab_riscv_macros::{instruction, instruction_execution};
use ab_riscv_primitives::instruction::Instruction;
use ab_riscv_primitives::instruction::rv64::Rv64Instruction;
use ab_riscv_primitives::instruction::rv64::b::zba::Rv64ZbaInstruction;
use ab_riscv_primitives::instruction::rv64::b::zbb::Rv64ZbbInstruction;
use ab_riscv_primitives::instruction::rv64::b::zbc::Rv64ZbcInstruction;
use ab_riscv_primitives::instruction::rv64::b::zbs::Rv64ZbsInstruction;
use ab_riscv_primitives::instruction::rv64::m::Rv64MInstruction;
use ab_riscv_primitives::registers::{EReg, Register};
use core::fmt;
use core::ops::ControlFlow;

/// Instructions that are the most popular among contracts
#[instruction(
    reorder = [
        Ld,
        Sd,
        Add,
        Addi,
        Xor,
        Rori,
        Srli,
        Or,
        And,
        Slli,
        Lbu,
        Auipc,
        Jalr,
        Sb,
        Roriw,
        Sub,
        Sltu,
        Mulhu,
        Mul,
        Sh1add,
    ],
    ignore = [
        Rv64Instruction,
        Rv64MInstruction,
        Rv64BInstruction,
        Rv64ZbcInstruction,
    ],
    inherit = [
        Rv64Instruction,
        Rv64MInstruction,
        Rv64BInstruction,
        Rv64ZbcInstruction,
    ],
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopularInstruction<Reg> {}

#[instruction]
impl<Reg> const Instruction for PopularInstruction<Reg>
where
    Reg: [const] Register<Type = u64>,
{
    type Reg = EReg<u64>;

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
impl<Reg> fmt::Display for PopularInstruction<Reg>
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
    > for PopularInstruction<Reg>
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

/// Instructions that are less popular among contracts
#[instruction(
    ignore = [PopularInstruction, Fence, Ecall],
    inherit = [
        Rv64Instruction,
        Rv64MInstruction,
        Rv64BInstruction,
        Rv64ZbcInstruction,
    ],
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotPopularInstruction<Reg> {}

#[instruction]
impl<Reg> const Instruction for NotPopularInstruction<Reg>
where
    Reg: [const] Register<Type = u64>,
{
    type Reg = EReg<u64>;

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
impl<Reg> fmt::Display for NotPopularInstruction<Reg>
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
    > for NotPopularInstruction<Reg>
where
    Reg: Register<Type = u64>,
    [(); Reg::N]:,
    Memory: VirtualMemory,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
    InstructionHandler: Rv64SystemInstructionHandler<Reg, Memory, PC, CustomError>,
{
    fn execute(
        self,
        state: &mut Rv64InterpreterState<Reg, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, Self, CustomError>> {
        Ok(ControlFlow::Continue(()))
    }
}

/// An instruction type used by contracts (prototype for macro usage)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractInstructionPrototype<Reg> {
    /// Instructions that are the most popular among contracts
    Popular(PopularInstruction<Reg>),
    /// Instructions that are less popular among contracts
    NotPopular(NotPopularInstruction<Reg>),
}

impl<Reg> const Instruction for ContractInstructionPrototype<Reg>
where
    Reg: [const] Register<Type = u64>,
{
    type Reg = EReg<u64>;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        if let Some(instruction) = PopularInstruction::try_decode(instruction).map(Self::Popular) {
            Some(instruction)
        } else {
            NotPopularInstruction::try_decode(instruction).map(Self::NotPopular)
        }
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

impl<Reg> fmt::Display for ContractInstructionPrototype<Reg>
where
    Reg: fmt::Display + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Popular(instructions) => fmt::Display::fmt(instructions, f),
            Self::NotPopular(instructions) => fmt::Display::fmt(instructions, f),
        }
    }
}

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
        match self {
            Self::Popular(instructions) => instructions
                .execute(state)
                .map_err(|error| error.map_instruction(Self::Popular)),
            Self::NotPopular(instructions) => instructions
                .execute(state)
                .map_err(|error| error.map_instruction(Self::NotPopular)),
        }
    }
}
