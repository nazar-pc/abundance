//! Zve64x extension

pub mod arith;
pub mod config;
pub mod fixed_point;
pub mod load;
pub mod mask;
pub mod muldiv;
pub mod perm;
pub mod reduction;
pub mod store;
pub mod widen_narrow;
pub mod zve64x_helpers;

use crate::v::vector_registers::VectorRegistersExt;
use crate::v::zve64x::arith::zve64x_arith_helpers;
use crate::v::zve64x::config::zve64x_config_helpers;
use crate::v::zve64x::fixed_point::zve64x_fixed_point_helpers;
use crate::v::zve64x::load::zve64x_load_helpers;
use crate::v::zve64x::mask::zve64x_mask_helpers;
use crate::v::zve64x::muldiv::zve64x_muldiv_helpers;
use crate::v::zve64x::perm::zve64x_perm_helpers;
use crate::v::zve64x::reduction::zve64x_reduction_helpers;
use crate::v::zve64x::store::zve64x_store_helpers;
use crate::v::zve64x::widen_narrow::zve64x_widen_narrow_helpers;
use crate::zicsr::zicsr_helpers;
use crate::{
    CsrError, Csrs, ExecutableInstruction, ExecutionError, ProgramCounter, RegisterFile,
    VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::fmt;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for Zve64xInstruction<Reg>
where
    Reg: Register,
{
    #[inline(always)]
    fn execute(
        self,
        regs: &mut Regs,
        ext_state: &mut ExtState,
        memory: &mut Memory,
        program_counter: &mut PC,
        _system_instruction_handler: &mut InstructionHandler,
    ) -> Result<
        ControlFlow<(), (Self::Reg, <Self::Reg as Register>::Type)>,
        ExecutionError<Reg::Type, CustomError>,
    > {
        Ok(ControlFlow::Continue(Default::default()))
    }
}
