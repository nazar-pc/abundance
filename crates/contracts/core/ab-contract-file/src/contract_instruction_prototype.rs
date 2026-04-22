use ab_riscv_interpreter::prelude::*;
use ab_riscv_macros::{instruction, instruction_execution};
use ab_riscv_primitives::prelude::*;
use core::fmt;
use core::ops::ControlFlow;

/// An instruction type used by contracts
#[instruction(
    ignore = [Fence, Ecall],
    reorder = [
        Ld,
        Sd,
        Add,
        Addi,
        Xor,
        Rori,
        Srli,
        Or,
        And,
        Slli,
        Lbu,
        Auipc,
        Jalr,
        Sb,
        Roriw,
        Sub,
        Sltu,
        Mulhu,
        Mul,
        Sh1add,
    ],
    inherit = [
        Rv64ZcaInstruction,
        Rv64ZcbInstruction,
        Rv64ZcmpInstruction,
        Rv64Instruction,
        Rv64MInstruction,
        Rv64BInstruction,
        Rv64ZbcInstruction,
        Rv64ZknInstruction,
        ZicondInstruction,
        Rv64ZknhInstruction,
    ],
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractInstructionPrototype<Reg> {}

#[instruction]
impl<Reg> const Instruction for ContractInstructionPrototype<Reg>
where
    Reg: [const] Register<Type = u64>,
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
impl<Reg> fmt::Display for ContractInstructionPrototype<Reg>
where
    Reg: Register,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        InterpreterState<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for ContractInstructionPrototype<Reg>
where
    Reg: Register<Type = u64>,
    Memory: VirtualMemory,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
    InstructionHandler: SystemInstructionHandler<Reg, Regs, Memory, PC, CustomError>,
{
    #[inline(always)]
    fn execute(
        self,
        state: &mut InterpreterState<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, CustomError>> {
        Ok(ControlFlow::Continue(()))
    }
}

impl<Reg> ContractInstructionPrototype<Reg> {
    /// Check if the instruction is a jump instruction of any kind (affects program counter)
    #[inline]
    pub fn is_jump(&self) -> bool {
        matches!(
            self,
            Self::CJ { .. }
                | Self::CBeqz { .. }
                | Self::CBnez { .. }
                | Self::CJr { .. }
                | Self::CJalr { .. }
                | Self::CmPopretz { .. }
                | Self::CmPopret { .. }
                | Self::Jalr { .. }
                | Self::Beq { .. }
                | Self::Bne { .. }
                | Self::Blt { .. }
                | Self::Bge { .. }
                | Self::Bltu { .. }
                | Self::Bgeu { .. }
                | Self::Jal { .. }
        )
    }
}
