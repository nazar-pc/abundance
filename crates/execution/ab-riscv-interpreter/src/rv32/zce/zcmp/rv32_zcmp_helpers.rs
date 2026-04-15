//! Opaque helpers for Zcmp extension

use crate::{
    ExecutionError, InterpreterState, ProgramCounter, SystemInstructionHandler, VirtualMemory,
};
use ab_riscv_primitives::prelude::*;

/// Execute CM.PUSH: store registers below sp, then decrement sp
#[inline(always)]
#[doc(hidden)]
pub fn do_push<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>(
    state: &mut InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
    urlist: ZcmpUrlist<Reg>,
    stack_adj: u32,
) -> Result<(), ExecutionError<Reg::Type, CustomError>>
where
    Reg: Register<Type = u32>,
    [(); Reg::N]:,
    Memory: VirtualMemory,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
    InstructionHandler: SystemInstructionHandler<Reg, Memory, PC, CustomError>,
{
    let sp = state.regs.read(Reg::SP);
    // Store from sp-4 downward, highest-priority register first
    let mut store_addr = u64::from(sp.wrapping_sub(size_of::<Reg::Type>() as u32));
    for reg in urlist.reg_list() {
        state.memory.write(store_addr, state.regs.read(reg))?;
        store_addr = store_addr.wrapping_sub(size_of::<Reg::Type>() as u64);
    }
    state.regs.write(Reg::SP, sp.wrapping_sub(stack_adj));
    Ok(())
}

/// Execute CM.POP and variants: restore registers and increment sp.
/// Returns the value of ra (x1) for use with popret/popretz.
#[inline(always)]
#[doc(hidden)]
pub fn do_pop<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>(
    state: &mut InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
    urlist: ZcmpUrlist<Reg>,
    stack_adj: u32,
) -> Result<u32, ExecutionError<Reg::Type, CustomError>>
where
    Reg: Register<Type = u32>,
    [(); Reg::N]:,
    Memory: VirtualMemory,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
    InstructionHandler: SystemInstructionHandler<Reg, Memory, PC, CustomError>,
{
    let sp = state.regs.read(Reg::SP);
    let new_sp = sp.wrapping_add(stack_adj);
    // Restore from [new_sp-4, new_sp-8, ...], matching push order
    let mut load_addr = u64::from(new_sp.wrapping_sub(size_of::<Reg::Type>() as u32));
    for reg in urlist.reg_list() {
        let value = state.memory.read::<u32>(load_addr)?;
        state.regs.write(reg, value);
        load_addr = load_addr.wrapping_sub(size_of::<Reg::Type>() as u64);
    }
    state.regs.write(Reg::SP, new_sp);
    Ok(state.regs.read(Reg::RA))
}
