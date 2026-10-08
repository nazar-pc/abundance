//! `#[instruction]` on an implementation only accepts `Instruction` and `Display` trait
//! implementations. The implemented trait selects which generated file replaces the item, so
//! anything else must be rejected instead of including a file that is never generated.

use ab_riscv_macros::instruction;

enum TestInstruction {
    A,
}

// Inherent implementation, there is no trait to select the generated file with
#[instruction]
impl TestInstruction {
    fn a(&self) {}
}

// Trait implementation that has no generated counterpart
#[instruction]
impl Clone for TestInstruction {
    fn clone(&self) -> Self {
        Self::A
    }
}

fn main() {}
