use ab_riscv_interpreter::prelude::*;
use ab_riscv_macros::{instruction, instruction_execution};
use ab_riscv_primitives::prelude::*;
use std::fmt;

/// First `mhpmeventN` CSR index (`N` starts at 3, the first non-reserved HPM event selector).
pub(crate) const MHPMEVENT3_CSR_INDEX: u16 = 0x323;
/// Last `mhpmeventN` CSR index.
pub(crate) const MHPMEVENT31_CSR_INDEX: u16 = 0x33F;

/// Whether `csr_index` is one of the `mhpmevent3..31` CSRs.
pub(crate) const fn is_mhpmevent(csr_index: u16) -> bool {
    csr_index >= MHPMEVENT3_CSR_INDEX && csr_index <= MHPMEVENT31_CSR_INDEX
}

/// Placeholder implementation for machine mode, which the interpreter doesn't support directly
#[instruction(
    inherit = [ZicsrInstruction],
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MachineModePlaceholder<Reg> {}

#[instruction]
const impl<Reg> Instruction for MachineModePlaceholder<Reg> {
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
impl<Reg> ExecutableInstructionOperands for MachineModePlaceholder<Reg> where Reg: Register {}

#[instruction_execution]
impl<Reg, ExtState> ExecutableInstructionCsr<ExtState> for MachineModePlaceholder<Reg>
where
    Reg: Register,
{
    fn prepare_csr_read(
        _ext_state: &ExtState,
        csr_index: u16,
        _will_write: bool,
        raw_value: Reg::Type,
        output_value: &mut Reg::Type,
    ) -> Result<bool, CsrError> {
        if matches!(
            MCsr::from_index(csr_index),
            Some(
                MCsr::Mstatus
                    | MCsr::Misa
                    | MCsr::Mie
                    | MCsr::Mtvec
                    | MCsr::Mstatush
                    | MCsr::Mcountinhibit
                    | MCsr::Mscratch
                    | MCsr::Mepc
                    | MCsr::Mcause
                    | MCsr::Mtval
                    | MCsr::Mip
            )
        ) || crate::instruction::is_mhpmevent(csr_index)
        {
            *output_value = raw_value;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn prepare_csr_write(
        _ext_state: &mut ExtState,
        csr_index: u16,
        write_value: Reg::Type,
        output_value: &mut Reg::Type,
    ) -> Result<bool, CsrError> {
        match MCsr::from_index(csr_index) {
            Some(
                MCsr::Mstatus
                | MCsr::Mie
                | MCsr::Mtvec
                | MCsr::Mstatush
                | MCsr::Mcountinhibit
                | MCsr::Mscratch
                | MCsr::Mcause
                | MCsr::Mtval
                | MCsr::Mip,
            ) => {
                *output_value = write_value;
                Ok(true)
            }
            Some(MCsr::Mepc) => {
                *output_value = write_value & !Reg::Type::from(1u32);
                Ok(true)
            }
            Some(MCsr::Misa) => {
                // MISA_CSR_IMPLEMENTED is false for this core: misa is hardwired to 0 and writes
                // are WARL-ignored rather than illegal
                *output_value = Reg::Type::from(0u32);
                Ok(true)
            }
            _ => {
                if crate::instruction::is_mhpmevent(csr_index) {
                    *output_value = write_value;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
        }
    }
}

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler>
    for MachineModePlaceholder<Reg>
where
    Reg: Register,
{
    fn execute(
        self,
        Rs1Rs2OperandValues {
            rs1_value,
            rs2_value: _,
        }: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
        _regs: &mut Regs,
        ext_state: &mut ExtState,
        _memory: &mut Memory,
        _program_counter: &mut PC,
        _system_instruction_handler: &mut InstructionHandler,
    ) -> ExecutionResult<Self::Reg> {
        ExecutionResult::ContinueNoWrite
    }
}
