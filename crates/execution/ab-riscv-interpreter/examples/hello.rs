//! The smallest thing that runs a real program: RV32I, no extensions, no `unsafe`.
//!
//! Everything the interpreter needs is taken from the [`basic`] module: a register file, a flat
//! memory, an instruction fetcher that decodes straight out of that memory, and an execution
//! environment that refuses `ecall`. The guest is `examples/guests/src/bin/hello.rs`, compiled for
//! `riscv32i-unknown-none-elf`, so [`Rv32Instruction`] alone decodes all of it.
//!
//! Since `ecall` is not available, the guest communicates the only way that is left: it writes into
//! memory that the host reads afterward.
//!
//! [`basic`]: ab_riscv_interpreter::basic

use ab_riscv_interpreter::basic::{
    BasicInstructionFetcher, BasicInterpreterState, BasicMemory, BasicRegisters,
    IllegalEcallSystemInstructionHandler,
};
use ab_riscv_interpreter::prelude::*;
use ab_riscv_primitives::prelude::*;
use anyhow::Context;
use object::{Object, ObjectSection};
use std::str;

/// The guest program, see `examples/guests` in the repository
const GUEST_ELF: &[u8] = include_bytes!("prebuilt/hello.elf");
/// What the guest is expected to write into the output buffer
const GREETING: &str = "Hello from a RISC-V guest!";
/// Guest memory base address, which is where the guest ELF is linked to be loaded
const MEMORY_BASE_ADDRESS: u32 = 0x1_0000;
/// Guest memory size, generous enough for the program, its stack, and the output buffer
const MEMORY_SIZE: usize = 64 * 1024;
/// Address at which the interpreter stops execution gracefully.
///
/// The guest returns to whatever is in `ra` when it is done, so putting this address there is how
/// the program gets to finish. Nothing is ever fetched from it, which is why an address outside
/// guest memory is fine.
const TRAP_ADDRESS: u32 = 0;
/// Where the guest writes the greeting, comfortably past the program image and below the stack
const OUTPUT_BUFFER_ADDRESS: u32 = MEMORY_BASE_ADDRESS + MEMORY_SIZE as u32 / 2;
/// Stack pointer at the top of guest memory, 16-byte aligned as the psABI requires
const STACK_POINTER: u32 = (MEMORY_BASE_ADDRESS + MEMORY_SIZE as u32) & !0xf;

/// The instruction set the interpreter is built for: the base ISA and nothing else
type GuestInstruction = Rv32Instruction<Reg<u32>>;

fn main() -> anyhow::Result<()> {
    let mut memory = BasicMemory::<{ MEMORY_BASE_ADDRESS as u64 }, MEMORY_SIZE>::default();

    // The guest is statically linked and has no relocations, so loading it is nothing but copying
    // every section that has an address to that address
    let elf = object::File::parse(GUEST_ELF).context("Failed to parse guest ELF")?;
    for section in elf.sections().filter(|section| section.address() != 0) {
        let address = section.address();
        let data = section.data().context("Failed to read guest ELF section")?;
        memory
            .write_slice(address, data)
            .map_err(anyhow::Error::from)
            .with_context(|| format!("Section at {address:#x} does not fit into guest memory"))?;
    }
    let entry_point = u32::try_from(elf.entry()).context("Guest ELF is not a 32-bit program")?;

    let mut regs = BasicRegisters::<Reg<u32>>::default();
    // Arguments and the return address, exactly as a RISC-V caller would set them up
    regs.write(Reg::Ra, TRAP_ADDRESS);
    regs.write(Reg::Sp, STACK_POINTER);
    regs.write(Reg::A0, OUTPUT_BUFFER_ADDRESS);

    let mut state = BasicInterpreterState {
        regs,
        env: IllegalEcallSystemInstructionHandler,
        memory,
        instruction_fetcher: BasicInstructionFetcher::<GuestInstruction>::new(
            TRAP_ADDRESS,
            entry_point,
        ),
    };

    state
        .execute::<GuestInstruction>()
        .context("Guest execution failed")?;

    // The greeting length is where the guest left it: in the register the psABI returns values in
    let length = state.regs.read(Reg::A0);
    let greeting = state
        .memory
        .read_slice(u64::from(OUTPUT_BUFFER_ADDRESS), length)
        .map_err(anyhow::Error::from)
        .context("Greeting is not in guest memory")?;
    let greeting = str::from_utf8(greeting).context("Greeting is not valid UTF-8")?;

    println!("Guest said: {greeting}");
    assert_eq!(greeting, GREETING);

    Ok(())
}
