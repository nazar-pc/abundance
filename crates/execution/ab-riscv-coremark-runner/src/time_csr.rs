#![expect(unreachable_pub, reason = "Macro requirements and generated code")]

use crate::interpreter::{EagerInstructionFetcher, GuestMemory};
use ab_riscv_interpreter::basic::{BasicRegisters, IgnoreEcallSystemInstructionHandler};
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

    fn process_csr_read(&self, csr_index: u16, raw_value: u64) -> Result<u64, CsrError> {
        let mut out = 0;
        if !<TimeCsrInstruction<Reg<u64>> as ExecutableInstruction<
            BasicRegisters<<TimeCsrInstruction<Reg<u64>> as Instruction>::Reg>,
            Self,
            GuestMemory<0, 0>,
            EagerInstructionFetcher,
            IgnoreEcallSystemInstructionHandler,
        >>::prepare_csr_read(self, csr_index, raw_value, &mut out)?
        {
            return Err(CsrError::IllegalRead { csr_index });
        }

        Ok(out)
    }

    fn process_csr_write(&mut self, csr_index: u16, write_value: u64) -> Result<u64, CsrError> {
        let mut out = 0;
        if !<TimeCsrInstruction<Reg<u64>> as ExecutableInstruction<
            BasicRegisters<<TimeCsrInstruction<Reg<u64>> as Instruction>::Reg>,
            Self,
            GuestMemory<0, 0>,
            EagerInstructionFetcher,
            IgnoreEcallSystemInstructionHandler,
        >>::prepare_csr_write(self, csr_index, write_value, &mut out)?
        {
            return Err(CsrError::IllegalWrite { csr_index });
        }

        Ok(out)
    }
}

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
pub enum TimeCsrInstruction<Reg> {}

#[instruction]
impl<Reg> const Instruction for TimeCsrInstruction<Reg>
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
impl<Reg> fmt::Display for TimeCsrInstruction<Reg>
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
    for TimeCsrInstruction<Reg>
where
    Reg: Register<Type = u64>,
    ExtState: AsMut<TimeCsrState> + AsRef<TimeCsrState>,
    CustomError: fmt::Debug,
{
    fn prepare_csr_read(
        ext_state: &ExtState,
        csr_index: u16,
        _raw_value: Reg::Type,
        output_value: &mut Reg::Type,
    ) -> Result<bool, CsrError<CustomError>> {
        const CSR_TIME: u16 = 0xC01;

        if csr_index == CSR_TIME {
            // Return elapsed nanoseconds
            *output_value = ext_state.as_ref().elapsed_ns();
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn prepare_csr_write(
        _ext_state: &mut ExtState,
        csr_index: u16,
        _write_value: Reg::Type,
        _output_value: &mut Reg::Type,
    ) -> Result<bool, CsrError<CustomError>> {
        const CSR_TIME: u16 = 0xC01;

        if csr_index == CSR_TIME {
            Err(CsrError::ReadOnly { csr_index })
        } else {
            Ok(false)
        }
    }

    fn execute(
        self,
        Rs1Rs2OperandValues {
            rs1_value,
            rs2_value: _,
        }: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
        regs: &mut Regs,
        ext_state: &mut ExtState,
        _memory: &mut Memory,
        _program_counter: &mut PC,
        _system_instruction_handler: &mut InstructionHandler,
    ) -> Result<
        ControlFlow<(), (Self::Reg, <Self::Reg as Register>::Type)>,
        ExecutionError<Reg::Type, CustomError>,
    > {
        Ok(ControlFlow::Continue(Default::default()))
    }
}
