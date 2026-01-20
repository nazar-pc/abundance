//! Part of the interpreter responsible for RISC-V RV64 base instruction set

pub mod b;
pub mod m;
#[cfg(test)]
mod test_utils;
#[cfg(test)]
mod tests;

use crate::{
    ExecutableInstruction, ExecutionError, ProgramCounter, ProgramCounterError, VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::instruction::Instruction;
use ab_riscv_primitives::instruction::rv64::Rv64Instruction;
use ab_riscv_primitives::registers::{Register, Registers};
use core::marker::PhantomData;
use core::ops::ControlFlow;

/// Custom handler for system instructions `ecall` and `ebreak`
pub trait Rv64SystemInstructionHandler<Reg, Memory, PC, CustomError>
where
    Reg: Register<Type = u64>,
    [(); Reg::N]:,
{
    /// Handle an `ecall` instruction.
    ///
    /// NOTE: the program counter here is the current value, meaning it is already incremented past
    /// the instruction itself.
    fn handle_ecall(
        &mut self,
        regs: &mut Registers<Reg>,
        memory: &mut Memory,
        program_counter: &mut PC,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, Rv64Instruction<Reg>, CustomError>>;

    /// Handle an `ebreak` instruction.
    ///
    /// NOTE: the program counter here is the current value, meaning it is already incremented past
    /// the instruction itself.
    #[inline(always)]
    fn handle_ebreak(
        &mut self,
        _regs: &mut Registers<Reg>,
        _memory: &mut Memory,
        _pc: Reg::Type,
        _instruction: Rv64Instruction<Reg>,
    ) {
        // NOP by default
    }
}

/// Basic system instruction handler that does nothing on `ebreak` and returns an error on `ecall`.
#[derive(Debug, Clone, Copy)]
pub struct BasicRv64SystemInstructionHandler<Reg> {
    _phantom: PhantomData<Reg>,
}

impl<Reg> Default for BasicRv64SystemInstructionHandler<Reg> {
    #[inline(always)]
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<Reg, Memory, PC, CustomError> Rv64SystemInstructionHandler<Reg, Memory, PC, CustomError>
    for BasicRv64SystemInstructionHandler<Rv64Instruction<Reg>>
where
    Reg: Register<Type = u64>,
    [(); Reg::N]:,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
{
    #[inline(always)]
    fn handle_ecall(
        &mut self,
        _regs: &mut Registers<Reg>,
        _memory: &mut Memory,
        program_counter: &mut PC,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, Rv64Instruction<Reg>, CustomError>> {
        let instruction = Rv64Instruction::Ecall;
        Err(ExecutionError::UnsupportedInstruction {
            address: program_counter.get_pc() - Reg::Type::from(instruction.size()),
            instruction,
        })
    }
}

/// RV64 interpreter state
#[derive(Debug)]
// 16-byte alignment seems faster than 64 (cache line) for some reason, reconsider in the future
#[repr(align(16))]
pub struct Rv64InterpreterState<Reg, Memory, IF, InstructionHandler, CustomError>
where
    Reg: Register<Type = u64>,
    [(); Reg::N]:,
{
    /// Registers
    pub regs: Registers<Reg>,
    /// Memory
    pub memory: Memory,
    /// Instruction fetcher
    pub instruction_fetcher: IF,
    /// System instruction handler
    pub system_instruction_handler: InstructionHandler,
    #[doc(hidden)]
    pub _phantom: PhantomData<CustomError>,
}

impl<Reg, Memory, IF, InstructionHandler, CustomError>
    Rv64InterpreterState<Reg, Memory, IF, InstructionHandler, CustomError>
where
    Reg: Register<Type = u64>,
    [(); Reg::N]:,
    IF: ProgramCounter<Reg::Type, Memory, CustomError>,
{
    /// Set program counter
    pub fn set_pc(
        &mut self,
        pc: Reg::Type,
    ) -> Result<ControlFlow<()>, ProgramCounterError<Reg::Type, CustomError>> {
        self.instruction_fetcher.set_pc(&mut self.memory, pc)
    }
}

#[instruction_execution]
impl<Reg, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        Rv64InterpreterState<Reg, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for Rv64Instruction<Reg>
where
    Reg: Register<Type = u64>,
    [(); Reg::N]:,
    Memory: VirtualMemory,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
    InstructionHandler: Rv64SystemInstructionHandler<Reg, Memory, PC, CustomError>,
{
    #[inline(always)]
    fn execute(
        self,
        state: &mut Rv64InterpreterState<Reg, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, Self, CustomError>> {
        match self {
            Self::Add { rd, rs1, rs2 } => {
                let value = state.regs.read(rs1).wrapping_add(state.regs.read(rs2));
                state.regs.write(rd, value);
            }
            Self::Sub { rd, rs1, rs2 } => {
                let value = state.regs.read(rs1).wrapping_sub(state.regs.read(rs2));
                state.regs.write(rd, value);
            }
            Self::Sll { rd, rs1, rs2 } => {
                let shamt = state.regs.read(rs2) & 0x3f;
                let value = state.regs.read(rs1) << shamt;
                state.regs.write(rd, value);
            }
            Self::Slt { rd, rs1, rs2 } => {
                let value = state.regs.read(rs1).cast_signed() < state.regs.read(rs2).cast_signed();
                state.regs.write(rd, value as u64);
            }
            Self::Sltu { rd, rs1, rs2 } => {
                let value = state.regs.read(rs1) < state.regs.read(rs2);
                state.regs.write(rd, value as u64);
            }
            Self::Xor { rd, rs1, rs2 } => {
                let value = state.regs.read(rs1) ^ state.regs.read(rs2);
                state.regs.write(rd, value);
            }
            Self::Srl { rd, rs1, rs2 } => {
                let shamt = state.regs.read(rs2) & 0x3f;
                let value = state.regs.read(rs1) >> shamt;
                state.regs.write(rd, value);
            }
            Self::Sra { rd, rs1, rs2 } => {
                let shamt = state.regs.read(rs2) & 0x3f;
                let value = state.regs.read(rs1).cast_signed() >> shamt;
                state.regs.write(rd, value.cast_unsigned());
            }
            Self::Or { rd, rs1, rs2 } => {
                let value = state.regs.read(rs1) | state.regs.read(rs2);
                state.regs.write(rd, value);
            }
            Self::And { rd, rs1, rs2 } => {
                let value = state.regs.read(rs1) & state.regs.read(rs2);
                state.regs.write(rd, value);
            }

            Self::Addw { rd, rs1, rs2 } => {
                let sum = (state.regs.read(rs1) as i32).wrapping_add(state.regs.read(rs2) as i32);
                state.regs.write(rd, (sum as i64).cast_unsigned());
            }
            Self::Subw { rd, rs1, rs2 } => {
                let diff = (state.regs.read(rs1) as i32).wrapping_sub(state.regs.read(rs2) as i32);
                state.regs.write(rd, (diff as i64).cast_unsigned());
            }
            Self::Sllw { rd, rs1, rs2 } => {
                let shamt = state.regs.read(rs2) & 0x1f;
                let shifted = (state.regs.read(rs1) as u32) << shamt;
                state
                    .regs
                    .write(rd, (shifted.cast_signed() as i64).cast_unsigned());
            }
            Self::Srlw { rd, rs1, rs2 } => {
                let shamt = state.regs.read(rs2) & 0x1f;
                let shifted = (state.regs.read(rs1) as u32) >> shamt;
                state
                    .regs
                    .write(rd, (shifted.cast_signed() as i64).cast_unsigned());
            }
            Self::Sraw { rd, rs1, rs2 } => {
                let shamt = state.regs.read(rs2) & 0x1f;
                let shifted = (state.regs.read(rs1) as i32) >> shamt;
                state.regs.write(rd, (shifted as i64).cast_unsigned());
            }

            Self::Addi { rd, rs1, imm } => {
                let value = state
                    .regs
                    .read(rs1)
                    .wrapping_add((imm as i64).cast_unsigned());
                state.regs.write(rd, value);
            }
            Self::Slti { rd, rs1, imm } => {
                let value = state.regs.read(rs1).cast_signed() < (imm as i64);
                state.regs.write(rd, value as u64);
            }
            Self::Sltiu { rd, rs1, imm } => {
                let value = state.regs.read(rs1) < ((imm as i64).cast_unsigned());
                state.regs.write(rd, value as u64);
            }
            Self::Xori { rd, rs1, imm } => {
                let value = state.regs.read(rs1) ^ ((imm as i64).cast_unsigned());
                state.regs.write(rd, value);
            }
            Self::Ori { rd, rs1, imm } => {
                let value = state.regs.read(rs1) | ((imm as i64).cast_unsigned());
                state.regs.write(rd, value);
            }
            Self::Andi { rd, rs1, imm } => {
                let value = state.regs.read(rs1) & ((imm as i64).cast_unsigned());
                state.regs.write(rd, value);
            }
            Self::Slli { rd, rs1, shamt } => {
                let value = state.regs.read(rs1) << shamt;
                state.regs.write(rd, value);
            }
            Self::Srli { rd, rs1, shamt } => {
                let value = state.regs.read(rs1) >> shamt;
                state.regs.write(rd, value);
            }
            Self::Srai { rd, rs1, shamt } => {
                let value = state.regs.read(rs1).cast_signed() >> shamt;
                state.regs.write(rd, value.cast_unsigned());
            }

            Self::Addiw { rd, rs1, imm } => {
                let sum = (state.regs.read(rs1) as i32).wrapping_add(imm);
                state.regs.write(rd, (sum as i64).cast_unsigned());
            }
            Self::Slliw { rd, rs1, shamt } => {
                let shifted = (state.regs.read(rs1) as u32) << shamt;
                state
                    .regs
                    .write(rd, (shifted.cast_signed() as i64).cast_unsigned());
            }
            Self::Srliw { rd, rs1, shamt } => {
                let shifted = (state.regs.read(rs1) as u32) >> shamt;
                state
                    .regs
                    .write(rd, (shifted.cast_signed() as i64).cast_unsigned());
            }
            Self::Sraiw { rd, rs1, shamt } => {
                let shifted = (state.regs.read(rs1) as i32) >> shamt;
                state.regs.write(rd, (shifted as i64).cast_unsigned());
            }

            Self::Lb { rd, rs1, imm } => {
                let addr = state
                    .regs
                    .read(rs1)
                    .wrapping_add((imm as i64).cast_unsigned());
                let value = state.memory.read::<i8>(addr)? as i64;
                state.regs.write(rd, value.cast_unsigned());
            }
            Self::Lh { rd, rs1, imm } => {
                let addr = state
                    .regs
                    .read(rs1)
                    .wrapping_add((imm as i64).cast_unsigned());
                let value = state.memory.read::<i16>(addr)? as i64;
                state.regs.write(rd, value.cast_unsigned());
            }
            Self::Lw { rd, rs1, imm } => {
                let addr = state
                    .regs
                    .read(rs1)
                    .wrapping_add((imm as i64).cast_unsigned());
                let value = state.memory.read::<i32>(addr)? as i64;
                state.regs.write(rd, value.cast_unsigned());
            }
            Self::Ld { rd, rs1, imm } => {
                let addr = state
                    .regs
                    .read(rs1)
                    .wrapping_add((imm as i64).cast_unsigned());
                let value = state.memory.read::<u64>(addr)?;
                state.regs.write(rd, value);
            }
            Self::Lbu { rd, rs1, imm } => {
                let addr = state
                    .regs
                    .read(rs1)
                    .wrapping_add((imm as i64).cast_unsigned());
                let value = state.memory.read::<u8>(addr)?;
                state.regs.write(rd, value as u64);
            }
            Self::Lhu { rd, rs1, imm } => {
                let addr = state
                    .regs
                    .read(rs1)
                    .wrapping_add((imm as i64).cast_unsigned());
                let value = state.memory.read::<u16>(addr)?;
                state.regs.write(rd, value as u64);
            }
            Self::Lwu { rd, rs1, imm } => {
                let addr = state
                    .regs
                    .read(rs1)
                    .wrapping_add((imm as i64).cast_unsigned());
                let value = state.memory.read::<u32>(addr)?;
                state.regs.write(rd, value as u64);
            }

            Self::Jalr { rd, rs1, imm } => {
                let target = (state
                    .regs
                    .read(rs1)
                    .wrapping_add((imm as i64).cast_unsigned()))
                    & !1u64;
                state.regs.write(rd, state.instruction_fetcher.get_pc());
                return state
                    .instruction_fetcher
                    .set_pc(&mut state.memory, target)
                    .map_err(ExecutionError::from);
            }

            Self::Sb { rs2, rs1, imm } => {
                let addr = state
                    .regs
                    .read(rs1)
                    .wrapping_add((imm as i64).cast_unsigned());
                state.memory.write(addr, state.regs.read(rs2) as u8)?;
            }
            Self::Sh { rs2, rs1, imm } => {
                let addr = state
                    .regs
                    .read(rs1)
                    .wrapping_add((imm as i64).cast_unsigned());
                state.memory.write(addr, state.regs.read(rs2) as u16)?;
            }
            Self::Sw { rs2, rs1, imm } => {
                let addr = state
                    .regs
                    .read(rs1)
                    .wrapping_add((imm as i64).cast_unsigned());
                state.memory.write(addr, state.regs.read(rs2) as u32)?;
            }
            Self::Sd { rs2, rs1, imm } => {
                let addr = state
                    .regs
                    .read(rs1)
                    .wrapping_add((imm as i64).cast_unsigned());
                state.memory.write(addr, state.regs.read(rs2))?;
            }

            Self::Beq { rs1, rs2, imm } => {
                if state.regs.read(rs1) == state.regs.read(rs2) {
                    let old_pc = state
                        .instruction_fetcher
                        .get_pc()
                        .wrapping_sub(self.size().into());
                    return state
                        .instruction_fetcher
                        .set_pc(
                            &mut state.memory,
                            old_pc.wrapping_add((imm as i64).cast_unsigned()),
                        )
                        .map_err(ExecutionError::from);
                }
            }
            Self::Bne { rs1, rs2, imm } => {
                if state.regs.read(rs1) != state.regs.read(rs2) {
                    let old_pc = state
                        .instruction_fetcher
                        .get_pc()
                        .wrapping_sub(self.size().into());
                    return state
                        .instruction_fetcher
                        .set_pc(
                            &mut state.memory,
                            old_pc.wrapping_add((imm as i64).cast_unsigned()),
                        )
                        .map_err(ExecutionError::from);
                }
            }
            Self::Blt { rs1, rs2, imm } => {
                if state.regs.read(rs1).cast_signed() < state.regs.read(rs2).cast_signed() {
                    let old_pc = state
                        .instruction_fetcher
                        .get_pc()
                        .wrapping_sub(self.size().into());
                    return state
                        .instruction_fetcher
                        .set_pc(
                            &mut state.memory,
                            old_pc.wrapping_add((imm as i64).cast_unsigned()),
                        )
                        .map_err(ExecutionError::from);
                }
            }
            Self::Bge { rs1, rs2, imm } => {
                if state.regs.read(rs1).cast_signed() >= state.regs.read(rs2).cast_signed() {
                    let old_pc = state
                        .instruction_fetcher
                        .get_pc()
                        .wrapping_sub(self.size().into());
                    return state
                        .instruction_fetcher
                        .set_pc(
                            &mut state.memory,
                            old_pc.wrapping_add((imm as i64).cast_unsigned()),
                        )
                        .map_err(ExecutionError::from);
                }
            }
            Self::Bltu { rs1, rs2, imm } => {
                if state.regs.read(rs1) < state.regs.read(rs2) {
                    let old_pc = state
                        .instruction_fetcher
                        .get_pc()
                        .wrapping_sub(self.size().into());
                    return state
                        .instruction_fetcher
                        .set_pc(
                            &mut state.memory,
                            old_pc.wrapping_add((imm as i64).cast_unsigned()),
                        )
                        .map_err(ExecutionError::from);
                }
            }
            Self::Bgeu { rs1, rs2, imm } => {
                if state.regs.read(rs1) >= state.regs.read(rs2) {
                    let old_pc = state
                        .instruction_fetcher
                        .get_pc()
                        .wrapping_sub(self.size().into());
                    return state
                        .instruction_fetcher
                        .set_pc(
                            &mut state.memory,
                            old_pc.wrapping_add((imm as i64).cast_unsigned()),
                        )
                        .map_err(ExecutionError::from);
                }
            }

            Self::Lui { rd, imm } => {
                state.regs.write(rd, (imm as i64).cast_unsigned());
            }

            Self::Auipc { rd, imm } => {
                let old_pc = state
                    .instruction_fetcher
                    .get_pc()
                    .wrapping_sub(self.size().into());
                state
                    .regs
                    .write(rd, old_pc.wrapping_add((imm as i64).cast_unsigned()));
            }

            Self::Jal { rd, imm } => {
                let pc = state.instruction_fetcher.get_pc();
                let old_pc = pc.wrapping_sub(self.size().into());
                state.regs.write(rd, pc);
                return state
                    .instruction_fetcher
                    .set_pc(
                        &mut state.memory,
                        old_pc.wrapping_add((imm as i64).cast_unsigned()),
                    )
                    .map_err(ExecutionError::from);
            }

            Self::Fence { .. } => {
                // NOP for single-threaded
            }

            Self::Ecall => {
                return state
                    .system_instruction_handler
                    .handle_ecall(
                        &mut state.regs,
                        &mut state.memory,
                        &mut state.instruction_fetcher,
                    )
                    .map_err(|error| {
                        error.map_instruction(|_instruction| {
                            // This mapping helps with instruction type during inheritance
                            Self::Ecall
                        })
                    });
            }
            Self::Ebreak => {
                state.system_instruction_handler.handle_ebreak(
                    &mut state.regs,
                    &mut state.memory,
                    state.instruction_fetcher.get_pc(),
                    Rv64Instruction::<Reg>::Ebreak,
                );
            }

            Self::Unimp => {
                let old_pc = state
                    .instruction_fetcher
                    .get_pc()
                    .wrapping_sub(self.size().into());
                return Err(ExecutionError::UnimpInstruction { address: old_pc });
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}
