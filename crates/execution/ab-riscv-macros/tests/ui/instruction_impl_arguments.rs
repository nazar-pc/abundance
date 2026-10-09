//! `#[instruction]` on an implementation takes no arguments, everything about the instruction set
//! is specified on the enum definition. Arguments must be rejected instead of being silently
//! ignored, which would make them look like they have an effect.

use ab_riscv_macros::instruction;

enum TestInstruction {
    A,
}

#[instruction(ignore = [A])]
impl Instruction for TestInstruction {}

fn main() {}
