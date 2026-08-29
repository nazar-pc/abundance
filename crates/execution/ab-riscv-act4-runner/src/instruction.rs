use ab_riscv_interpreter::prelude::*;
use ab_riscv_macros::{instruction, instruction_execution};
use ab_riscv_primitives::prelude::*;
use std::fmt;

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
impl<Reg, Env> ExecutableInstructionCsr<Env> for MachineModePlaceholder<Reg>
where
    Reg: Register,
    Env: Csrs<Reg>,
{
    fn prepare_csr_read(
        _env: &Env,
        csr_index: u16,
        _will_write: bool,
        raw_value: Reg::Type,
        output_value: &mut Reg::Type,
    ) -> Result<bool, CsrError> {
        if matches!(
            MCsr::from_index(csr_index),
            Some(
                MCsr::Mvendorid
                    | MCsr::Marchid
                    | MCsr::Mimpid
                    | MCsr::Mhartid
                    | MCsr::Mstatus
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
                    | MCsr::Mconfigptr
                    | MCsr::Mseccfg
                    | MCsr::Mseccfgh
            )
        ) {
            *output_value = raw_value;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn prepare_csr_write(
        env: &mut Env,
        csr_index: u16,
        write_value: Reg::Type,
        output_value: &mut Reg::Type,
    ) -> Result<bool, CsrError> {
        match MCsr::from_index(csr_index) {
            Some(MCsr::Mscratch | MCsr::Mcause | MCsr::Mtval) => {
                *output_value = write_value;
                Ok(true)
            }
            Some(MCsr::Mcountinhibit) => {
                *output_value = crate::interpreter::mask_mcountinhibit::<Reg>(write_value);
                Ok(true)
            }
            Some(MCsr::Mtvec) => {
                // MTVEC_MODES is [0] (Direct only) for this core, and MTVEC_BASE_ALIGNMENT_DIRECT
                // is 4, so the low 2 (MODE) bits must always read back 0. Per
                // MTVEC_ILLEGAL_WRITE_BEHAVIOR ("retain"), a write with any other MODE value
                // doesn't just get those bits cleared - the whole CSR keeps its previous value.
                if write_value & Reg::Type::from(0b11u32) == Reg::Type::default() {
                    *output_value = write_value;
                } else {
                    *output_value = env.read_csr(csr_index)?;
                }
                Ok(true)
            }
            Some(MCsr::Mepc) => {
                *output_value = write_value & !Reg::Type::from(1u32);
                Ok(true)
            }
            Some(MCsr::Misa) => {
                // MISA_CSR_IMPLEMENTED is false for this core: misa isn't writable, so every write
                // is WARL-ignored - but it still reads back MXL and each implemented single-letter
                // extension's bit accurately, see `misa_value()`.
                *output_value = crate::interpreter::misa_value::<Reg>();
                Ok(true)
            }
            Some(MCsr::Mstatus) => {
                *output_value = crate::interpreter::mask_mstatus::<Reg>(write_value);
                Ok(true)
            }
            Some(MCsr::Mstatush) => {
                // mstatush only carries MBE/SBE (endianness); this core is fixed little-endian
                // (M_MODE_ENDIANNESS) with no S-mode, so it's hardwired to 0 like misa above
                *output_value = Reg::Type::from(0u32);
                Ok(true)
            }
            Some(MCsr::Mseccfg | MCsr::Mseccfgh) => {
                // Smepmp (MML/MMWP/RLB) isn't implemented, and SSEED/USEED have no S/U mode to
                // grant seed-CSR access to, so every field of both halves is hardwired to 0
                *output_value = Reg::Type::from(0u32);
                Ok(true)
            }
            Some(MCsr::Mie) => {
                *output_value = crate::interpreter::mask_mie::<Reg>(write_value);
                Ok(true)
            }
            Some(MCsr::Mip) => {
                *output_value = crate::interpreter::mask_mip::<Reg>(write_value);
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}

#[instruction_execution]
impl<Reg, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
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
        env: &mut Env,
        _memory: &mut Memory,
        _program_counter: &mut PC,
    ) -> ExecutionResult<Self::Reg> {
        ExecutionResult::ContinueNoWrite
    }
}
