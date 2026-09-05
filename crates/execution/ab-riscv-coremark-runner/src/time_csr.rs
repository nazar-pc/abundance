#![expect(unreachable_pub, reason = "Macro requirements and generated code")]

use ab_riscv_interpreter::prelude::*;
use ab_riscv_macros::{instruction, instruction_execution};
use ab_riscv_primitives::prelude::*;
use core::fmt;
use core::ops::ControlFlow;
use std::time::Instant;

/// State for the counter CSR (time-only)
#[derive(Debug, Clone)]
pub struct TimeCsrState {
    start: Instant,
}

impl AsMut<TimeCsrState> for TimeCsrState {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut TimeCsrState {
        self
    }
}

impl AsRef<TimeCsrState> for TimeCsrState {
    #[inline(always)]
    fn as_ref(&self) -> &TimeCsrState {
        self
    }
}

impl Default for TimeCsrState {
    fn default() -> Self {
        Self {
            start: Instant::now(),
        }
    }
}

impl Csrs<Reg<u64>> for TimeCsrState {
    fn privilege_level(&self) -> PrivilegeLevel {
        PrivilegeLevel::Machine
    }

    fn read_csr(&self, _csr_index: u16) -> Result<u64, CsrError> {
        Ok(0)
    }

    fn write_csr(&mut self, _csr_index: u16, _value: u64) -> Result<(), CsrError> {
        Ok(())
    }
}

impl<Regs, Memory, PC> SystemInstructionHandler<Reg<u64>, Regs, Memory, PC> for TimeCsrState
where
    PC: ProgramCounter<u64, Memory>,
{
    /// Coremark makes no system calls, so `ecall` is rejected as an illegal instruction
    fn handle_ecall(
        &mut self,
        _regs: &mut Regs,
        _memory: &mut Memory,
        program_counter: &mut PC,
    ) -> Result<ControlFlow<()>, ExecutionError<u64>> {
        Err(ExecutionError::IllegalInstruction {
            address: PackedAddress::new(program_counter.old_pc(size_of::<u32>() as u8)),
        })
    }
}

impl WrsHandler for TimeCsrState {}

impl TimeCsrState {
    pub(crate) fn elapsed_ns(&self) -> u64 {
        self.start.elapsed().as_nanos() as u64
    }
}

/// Minimal placeholder for the counter (time-only) CSR.
///
/// No decoded instruction variants are needed, all work happens in `prepare_csr_read`.
#[instruction(
    inherit = [ZicsrInstruction],
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TimeCsrInstruction<Reg> {}

#[instruction]
const impl<Reg> Instruction for TimeCsrInstruction<Reg> {
    const ALIGNMENT: u8 = align_of::<u32>() as u8;

    type Reg = Reg;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        None
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

#[instruction]
impl<Reg> fmt::Display for TimeCsrInstruction<Reg>
where
    Reg: fmt::Display + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[instruction_execution]
impl<Reg> ExecutableInstructionOperands for TimeCsrInstruction<Reg> where Reg: Register {}

#[instruction_execution]
impl<Reg, Env> ExecutableInstructionCsr<Env> for TimeCsrInstruction<Reg>
where
    Reg: Register<Type = u64>,
    Env: AsMut<TimeCsrState> + AsRef<TimeCsrState>,
{
    fn prepare_csr_read(
        env: &Env,
        csr_index: u16,
        _will_write: bool,
        _raw_value: Reg::Type,
        output_value: &mut Reg::Type,
    ) -> Result<bool, CsrError> {
        const CSR_TIME: u16 = 0xC01;

        if csr_index == CSR_TIME {
            // Return elapsed nanoseconds
            *output_value = env.as_ref().elapsed_ns();
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn prepare_csr_write(
        _env: &mut Env,
        csr_index: u16,
        _write_value: Reg::Type,
        _output_value: &mut Reg::Type,
    ) -> Result<bool, CsrError> {
        const CSR_TIME: u16 = 0xC01;

        if csr_index == CSR_TIME {
            Err(CsrError::ReadOnly { csr_index })
        } else {
            Ok(false)
        }
    }
}

#[instruction_execution]
impl<Reg, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
    for TimeCsrInstruction<Reg>
where
    Reg: Register<Type = u64>,
    Env: AsMut<TimeCsrState> + AsRef<TimeCsrState>,
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
