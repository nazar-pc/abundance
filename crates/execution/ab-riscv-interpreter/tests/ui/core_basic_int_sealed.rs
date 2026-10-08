//! `BasicInt` can't be implemented outside the crate. Memory implementations read arbitrary bytes
//! as `T: BasicInt`, which is only sound for plain integers where every bit pattern is valid.

use ab_riscv_interpreter::prelude::*;

// Not every bit pattern is a valid `bool`
#[derive(Clone, Copy)]
struct NotAnInteger(bool);

impl BasicInt for NotAnInteger {}

fn main() {}
