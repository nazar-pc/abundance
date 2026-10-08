//! `#[instruction_execution]` only accepts `ExecutableInstructionOperands`,
//! `ExecutableInstructionCsr` and `ExecutableInstruction` trait implementations. The implemented
//! trait selects which generated file replaces the item, so anything else (including `Instruction`
//! that belongs to `#[instruction]`) must be rejected.

use ab_riscv_macros::instruction_execution;

trait Instruction {}

enum TestInstruction {
    A,
}

// Inherent implementation, there is no trait to select the generated file with
#[instruction_execution]
impl TestInstruction {
    fn a(&self) {}
}

// `Instruction` is handled by `#[instruction]`, not by `#[instruction_execution]`
#[instruction_execution]
impl Instruction for TestInstruction {}

fn main() {}
