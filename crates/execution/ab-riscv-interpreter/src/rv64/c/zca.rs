//! RV64 Zca extension

#[cfg(test)]
mod tests;

use crate::{
    ExecutableInstruction, ExecutionError, InterpreterState, ProgramCounter,
    SystemInstructionHandler, VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::instructions::Instruction;
use ab_riscv_primitives::instructions::rv64::c::zca::Rv64ZcaInstruction;
use ab_riscv_primitives::registers::general_purpose::Register;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for Rv64ZcaInstruction<Reg>
where
    Reg: Register<Type = u64>,
    [(); Reg::N]:,
    Memory: VirtualMemory,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
    InstructionHandler: SystemInstructionHandler<Reg, Memory, PC, CustomError>,
{
    #[inline(always)]
    fn execute(
        self,
        state: &mut InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, CustomError>> {
        match self {
            // Quadrant 00
            Self::CAddi4spn { rd, nzuimm } => {
                let sp_val = state.regs.read(Reg::SP);
                state.regs.write(rd, sp_val.wrapping_add(u64::from(nzuimm)));
            }
            Self::CLw { rd, rs1, uimm } => {
                let addr = state.regs.read(rs1).wrapping_add(u64::from(uimm));
                let value = i64::from(state.memory.read::<i32>(addr)?);
                state.regs.write(rd, value.cast_unsigned());
            }
            Self::CLd { rd, rs1, uimm } => {
                let addr = state.regs.read(rs1).wrapping_add(u64::from(uimm));
                let value = state.memory.read::<u64>(addr)?;
                state.regs.write(rd, value);
            }
            Self::CSw { rs1, rs2, uimm } => {
                let addr = state.regs.read(rs1).wrapping_add(u64::from(uimm));
                state.memory.write(addr, state.regs.read(rs2) as u32)?;
            }
            Self::CSd { rs1, rs2, uimm } => {
                let addr = state.regs.read(rs1).wrapping_add(u64::from(uimm));
                state.memory.write(addr, state.regs.read(rs2))?;
            }

            // Quadrant 01
            Self::CNop => {}
            Self::CAddi { rd, nzimm } => {
                let value = state
                    .regs
                    .read(rd)
                    .wrapping_add(i64::from(nzimm).cast_unsigned());
                state.regs.write(rd, value);
            }
            Self::CAddiw { rd, imm } => {
                let sum = (state.regs.read(rd) as i32).wrapping_add(i32::from(imm));
                state.regs.write(rd, i64::from(sum).cast_unsigned());
            }
            Self::CLi { rd, imm } => {
                state.regs.write(rd, i64::from(imm).cast_unsigned());
            }
            Self::CAddi16sp { nzimm } => {
                let value = state
                    .regs
                    .read(Reg::SP)
                    .wrapping_add(i64::from(nzimm).cast_unsigned());
                state.regs.write(Reg::SP, value);
            }
            Self::CLui { rd, nzimm } => {
                state.regs.write(rd, i64::from(nzimm).cast_unsigned());
            }
            Self::CSrli { rd, shamt } => {
                let value = state.regs.read(rd) >> shamt;
                state.regs.write(rd, value);
            }
            Self::CSrai { rd, shamt } => {
                let value = state.regs.read(rd).cast_signed() >> shamt;
                state.regs.write(rd, value.cast_unsigned());
            }
            Self::CAndi { rd, imm } => {
                let value = state.regs.read(rd) & i64::from(imm).cast_unsigned();
                state.regs.write(rd, value);
            }
            Self::CSub { rd, rs2 } => {
                let value = state.regs.read(rd).wrapping_sub(state.regs.read(rs2));
                state.regs.write(rd, value);
            }
            Self::CXor { rd, rs2 } => {
                let value = state.regs.read(rd) ^ state.regs.read(rs2);
                state.regs.write(rd, value);
            }
            Self::COr { rd, rs2 } => {
                let value = state.regs.read(rd) | state.regs.read(rs2);
                state.regs.write(rd, value);
            }
            Self::CAnd { rd, rs2 } => {
                let value = state.regs.read(rd) & state.regs.read(rs2);
                state.regs.write(rd, value);
            }
            Self::CSubw { rd, rs2 } => {
                let diff = (state.regs.read(rd) as i32).wrapping_sub(state.regs.read(rs2) as i32);
                state.regs.write(rd, i64::from(diff).cast_unsigned());
            }
            Self::CAddw { rd, rs2 } => {
                let sum = (state.regs.read(rd) as i32).wrapping_add(state.regs.read(rs2) as i32);
                state.regs.write(rd, i64::from(sum).cast_unsigned());
            }
            Self::CJ { imm } => {
                let old_pc = state.instruction_fetcher.old_pc(self.size());
                return state
                    .instruction_fetcher
                    .set_pc(
                        &state.memory,
                        old_pc.wrapping_add(i64::from(imm).cast_unsigned()),
                    )
                    .map_err(ExecutionError::from);
            }
            Self::CBeqz { rs1, imm } => {
                if state.regs.read(rs1) == 0 {
                    let old_pc = state.instruction_fetcher.old_pc(self.size());
                    return state
                        .instruction_fetcher
                        .set_pc(
                            &state.memory,
                            old_pc.wrapping_add(i64::from(imm).cast_unsigned()),
                        )
                        .map_err(ExecutionError::from);
                }
            }
            Self::CBnez { rs1, imm } => {
                if state.regs.read(rs1) != 0 {
                    let old_pc = state.instruction_fetcher.old_pc(self.size());
                    return state
                        .instruction_fetcher
                        .set_pc(
                            &state.memory,
                            old_pc.wrapping_add(i64::from(imm).cast_unsigned()),
                        )
                        .map_err(ExecutionError::from);
                }
            }

            // Quadrant 10
            Self::CSlli { rd, shamt } => {
                let value = state.regs.read(rd) << shamt;
                state.regs.write(rd, value);
            }
            Self::CLwsp { rd, uimm } => {
                let addr = state.regs.read(Reg::SP).wrapping_add(u64::from(uimm));
                let value = i64::from(state.memory.read::<i32>(addr)?);
                state.regs.write(rd, value.cast_unsigned());
            }
            Self::CLdsp { rd, uimm } => {
                let addr = state.regs.read(Reg::SP).wrapping_add(u64::from(uimm));
                let value = state.memory.read::<u64>(addr)?;
                state.regs.write(rd, value);
            }
            Self::CJr { rs1 } => {
                let target = state.regs.read(rs1) & !1;
                return state
                    .instruction_fetcher
                    .set_pc(&state.memory, target)
                    .map_err(ExecutionError::from);
            }
            Self::CMv { rd, rs2 } => {
                state.regs.write(rd, state.regs.read(rs2));
            }
            Self::CEbreak => {
                state.system_instruction_handler.handle_ebreak(
                    &mut state.regs,
                    &mut state.memory,
                    state.instruction_fetcher.get_pc(),
                );
            }
            Self::CJalr { rs1 } => {
                let target = state.regs.read(rs1) & !1;
                let return_addr = state.instruction_fetcher.get_pc();
                state.regs.write(Reg::RA, return_addr);
                return state
                    .instruction_fetcher
                    .set_pc(&state.memory, target)
                    .map_err(ExecutionError::from);
            }
            Self::CAdd { rd, rs2 } => {
                let value = state.regs.read(rd).wrapping_add(state.regs.read(rs2));
                state.regs.write(rd, value);
            }
            Self::CSwsp { rs2, uimm } => {
                let addr = state.regs.read(Reg::SP).wrapping_add(u64::from(uimm));
                state.memory.write(addr, state.regs.read(rs2) as u32)?;
            }
            Self::CSdsp { rs2, uimm } => {
                let addr = state.regs.read(Reg::SP).wrapping_add(u64::from(uimm));
                state.memory.write(addr, state.regs.read(rs2))?;
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}
