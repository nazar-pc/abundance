use crate::instruction::MachineModePlaceholder;
use crate::interpreter::{CoreConfig, MISA_A, MISA_B, MISA_I, MISA_M};
use ab_riscv_interpreter::prelude::*;
use ab_riscv_macros::{instruction, instruction_execution};
use ab_riscv_primitives::prelude::*;
use core::fmt;
use core::ops::ControlFlow;

/// Configuration of the RV32I-max core
pub(crate) const ABUNDANCE_RV32I_MAX_CONFIG: CoreConfig = CoreConfig {
    misa_extensions: MISA_A | MISA_B | MISA_I | MISA_M,
    zkr: true,
    zca: true,
};

/// All instructions supported by the interpreter for RV32I base ISA
pub(crate) type AbundanceRv32IMaxInstruction =
    AbundanceRv32IMaxInstructionPrototype<BasicHart<Reg<u32>>>;

/// All instructions supported by the interpreter for RV32I base ISA
#[instruction(
    inherit = [
        Rv32Instruction,
        Rv32AInstruction,
        Rv32BInstruction,
        Rv32MInstruction,
        Rv32ZabhaInstruction,
        Rv32ZacasInstruction,
        Rv32ZalasrInstruction,
        Rv32ZbcInstruction,
        Rv32ZcaInstruction,
        Rv32ZcbInstruction,
        Rv32ZcmpInstruction,
        Rv32ZknInstruction,
        ZawrsInstruction,
        ZicondInstruction,
        ZicsrInstruction,
        ZifenceiInstruction,
        ZkrInstruction,
        ZvbbInstruction,
        ZvbcInstruction,
        ZveXxInstruction,
        MachineModePlaceholder,
    ],
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AbundanceRv32IMaxInstructionPrototype<Hart>
where
    Hart: HartConfig, {}

#[instruction]
const impl<Reg, Hart> Instruction for AbundanceRv32IMaxInstructionPrototype<Hart>
where
    Hart: [const] HartConfig<Reg = Reg>,
{
    const ALIGNMENT: u8 = align_of::<u32>() as u8;

    type Hart = Hart;

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
impl<Reg, Hart> fmt::Display for AbundanceRv32IMaxInstructionPrototype<Hart>
where
    Reg: Register,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[instruction_execution]
impl<Reg, Hart> ExecutableInstructionOperands for AbundanceRv32IMaxInstructionPrototype<Hart> where
    Hart: HartConfig<Reg = Reg>
{
}

#[instruction_execution]
#[expect(
    clippy::useless_conversion,
    reason = "https://github.com/rust-lang/rust-clippy/issues/17083"
)]
impl<Reg, Hart, Env> ExecutableInstructionCsr<Env> for AbundanceRv32IMaxInstructionPrototype<Hart> where
    Hart: HartConfig<Reg = Reg>
{
}

#[instruction_execution]
impl<Reg, Hart, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
    for AbundanceRv32IMaxInstructionPrototype<Hart>
where
    Reg: Register,
    Hart: HartConfig<Reg = Reg>,
{
    fn execute(
        self,
        Rs1Rs2OperandValues {
            rs1_value,
            rs2_value,
        }: Rs1Rs2OperandValues<Reg::Type>,
        regs: &mut Regs,
        env: &mut Env,
        memory: &mut Memory,
        program_counter: &mut PC,
    ) -> ExecutionResult<Reg> {
        ExecutionResult::ContinueNoWrite
    }
}
