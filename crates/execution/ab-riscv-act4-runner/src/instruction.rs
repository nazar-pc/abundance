use ab_riscv_interpreter::prelude::*;
use ab_riscv_macros::{instruction, instruction_execution};
use ab_riscv_primitives::prelude::*;
use std::fmt;
use std::ops::ControlFlow;

/// Placeholder implementation for machine mode, which the interpreter doesn't support directly
#[instruction(
    inherit = [ZicsrInstruction],
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// TODO: Do something in the generated code that requires an import and suppresses this naturally
#[expect(dead_code, reason = "Used as a dependency below, so not truly unused")]
pub(crate) enum MachineModePlaceholder<Reg> {}

#[instruction]
impl<Reg> const Instruction for MachineModePlaceholder<Reg>
where
    Reg: [const] Register,
{
    type Reg = Reg;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        None
    }

    #[inline(always)]
    fn alignment() -> u8 {
        align_of::<u32>() as u8
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

#[instruction]
impl<Reg> fmt::Display for MachineModePlaceholder<Reg>
where
    Reg: fmt::Display + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for MachineModePlaceholder<Reg>
where
    Reg: Register,
{
    fn prepare_csr_read<C>(
        _csrs: &C,
        csr_index: u16,
        raw_value: Reg::Type,
        output_value: &mut Reg::Type,
    ) -> Result<bool, CsrError<CustomError>>
    where
        C: Csrs<Self::Reg, CustomError>,
    {
        if let Some(
            MCsr::Mstatus | MCsr::Mtvec | MCsr::Mscratch | MCsr::Mepc | MCsr::Mcause | MCsr::Mtval,
        ) = MCsr::from_index(csr_index)
        {
            *output_value = raw_value;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn prepare_csr_write<C>(
        _csrs: &mut C,
        csr_index: u16,
        write_value: Reg::Type,
        output_value: &mut Reg::Type,
    ) -> Result<bool, CsrError<CustomError>>
    where
        C: Csrs<Self::Reg, CustomError>,
    {
        if let Some(
            MCsr::Mstatus | MCsr::Mtvec | MCsr::Mscratch | MCsr::Mepc | MCsr::Mcause | MCsr::Mtval,
        ) = MCsr::from_index(csr_index)
        {
            *output_value = write_value;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn execute(
        self,
        regs: &mut Regs,
        ext_state: &mut ExtState,
        _memory: &mut Memory,
        _program_counter: &mut PC,
        _system_instruction_handler: &mut InstructionHandler,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, CustomError>> {
        Ok(ControlFlow::Continue(()))
    }
}
