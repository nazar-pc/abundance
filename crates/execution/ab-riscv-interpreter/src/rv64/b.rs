//! RV64 B extension

pub mod zba;
pub mod zbb;
pub mod zbc;
pub mod zbs;

use crate::rv64::Rv64InterpreterState;
use crate::{ExecutableInstruction, ExecutionError};
use ab_riscv_primitives::instruction::rv64::b::{Rv64BInstruction, Rv64BZbcInstruction};
use ab_riscv_primitives::registers::Register;
use core::ops::ControlFlow;

impl<Reg, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        Rv64InterpreterState<Reg, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for Rv64BInstruction<Reg>
where
    Reg: Register<Type = u64>,
    [(); Reg::N]:,
{
    #[inline(always)]
    fn execute(
        self,
        state: &mut Rv64InterpreterState<Reg, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, Self, CustomError>> {
        match self {
            Self::Zba(instruction) => instruction
                .execute(state)
                .map_err(|error| error.map_instruction(Self::Zba)),
            Self::Zbb(instruction) => instruction
                .execute(state)
                .map_err(|error| error.map_instruction(Self::Zbb)),
            Self::Zbs(instruction) => instruction
                .execute(state)
                .map_err(|error| error.map_instruction(Self::Zbs)),
        }
    }
}

impl<Reg, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        Rv64InterpreterState<Reg, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for Rv64BZbcInstruction<Reg>
where
    Reg: Register<Type = u64>,
    [(); Reg::N]:,
{
    #[inline(always)]
    fn execute(
        self,
        state: &mut Rv64InterpreterState<Reg, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, Self, CustomError>> {
        match self {
            Self::Zba(instruction) => instruction
                .execute(state)
                .map_err(|error| error.map_instruction(Self::Zba)),
            Self::Zbb(instruction) => instruction
                .execute(state)
                .map_err(|error| error.map_instruction(Self::Zbb)),
            Self::Zbc(instruction) => instruction
                .execute(state)
                .map_err(|error| error.map_instruction(Self::Zbc)),
            Self::Zbs(instruction) => instruction
                .execute(state)
                .map_err(|error| error.map_instruction(Self::Zbs)),
        }
    }
}
