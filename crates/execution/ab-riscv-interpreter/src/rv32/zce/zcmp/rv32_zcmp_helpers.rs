//! Opaque helpers for Zcmp extension

use crate::{ExecutionError, RegisterFile, VirtualMemory};
use ab_riscv_primitives::prelude::*;

/// Execute CM.PUSH: store registers below sp, then decrement sp
#[inline(always)]
#[doc(hidden)]
pub fn do_push<Reg, Regs, Memory, CustomError>(
    regs: &mut Regs,
    memory: &mut Memory,
    urlist: ZcmpUrlist<Reg>,
    stack_adj: u32,
) -> Result<(), ExecutionError<Reg::Type, CustomError>>
where
    Reg: Register<Type = u32>,
    Regs: RegisterFile<Reg>,
    Memory: VirtualMemory,
{
    let sp = regs.read(Reg::SP);
    // Store from sp-4 downward, highest-priority register first
    let mut store_addr = u64::from(sp.wrapping_sub(size_of::<Reg::Type>() as u32));
    for reg in urlist.reg_list() {
        memory.write(store_addr, regs.read(reg))?;
        store_addr = store_addr.wrapping_sub(size_of::<Reg::Type>() as u64);
    }
    regs.write(Reg::SP, sp.wrapping_sub(stack_adj));
    Ok(())
}

/// Execute CM.POP and variants: restore registers and increment sp.
/// Returns the value of ra (x1) for use with popret/popretz.
#[inline(always)]
#[doc(hidden)]
pub fn do_pop<Reg, Regs, Memory, CustomError>(
    regs: &mut Regs,
    memory: &mut Memory,
    urlist: ZcmpUrlist<Reg>,
    stack_adj: u32,
) -> Result<u32, ExecutionError<Reg::Type, CustomError>>
where
    Reg: Register<Type = u32>,
    Regs: RegisterFile<Reg>,
    Memory: VirtualMemory,
{
    let sp = regs.read(Reg::SP);
    let new_sp = sp.wrapping_add(stack_adj);
    // Restore from [new_sp-4, new_sp-8, ...], matching push order
    let mut load_addr = u64::from(new_sp.wrapping_sub(size_of::<Reg::Type>() as u32));
    for reg in urlist.reg_list() {
        let value = memory.read::<u32>(load_addr)?;
        regs.write(reg, value);
        load_addr = load_addr.wrapping_sub(size_of::<Reg::Type>() as u64);
    }
    regs.write(Reg::SP, new_sp);
    Ok(regs.read(Reg::RA))
}
