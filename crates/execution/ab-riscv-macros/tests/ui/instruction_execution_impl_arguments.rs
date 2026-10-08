//! `#[instruction_execution]` takes no arguments, everything about the instruction set is
//! specified on the enum definition. Arguments must be rejected instead of being silently ignored,
//! which would make them look like they have an effect.

use ab_riscv_macros::instruction_execution;

enum TestInstruction {
    A,
}

#[instruction_execution(ignore = [A])]
impl ExecutableInstruction for TestInstruction {}

fn main() {}
