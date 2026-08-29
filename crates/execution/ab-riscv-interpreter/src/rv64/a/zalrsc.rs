//! RV64 Zalrsc extension

#[cfg(test)]
mod tests;

use crate::rv32::a::{ReservationSet, amo_helpers};
use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    ExecutionResult, FetchInstructionResult, InstructionFetcher, OpaqueThreadedExecutionResult,
    PackedAddress, RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands,
    ThreadedExecutableInstruction, ThreadedExecutionResult, VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;

#[instruction_execution]
const impl<Reg> ExecutableInstructionOperands for Rv64ZalrscInstruction<Reg> where
    Reg: Register<Type = u64>
{
}

#[instruction_execution]
const impl<Reg, Env> ExecutableInstructionCsr<Env> for Rv64ZalrscInstruction<Reg> where
    Reg: Register<Type = u64>
{
}

#[instruction_execution]
const impl<Reg, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
    for Rv64ZalrscInstruction<Reg>
where
    Reg: [const] Register<Type = u64>,
    Regs: [const] RegisterFile<Reg>,
    Memory: [const] VirtualMemory,
    Env: [const] ReservationSet<Reg>,
{
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn execute(
        self,
        Rs1Rs2OperandValues {
            rs1_value,
            rs2_value,
        }: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
        _regs: &mut Regs,
        env: &mut Env,
        memory: &mut Memory,
        _program_counter: &mut PC,
    ) -> ExecutionResult<Self::Reg> {
        match self {
            Self::Lr {
                rd,
                rs1: _,
                aq: _,
                rl: _,
            } => {
                if !rs1_value.is_multiple_of(4) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::MisalignedRead {
                        address: PackedAddress::new(rs1_value),
                    });
                }
                let value = memory.read::<i32>(rs1_value)?;
                env.set_reservation(rs1_value);
                ExecutionResult::Continue {
                    rd,
                    value: i64::from(value).cast_unsigned(),
                }
            }
            Self::Sc {
                rd,
                rs1: _,
                rs2: _,
                aq: _,
                rl: _,
            } => {
                if !rs1_value.is_multiple_of(4) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::MisalignedWrite {
                        address: PackedAddress::new(rs1_value),
                    });
                }
                let success = env.reservation() == Some(rs1_value);
                env.clear_reservation();

                if success {
                    memory.write(rs1_value, rs2_value as u32)?;
                    ExecutionResult::Continue { rd, value: 0 }
                } else {
                    // A failing `sc` (lost/no reservation) still checks its target address for a
                    // Store/AMO access fault, exactly as a successful one would - only the actual
                    // write is skipped. `amo_read` (not a plain `memory.read`) both probes the
                    // same address range a write would use, side-effect-free, and classifies any
                    // fault as Store/AMO rather than Load, matching a real `sc`'s own fault cause.
                    amo_helpers::amo_read::<u32, _, _>(memory, rs1_value)?;
                    ExecutionResult::Continue { rd, value: 1 }
                }
            }
            Self::LrD {
                rd,
                rs1: _,
                aq: _,
                rl: _,
            } => {
                if !rs1_value.is_multiple_of(8) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::MisalignedRead {
                        address: PackedAddress::new(rs1_value),
                    });
                }
                let value = memory.read::<u64>(rs1_value)?;
                env.set_reservation(rs1_value);
                ExecutionResult::Continue { rd, value }
            }
            Self::ScD {
                rd,
                rs1: _,
                rs2: _,
                aq: _,
                rl: _,
            } => {
                if !rs1_value.is_multiple_of(8) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::MisalignedWrite {
                        address: PackedAddress::new(rs1_value),
                    });
                }
                let success = env.reservation() == Some(rs1_value);
                env.clear_reservation();

                if success {
                    memory.write(rs1_value, rs2_value)?;
                    ExecutionResult::Continue { rd, value: 0 }
                } else {
                    // See the comment in the `Sc` (32-bit) arm above.
                    amo_helpers::amo_read::<u64, _, _>(memory, rs1_value)?;
                    ExecutionResult::Continue { rd, value: 1 }
                }
            }
        }
    }
}
