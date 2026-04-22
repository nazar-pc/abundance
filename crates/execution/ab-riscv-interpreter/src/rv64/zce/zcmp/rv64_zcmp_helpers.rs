//! Opaque helpers for Zcmp extension

use crate::{
    ExecutionError, InterpreterState, ProgramCounter, RegisterFile, SystemInstructionHandler,
    VirtualMemory,
};
use ab_riscv_primitives::prelude::*;

/// Execute CM.PUSH: store registers below sp, then decrement sp
#[inline(always)]
#[doc(hidden)]
pub fn do_push<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>(
    state: &mut InterpreterState<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>,
    urlist: ZcmpUrlist<Reg>,
    stack_adj: u32,
) -> Result<(), ExecutionError<Reg::Type, CustomError>>
where
    Reg: Register<Type = u64>,
    Regs: RegisterFile<Reg>,
    Memory: VirtualMemory,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
    InstructionHandler: SystemInstructionHandler<Reg, Regs, Memory, PC, CustomError>,
{
    let sp = state.regs.read(Reg::SP);
    // Store from sp-8 downward, highest-priority register first
    let mut store_addr = sp.wrapping_sub(size_of::<Reg::Type>() as u64);
    for reg in urlist.reg_list() {
        state.memory.write(store_addr, state.regs.read(reg))?;
        store_addr = store_addr.wrapping_sub(size_of::<Reg::Type>() as u64);
    }
    state
        .regs
        .write(Reg::SP, sp.wrapping_sub(u64::from(stack_adj)));
    Ok(())
}

/// Execute CM.POP and variants: restore registers and increment sp.
/// Returns the value of ra (x1) for use with popret/popretz.
#[inline(always)]
#[doc(hidden)]
pub fn do_pop<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>(
    state: &mut InterpreterState<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>,
    urlist: ZcmpUrlist<Reg>,
    stack_adj: u32,
) -> Result<u64, ExecutionError<Reg::Type, CustomError>>
where
    Reg: Register<Type = u64>,
    Regs: RegisterFile<Reg>,
    Memory: VirtualMemory,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
    InstructionHandler: SystemInstructionHandler<Reg, Regs, Memory, PC, CustomError>,
{
    let sp = state.regs.read(Reg::SP);
    let new_sp = sp.wrapping_add(u64::from(stack_adj));
    // Restore from [new_sp-8, new_sp-16, ...], matching push order
    let mut load_addr = new_sp.wrapping_sub(size_of::<Reg::Type>() as u64);
    for reg in urlist.reg_list() {
        let value = state.memory.read::<u64>(load_addr)?;
        state.regs.write(reg, value);
        load_addr = load_addr.wrapping_sub(size_of::<Reg::Type>() as u64);
    }
    state.regs.write(Reg::SP, new_sp);
    Ok(state.regs.read(Reg::RA))
}
