use ab_riscv_interpreter::rv64::b::zbb::rv64_zbb_helpers;
use ab_riscv_interpreter::rv64::b::zbc::rv64_zbc_helpers;
use ab_riscv_interpreter::{
    ExecutableInstruction, ExecutionError, InterpreterState, ProgramCounter,
    SystemInstructionHandler, VirtualMemory,
};
use ab_riscv_macros::{instruction, instruction_execution};
use ab_riscv_primitives::instructions::Instruction;
use ab_riscv_primitives::instructions::rv64::Rv64Instruction;
use ab_riscv_primitives::instructions::rv64::b::zba::Rv64ZbaInstruction;
use ab_riscv_primitives::instructions::rv64::b::zbb::Rv64ZbbInstruction;
use ab_riscv_primitives::instructions::rv64::b::zbc::Rv64ZbcInstruction;
use ab_riscv_primitives::instructions::rv64::b::zbs::Rv64ZbsInstruction;
use ab_riscv_primitives::instructions::rv64::m::Rv64MInstruction;
use ab_riscv_primitives::instructions::rv64::zk::zbkb::Rv64ZbkbInstruction;
use ab_riscv_primitives::instructions::rv64::zk::zkn::zknh::Rv64ZknhInstruction;
use ab_riscv_primitives::instructions::zicond::ZicondInstruction;
use ab_riscv_primitives::registers::general_purpose::Register;
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
        Rv64Instruction,
        Rv64MInstruction,
        Rv64BInstruction,
        Rv64ZbcInstruction,
        Rv64ZbkbInstruction,
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
    Reg: fmt::Display + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[instruction_execution]
impl<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for ContractInstructionPrototype<Reg>
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
        Ok(ControlFlow::Continue(()))
    }
}
