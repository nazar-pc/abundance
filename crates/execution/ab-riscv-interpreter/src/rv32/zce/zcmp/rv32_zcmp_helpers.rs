//! Opaque helpers for Zcmp extension

use crate::{ExecutionError, ExecutionResult, RegisterFile, VirtualMemory};
use ab_riscv_primitives::prelude::*;

/// Execute CM.PUSH: store registers below sp, then decrement sp
#[inline]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn do_push<Reg, Regs, Memory>(
    regs: &mut Regs,
    memory: &mut Memory,
    urlist: ZcmpUrlist<Reg>,
    stack_adj: u8,
) -> ExecutionResult<Reg>
where
    Reg: ZcmpRegister<Type = u32>,
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
    ExecutionResult::Continue {
        rd: Reg::SP,
        value: sp.wrapping_sub(u32::from(stack_adj)),
    }
}

/// Execute CM.POP and variants: restore registers and increment sp.
/// Returns the value of ra (x1) for use with popret/popretz.
#[inline]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn do_pop<Reg, Regs, Memory>(
    regs: &mut Regs,
    memory: &mut Memory,
    urlist: ZcmpUrlist<Reg>,
    stack_adj: u8,
) -> Result<u32, ExecutionError<Reg::Type>>
where
    Reg: ZcmpRegister<Type = u32>,
    Regs: RegisterFile<Reg>,
    Memory: VirtualMemory,
{
    let sp = regs.read(Reg::SP);
    let new_sp = sp.wrapping_add(u32::from(stack_adj));
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
