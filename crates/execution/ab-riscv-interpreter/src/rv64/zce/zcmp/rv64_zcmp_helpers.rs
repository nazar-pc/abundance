//! Opaque helpers for Zcmp extension

use crate::{ExecutionError, RegisterFile, VirtualMemory};
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

/// Execute CM.PUSH: store registers below sp, then decrement sp
#[inline]
#[doc(hidden)]
#[expect(clippy::type_complexity, reason = "Internal helper")]
pub fn do_push<Reg, Regs, Memory, CustomError>(
    regs: &mut Regs,
    memory: &mut Memory,
    urlist: ZcmpUrlist<Reg>,
    stack_adj: u8,
) -> Result<ControlFlow<(), (Reg, u64)>, ExecutionError<Reg::Type, CustomError>>
where
    Reg: ZcmpRegister<Type = u64>,
    Regs: RegisterFile<Reg>,
    Memory: VirtualMemory,
{
    let sp = regs.read(Reg::SP);
    // Store from sp-8 downward, highest-priority register first
    let mut store_addr = sp.wrapping_sub(size_of::<Reg::Type>() as u64);
    for reg in urlist.reg_list() {
        memory.write(store_addr, regs.read(reg))?;
        store_addr = store_addr.wrapping_sub(size_of::<Reg::Type>() as u64);
    }
    Ok(ControlFlow::Continue((
        Reg::SP,
        sp.wrapping_sub(u64::from(stack_adj)),
    )))
}

/// Execute CM.POP and variants: restore registers and increment sp.
/// Returns the value of ra (x1) for use with popret/popretz.
#[inline]
#[doc(hidden)]
pub fn do_pop<Reg, Regs, Memory, CustomError>(
    regs: &mut Regs,
    memory: &mut Memory,
    urlist: ZcmpUrlist<Reg>,
    stack_adj: u8,
) -> Result<u64, ExecutionError<Reg::Type, CustomError>>
where
    Reg: ZcmpRegister<Type = u64>,
    Regs: RegisterFile<Reg>,
    Memory: VirtualMemory,
{
    let sp = regs.read(Reg::SP);
    let new_sp = sp.wrapping_add(u64::from(stack_adj));
    // Restore from [new_sp-8, new_sp-16, ...], matching push order
    let mut load_addr = new_sp.wrapping_sub(size_of::<Reg::Type>() as u64);
    for reg in urlist.reg_list() {
        let value = memory.read::<u64>(load_addr)?;
        regs.write(reg, value);
        load_addr = load_addr.wrapping_sub(size_of::<Reg::Type>() as u64);
    }
    regs.write(Reg::SP, new_sp);
    Ok(regs.read(Reg::RA))
}
