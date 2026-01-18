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
        execute_rv64(state, self)
    }
}

/// Execute instructions from a base RV64I/RV64E instruction set
#[inline(always)]
pub fn execute_rv64<Reg, Memory, PC, InstructionHandler, CustomError>(
    state: &mut Rv64InterpreterState<Reg, Memory, PC, InstructionHandler, CustomError>,
    instruction: Rv64Instruction<Reg>,
) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, Rv64Instruction<Reg>, CustomError>>
where
    Reg: Register<Type = u64>,
    [(); Reg::N]:,
    Memory: VirtualMemory,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
    InstructionHandler: Rv64SystemInstructionHandler<Reg, Memory, PC, CustomError>,
{
    match instruction {
        Rv64Instruction::Add { rd, rs1, rs2 } => {
            let value = state.regs.read(rs1).wrapping_add(state.regs.read(rs2));
            state.regs.write(rd, value);
        }
        Rv64Instruction::Sub { rd, rs1, rs2 } => {
            let value = state.regs.read(rs1).wrapping_sub(state.regs.read(rs2));
            state.regs.write(rd, value);
        }
        Rv64Instruction::Sll { rd, rs1, rs2 } => {
            let shamt = state.regs.read(rs2) & 0x3f;
            let value = state.regs.read(rs1) << shamt;
            state.regs.write(rd, value);
        }
        Rv64Instruction::Slt { rd, rs1, rs2 } => {
            let value = state.regs.read(rs1).cast_signed() < state.regs.read(rs2).cast_signed();
            state.regs.write(rd, value as u64);
        }
        Rv64Instruction::Sltu { rd, rs1, rs2 } => {
            let value = state.regs.read(rs1) < state.regs.read(rs2);
            state.regs.write(rd, value as u64);
        }
        Rv64Instruction::Xor { rd, rs1, rs2 } => {
            let value = state.regs.read(rs1) ^ state.regs.read(rs2);
            state.regs.write(rd, value);
        }
        Rv64Instruction::Srl { rd, rs1, rs2 } => {
            let shamt = state.regs.read(rs2) & 0x3f;
            let value = state.regs.read(rs1) >> shamt;
            state.regs.write(rd, value);
        }
        Rv64Instruction::Sra { rd, rs1, rs2 } => {
            let shamt = state.regs.read(rs2) & 0x3f;
            let value = state.regs.read(rs1).cast_signed() >> shamt;
            state.regs.write(rd, value.cast_unsigned());
        }
        Rv64Instruction::Or { rd, rs1, rs2 } => {
            let value = state.regs.read(rs1) | state.regs.read(rs2);
            state.regs.write(rd, value);
        }
        Rv64Instruction::And { rd, rs1, rs2 } => {
            let value = state.regs.read(rs1) & state.regs.read(rs2);
            state.regs.write(rd, value);
        }

        Rv64Instruction::Addw { rd, rs1, rs2 } => {
            let sum = (state.regs.read(rs1) as i32).wrapping_add(state.regs.read(rs2) as i32);
            state.regs.write(rd, (sum as i64).cast_unsigned());
        }
        Rv64Instruction::Subw { rd, rs1, rs2 } => {
            let diff = (state.regs.read(rs1) as i32).wrapping_sub(state.regs.read(rs2) as i32);
            state.regs.write(rd, (diff as i64).cast_unsigned());
        }
        Rv64Instruction::Sllw { rd, rs1, rs2 } => {
            let shamt = state.regs.read(rs2) & 0x1f;
            let shifted = (state.regs.read(rs1) as u32) << shamt;
            state
                .regs
                .write(rd, (shifted.cast_signed() as i64).cast_unsigned());
        }
        Rv64Instruction::Srlw { rd, rs1, rs2 } => {
            let shamt = state.regs.read(rs2) & 0x1f;
            let shifted = (state.regs.read(rs1) as u32) >> shamt;
            state
                .regs
                .write(rd, (shifted.cast_signed() as i64).cast_unsigned());
        }
        Rv64Instruction::Sraw { rd, rs1, rs2 } => {
            let shamt = state.regs.read(rs2) & 0x1f;
            let shifted = (state.regs.read(rs1) as i32) >> shamt;
            state.regs.write(rd, (shifted as i64).cast_unsigned());
        }

        Rv64Instruction::Addi { rd, rs1, imm } => {
            let value = state
                .regs
                .read(rs1)
                .wrapping_add((imm as i64).cast_unsigned());
            state.regs.write(rd, value);
        }
        Rv64Instruction::Slti { rd, rs1, imm } => {
            let value = state.regs.read(rs1).cast_signed() < (imm as i64);
            state.regs.write(rd, value as u64);
        }
        Rv64Instruction::Sltiu { rd, rs1, imm } => {
            let value = state.regs.read(rs1) < ((imm as i64).cast_unsigned());
            state.regs.write(rd, value as u64);
        }
        Rv64Instruction::Xori { rd, rs1, imm } => {
            let value = state.regs.read(rs1) ^ ((imm as i64).cast_unsigned());
            state.regs.write(rd, value);
        }
        Rv64Instruction::Ori { rd, rs1, imm } => {
            let value = state.regs.read(rs1) | ((imm as i64).cast_unsigned());
            state.regs.write(rd, value);
        }
        Rv64Instruction::Andi { rd, rs1, imm } => {
            let value = state.regs.read(rs1) & ((imm as i64).cast_unsigned());
            state.regs.write(rd, value);
        }
        Rv64Instruction::Slli { rd, rs1, shamt } => {
            let value = state.regs.read(rs1) << shamt;
            state.regs.write(rd, value);
        }
        Rv64Instruction::Srli { rd, rs1, shamt } => {
            let value = state.regs.read(rs1) >> shamt;
            state.regs.write(rd, value);
        }
        Rv64Instruction::Srai { rd, rs1, shamt } => {
            let value = state.regs.read(rs1).cast_signed() >> shamt;
            state.regs.write(rd, value.cast_unsigned());
        }

        Rv64Instruction::Addiw { rd, rs1, imm } => {
            let sum = (state.regs.read(rs1) as i32).wrapping_add(imm);
            state.regs.write(rd, (sum as i64).cast_unsigned());
        }
        Rv64Instruction::Slliw { rd, rs1, shamt } => {
            let shifted = (state.regs.read(rs1) as u32) << shamt;
            state
                .regs
                .write(rd, (shifted.cast_signed() as i64).cast_unsigned());
        }
        Rv64Instruction::Srliw { rd, rs1, shamt } => {
            let shifted = (state.regs.read(rs1) as u32) >> shamt;
            state
                .regs
                .write(rd, (shifted.cast_signed() as i64).cast_unsigned());
        }
        Rv64Instruction::Sraiw { rd, rs1, shamt } => {
            let shifted = (state.regs.read(rs1) as i32) >> shamt;
            state.regs.write(rd, (shifted as i64).cast_unsigned());
        }

        Rv64Instruction::Lb { rd, rs1, imm } => {
            let addr = state
                .regs
                .read(rs1)
                .wrapping_add((imm as i64).cast_unsigned());
            let value = state.memory.read::<i8>(addr)? as i64;
            state.regs.write(rd, value.cast_unsigned());
        }
        Rv64Instruction::Lh { rd, rs1, imm } => {
            let addr = state
                .regs
                .read(rs1)
                .wrapping_add((imm as i64).cast_unsigned());
            let value = state.memory.read::<i16>(addr)? as i64;
            state.regs.write(rd, value.cast_unsigned());
        }
        Rv64Instruction::Lw { rd, rs1, imm } => {
            let addr = state
                .regs
                .read(rs1)
                .wrapping_add((imm as i64).cast_unsigned());
            let value = state.memory.read::<i32>(addr)? as i64;
            state.regs.write(rd, value.cast_unsigned());
        }
        Rv64Instruction::Ld { rd, rs1, imm } => {
            let addr = state
                .regs
                .read(rs1)
                .wrapping_add((imm as i64).cast_unsigned());
            let value = state.memory.read::<u64>(addr)?;
            state.regs.write(rd, value);
        }
        Rv64Instruction::Lbu { rd, rs1, imm } => {
            let addr = state
                .regs
                .read(rs1)
                .wrapping_add((imm as i64).cast_unsigned());
            let value = state.memory.read::<u8>(addr)?;
            state.regs.write(rd, value as u64);
        }
        Rv64Instruction::Lhu { rd, rs1, imm } => {
            let addr = state
                .regs
                .read(rs1)
                .wrapping_add((imm as i64).cast_unsigned());
            let value = state.memory.read::<u16>(addr)?;
            state.regs.write(rd, value as u64);
        }
        Rv64Instruction::Lwu { rd, rs1, imm } => {
            let addr = state
                .regs
                .read(rs1)
                .wrapping_add((imm as i64).cast_unsigned());
            let value = state.memory.read::<u32>(addr)?;
            state.regs.write(rd, value as u64);
        }

        Rv64Instruction::Jalr { rd, rs1, imm } => {
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

        Rv64Instruction::Sb { rs2, rs1, imm } => {
            let addr = state
                .regs
                .read(rs1)
                .wrapping_add((imm as i64).cast_unsigned());
            state.memory.write(addr, state.regs.read(rs2) as u8)?;
        }
        Rv64Instruction::Sh { rs2, rs1, imm } => {
            let addr = state
                .regs
                .read(rs1)
                .wrapping_add((imm as i64).cast_unsigned());
            state.memory.write(addr, state.regs.read(rs2) as u16)?;
        }
        Rv64Instruction::Sw { rs2, rs1, imm } => {
            let addr = state
                .regs
                .read(rs1)
                .wrapping_add((imm as i64).cast_unsigned());
            state.memory.write(addr, state.regs.read(rs2) as u32)?;
        }
        Rv64Instruction::Sd { rs2, rs1, imm } => {
            let addr = state
                .regs
                .read(rs1)
                .wrapping_add((imm as i64).cast_unsigned());
            state.memory.write(addr, state.regs.read(rs2))?;
        }

        Rv64Instruction::Beq { rs1, rs2, imm } => {
            if state.regs.read(rs1) == state.regs.read(rs2) {
                let old_pc = state
                    .instruction_fetcher
                    .get_pc()
                    .wrapping_sub(instruction.size().into());
                return state
                    .instruction_fetcher
                    .set_pc(
                        &mut state.memory,
                        old_pc.wrapping_add((imm as i64).cast_unsigned()),
                    )
                    .map_err(ExecutionError::from);
            }
        }
        Rv64Instruction::Bne { rs1, rs2, imm } => {
            if state.regs.read(rs1) != state.regs.read(rs2) {
                let old_pc = state
                    .instruction_fetcher
                    .get_pc()
                    .wrapping_sub(instruction.size().into());
                return state
                    .instruction_fetcher
                    .set_pc(
                        &mut state.memory,
                        old_pc.wrapping_add((imm as i64).cast_unsigned()),
                    )
                    .map_err(ExecutionError::from);
            }
        }
        Rv64Instruction::Blt { rs1, rs2, imm } => {
            if state.regs.read(rs1).cast_signed() < state.regs.read(rs2).cast_signed() {
                let old_pc = state
                    .instruction_fetcher
                    .get_pc()
                    .wrapping_sub(instruction.size().into());
                return state
                    .instruction_fetcher
                    .set_pc(
                        &mut state.memory,
                        old_pc.wrapping_add((imm as i64).cast_unsigned()),
                    )
                    .map_err(ExecutionError::from);
            }
        }
        Rv64Instruction::Bge { rs1, rs2, imm } => {
            if state.regs.read(rs1).cast_signed() >= state.regs.read(rs2).cast_signed() {
                let old_pc = state
                    .instruction_fetcher
                    .get_pc()
                    .wrapping_sub(instruction.size().into());
                return state
                    .instruction_fetcher
                    .set_pc(
                        &mut state.memory,
                        old_pc.wrapping_add((imm as i64).cast_unsigned()),
                    )
                    .map_err(ExecutionError::from);
            }
        }
        Rv64Instruction::Bltu { rs1, rs2, imm } => {
            if state.regs.read(rs1) < state.regs.read(rs2) {
                let old_pc = state
                    .instruction_fetcher
                    .get_pc()
                    .wrapping_sub(instruction.size().into());
                return state
                    .instruction_fetcher
                    .set_pc(
                        &mut state.memory,
                        old_pc.wrapping_add((imm as i64).cast_unsigned()),
                    )
                    .map_err(ExecutionError::from);
            }
        }
        Rv64Instruction::Bgeu { rs1, rs2, imm } => {
            if state.regs.read(rs1) >= state.regs.read(rs2) {
                let old_pc = state
                    .instruction_fetcher
                    .get_pc()
                    .wrapping_sub(instruction.size().into());
                return state
                    .instruction_fetcher
                    .set_pc(
                        &mut state.memory,
                        old_pc.wrapping_add((imm as i64).cast_unsigned()),
                    )
                    .map_err(ExecutionError::from);
            }
        }

        Rv64Instruction::Lui { rd, imm } => {
            state.regs.write(rd, (imm as i64).cast_unsigned());
        }

        Rv64Instruction::Auipc { rd, imm } => {
            let old_pc = state
                .instruction_fetcher
                .get_pc()
                .wrapping_sub(instruction.size().into());
            state
                .regs
                .write(rd, old_pc.wrapping_add((imm as i64).cast_unsigned()));
        }

        Rv64Instruction::Jal { rd, imm } => {
            let pc = state.instruction_fetcher.get_pc();
            let old_pc = pc.wrapping_sub(instruction.size().into());
            state.regs.write(rd, pc);
            return state
                .instruction_fetcher
                .set_pc(
                    &mut state.memory,
                    old_pc.wrapping_add((imm as i64).cast_unsigned()),
                )
                .map_err(ExecutionError::from);
        }

        Rv64Instruction::Fence { .. } => {
            // NOP for single-threaded
        }

        Rv64Instruction::Ecall => {
            return state.system_instruction_handler.handle_ecall(
                &mut state.regs,
                &mut state.memory,
                &mut state.instruction_fetcher,
            );
        }
        Rv64Instruction::Ebreak => {
            state.system_instruction_handler.handle_ebreak(
                &mut state.regs,
                &mut state.memory,
                state.instruction_fetcher.get_pc(),
                Rv64Instruction::Ebreak,
            );
        }

        Rv64Instruction::Unimp => {
            let old_pc = state
                .instruction_fetcher
                .get_pc()
                .wrapping_sub(instruction.size().into());
            return Err(ExecutionError::UnimpInstruction { address: old_pc });
        }

        Rv64Instruction::Invalid(raw_instruction) => {
            let old_pc = state
                .instruction_fetcher
                .get_pc()
                .wrapping_sub(instruction.size().into());
            return Err(ExecutionError::InvalidInstruction {
                address: old_pc,
                instruction: raw_instruction,
            });
        }
    }

    Ok(ControlFlow::Continue(()))
}
