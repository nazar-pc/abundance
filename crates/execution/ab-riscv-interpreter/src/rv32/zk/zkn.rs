//! RV32 Zkn extension

pub mod zknd;
pub mod zkne;
pub mod zknh;

use crate::rv32::b::zbc::rv32_zbc_helpers;
use crate::rv32::zk::zbkb::rv32_zbkb_helpers;
use crate::rv32::zk::zbkx::rv32_zbkx_helpers;
use crate::rv32::zk::zkn::zknd::rv32_zknd_helpers;
use crate::rv32::zk::zkn::zkne::rv32_zkne_helpers;
use crate::rv32::zk::zkn::zknh::rv32_zknh_helpers;
use crate::{ExecutableInstruction, ExecutionError, RegisterFile, Rs1Rs2Operands};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for Rv32ZknInstruction<Reg>
where
    Reg: Register<Type = u32>,
{
    #[inline(always)]
    fn execute(
        self,
        regs: &mut Regs,
        _ext_state: &mut ExtState,
        _memory: &mut Memory,
        _program_counter: &mut PC,
        _system_instruction_handler: &mut InstructionHandler,
    ) -> Result<
        ControlFlow<(), (Self::Reg, <Self::Reg as Register>::Type)>,
        ExecutionError<Reg::Type, CustomError>,
    > {
        Ok(ControlFlow::Continue(Default::default()))
    }
}
