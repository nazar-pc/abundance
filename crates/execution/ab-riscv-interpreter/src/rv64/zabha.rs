//! RV64 Zabha extension

#[cfg(test)]
mod tests;

use crate::rv32::a::amo_helpers;
use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    ExecutionResult, FetchInstructionResult, InstructionFetcher, OpaqueThreadedExecutionResult,
    PackedAddress, RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands,
    ThreadedExecutableInstruction, ThreadedExecutionResult, VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;

#[instruction_execution]
const impl<Reg> ExecutableInstructionOperands for Rv64ZabhaInstruction<Reg> where
    Reg: Register<Type = u64>
{
}

#[instruction_execution]
const impl<Reg, Env> ExecutableInstructionCsr<Env> for Rv64ZabhaInstruction<Reg> where
    Reg: Register<Type = u64>
{
}

#[instruction_execution]
const impl<Reg, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
    for Rv64ZabhaInstruction<Reg>
where
    Reg: [const] Register<Type = u64>,
    Regs: [const] RegisterFile<Reg>,
    Memory: [const] VirtualMemory,
{
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn execute(
        self,
        Rs1Rs2OperandValues {
            rs1_value,
            rs2_value,
        }: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
        regs: &mut Regs,
        _env: &mut Env,
        memory: &mut Memory,
        _program_counter: &mut PC,
    ) -> ExecutionResult<Self::Reg> {
        match self {
            Self::AmoswapB {
                rd,
                rs1: _,
                rs2: _,
                aq: _,
                rl: _,
            } => {
                let old = amo_helpers::amo_read::<i8, _, _>(memory, rs1_value)?;
                memory.write(rs1_value, rs2_value as u8)?;
                ExecutionResult::Continue {
                    rd,
                    value: i64::from(old).cast_unsigned(),
                }
            }
            Self::AmoswapH {
                rd,
                rs1: _,
                rs2: _,
                aq: _,
                rl: _,
            } => {
                // The 2-byte access must not cross a misaligned atomicity granule (4096 bytes)
                // boundary
                if rs1_value / 4096 != (rs1_value + 1) / 4096 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::MisalignedAtomic {
                        address: PackedAddress::new(rs1_value),
                    });
                }
                let old = amo_helpers::amo_read::<i16, _, _>(memory, rs1_value)?;
                memory.write(rs1_value, rs2_value as u16)?;
                ExecutionResult::Continue {
                    rd,
                    value: i64::from(old).cast_unsigned(),
                }
            }
            Self::AmoaddB {
                rd,
                rs1: _,
                rs2: _,
                aq: _,
                rl: _,
            } => {
                let old = amo_helpers::amo_read::<i8, _, _>(memory, rs1_value)?;
                let new = old.cast_unsigned().wrapping_add(rs2_value as u8);
                memory.write(rs1_value, new)?;
                ExecutionResult::Continue {
                    rd,
                    value: i64::from(old).cast_unsigned(),
                }
            }
            Self::AmoaddH {
                rd,
                rs1: _,
                rs2: _,
                aq: _,
                rl: _,
            } => {
                // The 2-byte access must not cross a misaligned atomicity granule (4096 bytes)
                // boundary
                if rs1_value / 4096 != (rs1_value + 1) / 4096 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::MisalignedAtomic {
                        address: PackedAddress::new(rs1_value),
                    });
                }
                let old = amo_helpers::amo_read::<i16, _, _>(memory, rs1_value)?;
                let new = old.cast_unsigned().wrapping_add(rs2_value as u16);
                memory.write(rs1_value, new)?;
                ExecutionResult::Continue {
                    rd,
                    value: i64::from(old).cast_unsigned(),
                }
            }
            Self::AmoxorB {
                rd,
                rs1: _,
                rs2: _,
                aq: _,
                rl: _,
            } => {
                let old = amo_helpers::amo_read::<i8, _, _>(memory, rs1_value)?;
                let new = old.cast_unsigned() ^ (rs2_value as u8);
                memory.write(rs1_value, new)?;
                ExecutionResult::Continue {
                    rd,
                    value: i64::from(old).cast_unsigned(),
                }
            }
            Self::AmoxorH {
                rd,
                rs1: _,
                rs2: _,
                aq: _,
                rl: _,
            } => {
                // The 2-byte access must not cross a misaligned atomicity granule (4096 bytes)
                // boundary
                if rs1_value / 4096 != (rs1_value + 1) / 4096 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::MisalignedAtomic {
                        address: PackedAddress::new(rs1_value),
                    });
                }
                let old = amo_helpers::amo_read::<i16, _, _>(memory, rs1_value)?;
                let new = old.cast_unsigned() ^ (rs2_value as u16);
                memory.write(rs1_value, new)?;
                ExecutionResult::Continue {
                    rd,
                    value: i64::from(old).cast_unsigned(),
                }
            }
            Self::AmoandB {
                rd,
                rs1: _,
                rs2: _,
                aq: _,
                rl: _,
            } => {
                let old = amo_helpers::amo_read::<i8, _, _>(memory, rs1_value)?;
                let new = old.cast_unsigned() & (rs2_value as u8);
                memory.write(rs1_value, new)?;
                ExecutionResult::Continue {
                    rd,
                    value: i64::from(old).cast_unsigned(),
                }
            }
            Self::AmoandH {
                rd,
                rs1: _,
                rs2: _,
                aq: _,
                rl: _,
            } => {
                // The 2-byte access must not cross a misaligned atomicity granule (4096 bytes)
                // boundary
                if rs1_value / 4096 != (rs1_value + 1) / 4096 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::MisalignedAtomic {
                        address: PackedAddress::new(rs1_value),
                    });
                }
                let old = amo_helpers::amo_read::<i16, _, _>(memory, rs1_value)?;
                let new = old.cast_unsigned() & (rs2_value as u16);
                memory.write(rs1_value, new)?;
                ExecutionResult::Continue {
                    rd,
                    value: i64::from(old).cast_unsigned(),
                }
            }
            Self::AmoorB {
                rd,
                rs1: _,
                rs2: _,
                aq: _,
                rl: _,
            } => {
                let old = amo_helpers::amo_read::<i8, _, _>(memory, rs1_value)?;
                let new = old.cast_unsigned() | (rs2_value as u8);
                memory.write(rs1_value, new)?;
                ExecutionResult::Continue {
                    rd,
                    value: i64::from(old).cast_unsigned(),
                }
            }
            Self::AmoorH {
                rd,
                rs1: _,
                rs2: _,
                aq: _,
                rl: _,
            } => {
                // The 2-byte access must not cross a misaligned atomicity granule (4096 bytes)
                // boundary
                if rs1_value / 4096 != (rs1_value + 1) / 4096 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::MisalignedAtomic {
                        address: PackedAddress::new(rs1_value),
                    });
                }
                let old = amo_helpers::amo_read::<i16, _, _>(memory, rs1_value)?;
                let new = old.cast_unsigned() | (rs2_value as u16);
                memory.write(rs1_value, new)?;
                ExecutionResult::Continue {
                    rd,
                    value: i64::from(old).cast_unsigned(),
                }
            }
            Self::AmominB {
                rd,
                rs1: _,
                rs2: _,
                aq: _,
                rl: _,
            } => {
                let old = amo_helpers::amo_read::<i8, _, _>(memory, rs1_value)?;
                let new = if old < (rs2_value as u8).cast_signed() {
                    old.cast_unsigned()
                } else {
                    rs2_value as u8
                };
                memory.write(rs1_value, new)?;
                ExecutionResult::Continue {
                    rd,
                    value: i64::from(old).cast_unsigned(),
                }
            }
            Self::AmominH {
                rd,
                rs1: _,
                rs2: _,
                aq: _,
                rl: _,
            } => {
                // The 2-byte access must not cross a misaligned atomicity granule (4096 bytes)
                // boundary
                if rs1_value / 4096 != (rs1_value + 1) / 4096 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::MisalignedAtomic {
                        address: PackedAddress::new(rs1_value),
                    });
                }
                let old = amo_helpers::amo_read::<i16, _, _>(memory, rs1_value)?;
                let new = if old < (rs2_value as u16).cast_signed() {
                    old.cast_unsigned()
                } else {
                    rs2_value as u16
                };
                memory.write(rs1_value, new)?;
                ExecutionResult::Continue {
                    rd,
                    value: i64::from(old).cast_unsigned(),
                }
            }
            Self::AmomaxB {
                rd,
                rs1: _,
                rs2: _,
                aq: _,
                rl: _,
            } => {
                let old = amo_helpers::amo_read::<i8, _, _>(memory, rs1_value)?;
                let new = if old > (rs2_value as u8).cast_signed() {
                    old.cast_unsigned()
                } else {
                    rs2_value as u8
                };
                memory.write(rs1_value, new)?;
                ExecutionResult::Continue {
                    rd,
                    value: i64::from(old).cast_unsigned(),
                }
            }
            Self::AmomaxH {
                rd,
                rs1: _,
                rs2: _,
                aq: _,
                rl: _,
            } => {
                // The 2-byte access must not cross a misaligned atomicity granule (4096 bytes)
                // boundary
                if rs1_value / 4096 != (rs1_value + 1) / 4096 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::MisalignedAtomic {
                        address: PackedAddress::new(rs1_value),
                    });
                }
                let old = amo_helpers::amo_read::<i16, _, _>(memory, rs1_value)?;
                let new = if old > (rs2_value as u16).cast_signed() {
                    old.cast_unsigned()
                } else {
                    rs2_value as u16
                };
                memory.write(rs1_value, new)?;
                ExecutionResult::Continue {
                    rd,
                    value: i64::from(old).cast_unsigned(),
                }
            }
            Self::AmominuB {
                rd,
                rs1: _,
                rs2: _,
                aq: _,
                rl: _,
            } => {
                let old = amo_helpers::amo_read::<i8, _, _>(memory, rs1_value)?;
                let new = if old.cast_unsigned() < (rs2_value as u8) {
                    old.cast_unsigned()
                } else {
                    rs2_value as u8
                };
                memory.write(rs1_value, new)?;
                ExecutionResult::Continue {
                    rd,
                    value: i64::from(old).cast_unsigned(),
                }
            }
            Self::AmominuH {
                rd,
                rs1: _,
                rs2: _,
                aq: _,
                rl: _,
            } => {
                // The 2-byte access must not cross a misaligned atomicity granule (4096 bytes)
                // boundary
                if rs1_value / 4096 != (rs1_value + 1) / 4096 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::MisalignedAtomic {
                        address: PackedAddress::new(rs1_value),
                    });
                }
                let old = amo_helpers::amo_read::<i16, _, _>(memory, rs1_value)?;
                let new = if old.cast_unsigned() < (rs2_value as u16) {
                    old.cast_unsigned()
                } else {
                    rs2_value as u16
                };
                memory.write(rs1_value, new)?;
                ExecutionResult::Continue {
                    rd,
                    value: i64::from(old).cast_unsigned(),
                }
            }
            Self::AmomaxuB {
                rd,
                rs1: _,
                rs2: _,
                aq: _,
                rl: _,
            } => {
                let old = amo_helpers::amo_read::<i8, _, _>(memory, rs1_value)?;
                let new = if old.cast_unsigned() > (rs2_value as u8) {
                    old.cast_unsigned()
                } else {
                    rs2_value as u8
                };
                memory.write(rs1_value, new)?;
                ExecutionResult::Continue {
                    rd,
                    value: i64::from(old).cast_unsigned(),
                }
            }
            Self::AmomaxuH {
                rd,
                rs1: _,
                rs2: _,
                aq: _,
                rl: _,
            } => {
                // The 2-byte access must not cross a misaligned atomicity granule (4096 bytes)
                // boundary
                if rs1_value / 4096 != (rs1_value + 1) / 4096 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::MisalignedAtomic {
                        address: PackedAddress::new(rs1_value),
                    });
                }
                let old = amo_helpers::amo_read::<i16, _, _>(memory, rs1_value)?;
                let new = if old.cast_unsigned() > (rs2_value as u16) {
                    old.cast_unsigned()
                } else {
                    rs2_value as u16
                };
                memory.write(rs1_value, new)?;
                ExecutionResult::Continue {
                    rd,
                    value: i64::from(old).cast_unsigned(),
                }
            }
            Self::AmocasB {
                rd,
                rs1: _,
                rs2: _,
                aq: _,
                rl: _,
            } => {
                let compare = regs.read(rd) as u8;
                let old = amo_helpers::amo_read::<i8, _, _>(memory, rs1_value)?;
                if old.cast_unsigned() == compare {
                    memory.write(rs1_value, rs2_value as u8)?;
                }
                ExecutionResult::Continue {
                    rd,
                    value: i64::from(old).cast_unsigned(),
                }
            }
            Self::AmocasH {
                rd,
                rs1: _,
                rs2: _,
                aq: _,
                rl: _,
            } => {
                let compare = regs.read(rd) as u16;
                // The 2-byte access must not cross a misaligned atomicity granule (4096 bytes)
                // boundary
                if rs1_value / 4096 != (rs1_value + 1) / 4096 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::MisalignedAtomic {
                        address: PackedAddress::new(rs1_value),
                    });
                }
                let old = amo_helpers::amo_read::<i16, _, _>(memory, rs1_value)?;
                if old.cast_unsigned() == compare {
                    memory.write(rs1_value, rs2_value as u16)?;
                }
                ExecutionResult::Continue {
                    rd,
                    value: i64::from(old).cast_unsigned(),
                }
            }
        }
    }
}
