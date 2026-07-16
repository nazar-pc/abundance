#![feature(
    const_cmp,
    const_trait_impl,
    const_try,
    const_try_residual,
    signed_bigint_helpers,
    try_blocks
)]

mod elf;
mod instruction;
mod interpreter;
mod time_csr;

use crate::elf::{LoadedElf, load_elf};
use crate::interpreter::{EagerInstructionFetcher, GuestMemory};
use crate::time_csr::TimeCsrState;
use ab_riscv_interpreter::basic::{
    BasicInterpreterState, BasicRegisters, IllegalEcallSystemInstructionHandler,
};
use ab_riscv_interpreter::prelude::*;
use ab_riscv_primitives::prelude::*;
use anyhow::Context;
use std::ffi::CStr;

/// Coremark ELF binary compiled by build.rs for the RISC-V guest
const COREMARK_ELF: &[u8] = include_bytes!(env!("COREMARK_ELF"));
/// Guest virtual address of the trap / return sentinel.
///
/// The caller writes this into `ra` before calling `main`; when `main` returns, the interpreter
/// sees PC = 0 and halts.
const TRAP_ADDRESS: u64 = 0x0;
/// Base address at which the PIE ELF is loaded into guest memory.
///
/// Address 0 is safe as a trap sentinel because `set_pc` checks for `TRAP_ADDRESS` before any
/// memory access, so the interpreter halts cleanly without ever dereferencing it.
const MEMORY_BASE_ADDRESS: u64 = 0x0;
/// Total guest memory size.
///
/// Must be large enough to hold the ELF segments, stack, and output buffer.
const MEMORY_SIZE: usize = 512 * 1024;

/// Read the null-terminated Coremark output string from the output buffer
fn read_output<Memory>(memory: &Memory, addr: u64, size: u32) -> Option<&str>
where
    Memory: VirtualMemory,
{
    let slice = memory.read_slice_up_to(addr, size);
    CStr::from_bytes_until_nul(slice).ok()?.to_str().ok()
}

fn main() -> anyhow::Result<()> {
    if COREMARK_ELF.is_empty() {
        return Err(anyhow::anyhow!(
            "Coremark ELF not found, install `riscv64-unknown-elf-gcc` and/or specify `RISCV_CC` \
            environment variable to specify a different toolchain, use `build-elf-required` \
            feature to make ELF building required"
        ));
    }
    let mut memory = GuestMemory::<MEMORY_BASE_ADDRESS, MEMORY_SIZE>::default();
    let LoadedElf {
        entry_point,
        global_pointer,
        text_addr,
        text_data,
        output_buf_addr,
        output_buf_size,
    } = load_elf(COREMARK_ELF, &mut memory)?;

    // argv is a pointer-to-pointer: write output_buf_addr as a `u64` into guest memory, then pass
    // its address in a1. Stack pointer sits below that, 16-byte aligned per psABI.
    let stack_top = (MEMORY_BASE_ADDRESS + MEMORY_SIZE as u64) & !0xF;
    let argv_addr = stack_top - 8;
    let stack_pointer = argv_addr - 8;

    memory
        .write::<u64>(argv_addr, output_buf_addr)
        .context("argv slot does not fit in guest memory")?;

    let host_start = std::time::Instant::now();

    let mut regs = BasicRegisters::default();
    regs.write(Reg::Ra, TRAP_ADDRESS);
    regs.write(Reg::Sp, stack_pointer);
    regs.write(Reg::Gp, global_pointer);
    regs.write(Reg::A0, 1);
    regs.write(Reg::A1, argv_addr);

    // SAFETY: entry_point is valid and aligned; ELF was produced by a trusted compiler
    let instruction_fetcher =
        unsafe { EagerInstructionFetcher::new(text_data, TRAP_ADDRESS, text_addr, entry_point) };

    let mut state = BasicInterpreterState {
        regs,
        ext_state: TimeCsrState::default(),
        memory,
        instruction_fetcher,
        system_instruction_handler: IllegalEcallSystemInstructionHandler,
    };

    state.execute().context("Coremark execution failed")?;

    let host_elapsed = host_start.elapsed();

    let output = read_output(&state.memory, output_buf_addr, output_buf_size)
        .context("Coremark output not found in guest memory")?;
    print!("{output}");

    println!("Host elapsed: {:.3} s", host_elapsed.as_secs_f64());

    Ok(())
}
