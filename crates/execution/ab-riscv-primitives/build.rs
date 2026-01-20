use ab_riscv_macros::process_instruction_macros;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    process_instruction_macros()?;

    Ok(())
}
